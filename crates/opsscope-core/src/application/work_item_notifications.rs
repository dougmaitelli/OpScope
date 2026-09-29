use super::{
    ConnectionRepository, Notification, NotificationProcessingFailure as Failure,
    NotificationSeverity, NotificationSink, PersistenceFailure, SettingsRepository,
};
use crate::domain::{
    ChangeRequest, ChangeRequestReviewStatus, ChangeRequestState, Issue, IssueState,
    RelevanceReason, Repository,
};
use crate::source_data::{RefreshMode, SourceData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

mod transitions;

/// None means this resource kind has not had a successful initial observation.
/// Closed items are retained so an observed reopen is not mistaken for a new issue.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct WorkItemNotificationState {
    pub pull_requests: Option<Vec<ChangeRequest>>,
    pub issues: Option<Vec<Issue>>,
}

pub trait WorkItemNotificationRepository: Send + Sync {
    fn load_work_items(
        &self,
        source_id: &str,
        repository_id: &str,
        account_id: &str,
    ) -> Result<WorkItemNotificationState, PersistenceFailure>;
    fn save_work_items(
        &self,
        source_id: &str,
        repository_id: &str,
        account_id: &str,
        state: &WorkItemNotificationState,
    ) -> Result<(), PersistenceFailure>;
}

#[derive(Clone)]
pub struct NotifyWorkItems {
    states: Arc<dyn WorkItemNotificationRepository>,
    settings: Arc<dyn SettingsRepository>,
    connections: Arc<dyn ConnectionRepository>,
    sink: Arc<dyn NotificationSink>,
}

impl NotifyWorkItems {
    #[must_use]
    pub fn new(
        states: Arc<dyn WorkItemNotificationRepository>,
        settings: Arc<dyn SettingsRepository>,
        connections: Arc<dyn ConnectionRepository>,
        sink: Arc<dyn NotificationSink>,
    ) -> Self {
        Self {
            states,
            settings,
            connections,
            sink,
        }
    }

    /// Only successful, fresh lists may be supplied. Missing items are resolved
    /// through details; a failed lookup never means closed and is retried later.
    pub async fn observe(
        &self,
        source_id: &str,
        repository: &Repository,
        source_data: &dyn SourceData,
        pull_requests: Option<&[ChangeRequest]>,
        issues: Option<&[Issue]>,
    ) -> Result<(), Failure> {
        if pull_requests.is_none() && issues.is_none() {
            return Ok(());
        }
        let connection = self
            .connections
            .get(source_id)
            .map_err(|_| Failure::StorageUnavailable)?
            .ok_or(Failure::StorageUnavailable)?;
        let account_id = &connection.account.external_id;
        let settings = self
            .settings
            .load_settings()
            .map_err(|_| Failure::StorageUnavailable)?;
        let previous = self
            .states
            .load_work_items(source_id, &repository.id, account_id)
            .map_err(|_| Failure::StorageUnavailable)?;
        let pull_requests = pull_requests.filter(|_| settings.pull_requests_enabled);
        let issues = issues.filter(|_| settings.issues_enabled);
        if pull_requests.is_none() && issues.is_none() {
            return Ok(());
        }
        let mut next = previous.clone();
        let mut events = Vec::new();

        if let Some(current) = pull_requests {
            let mut observed = current.to_vec();
            for old in previous.pull_requests.iter().flatten().filter(|old| {
                old.state == ChangeRequestState::Open
                    && !current.iter().any(|item| item.id == old.id)
            }) {
                match source_data
                    .change_request_details(source_id, repository, old.number, RefreshMode::Force)
                    .await
                {
                    Ok(Some(details)) if details.change_request.id == old.id => {
                        observed.push(details.change_request)
                    }
                    _ => eprintln!(
                        "unable to resolve PR #{} for notification observation",
                        old.number
                    ),
                }
            }
            let old = previous.pull_requests.as_deref();
            events.extend(transitions::pull_requests(
                old, &observed, &settings, account_id,
            ));
            // Preserve unknown relevance/review observations to avoid false transitions
            // when optional metadata recovers. They are not used to deliver events.
            let mut merged: HashMap<_, _> = old
                .unwrap_or_default()
                .iter()
                .map(|item| (item.id.clone(), item.clone()))
                .collect();
            for mut item in observed {
                if let Some(old) = merged.get(&item.id) {
                    if review_requested(&item).is_none() {
                        item.relevance = old.relevance.clone();
                    }
                    if item.review_status == ChangeRequestReviewStatus::Unknown {
                        item.review_status = old.review_status;
                    }
                }
                merged.insert(item.id.clone(), item);
            }
            let mut items: Vec<_> = merged.into_values().collect();
            items.sort_by(|a, b| a.id.cmp(&b.id));
            next.pull_requests = Some(items);
        }
        if let Some(current) = issues {
            let mut observed = current.to_vec();
            for old in previous.issues.iter().flatten().filter(|old| {
                old.state == IssueState::Open && !current.iter().any(|item| item.id == old.id)
            }) {
                match source_data
                    .issue_details(source_id, repository, old.number, RefreshMode::Force)
                    .await
                {
                    Ok(Some(details)) if details.issue.id == old.id => observed.push(details.issue),
                    _ => eprintln!(
                        "unable to resolve issue #{} for notification observation",
                        old.number
                    ),
                }
            }
            let old = previous.issues.as_deref();
            events.extend(transitions::issues(
                old,
                &observed,
                &settings,
                account_id,
                connection.account.handle.as_deref(),
            ));
            let mut merged: HashMap<_, _> = old
                .unwrap_or_default()
                .iter()
                .map(|item| (item.id.clone(), item.clone()))
                .collect();
            for item in observed {
                merged.insert(item.id.clone(), item);
            }
            let mut items: Vec<_> = merged.into_values().collect();
            items.sort_by(|a, b| a.id.cmp(&b.id));
            next.issues = Some(items);
        }

        // Advance disabled/irrelevant observations too, preventing replay on settings changes.
        // Persist before delivery for the same at-most-once semantics as workflow alerts.
        self.states
            .save_work_items(source_id, &repository.id, account_id, &next)
            .map_err(|_| Failure::StorageUnavailable)?;
        if events.is_empty() {
            return Ok(());
        }
        self.sink
            .send(&Notification {
                title: format!(
                    "{} updates in {}/{}",
                    events.len(),
                    repository.owner,
                    repository.name
                ),
                body: events.join("\n"),
                severity: NotificationSeverity::Info,
            })
            .await
            .map_err(|_| Failure::DeliveryUnavailable)
    }
}

fn review_requested(item: &ChangeRequest) -> Option<bool> {
    if item
        .relevance
        .reasons
        .contains(&RelevanceReason::ReviewRequested)
    {
        Some(true)
    } else if item.relevance.complete {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
