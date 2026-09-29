use super::{
    ConnectedSource, PersistenceFailure, RepositorySelection, RepositorySelectionRepository,
};
use crate::domain::{
    ChangeRequest, ChangeRequestCheckStatus, ChangeRequestMergeStatus, ChangeRequestReviewStatus,
    ChangeRequestState, Repository,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeRequestActivityKind {
    Opened,
    ReadyForReview,
    ReviewApproved,
    ChangesRequested,
    ChecksFailed,
    ChecksRecovered,
    ConflictDetected,
    ConflictResolved,
    Merged,
    Closed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRequestActivityEvent {
    pub id: String,
    pub kind: ChangeRequestActivityKind,
    pub occurred_at: String,
    pub source_id: String,
    pub source_name: String,
    pub source_abbreviation: String,
    pub repository_id: String,
    pub repository_owner: String,
    pub repository_name: String,
    pub change_request: ChangeRequest,
}

pub trait ActivityEventRepository: Send + Sync {
    fn load_change_request_state(
        &self,
        source_id: &str,
        repository_id: &str,
    ) -> Result<Option<Vec<ChangeRequest>>, PersistenceFailure>;

    fn save_change_request_observation(
        &self,
        source_id: &str,
        repository_id: &str,
        observed: &[ChangeRequest],
        events: &[ChangeRequestActivityEvent],
    ) -> Result<(), PersistenceFailure>;

    fn list_change_request_events(
        &self,
    ) -> Result<Vec<ChangeRequestActivityEvent>, PersistenceFailure>;
}

#[derive(Clone)]
pub struct TrackChangeRequestActivity {
    events: Arc<dyn ActivityEventRepository>,
}

impl TrackChangeRequestActivity {
    #[must_use]
    pub fn new(events: Arc<dyn ActivityEventRepository>) -> Self {
        Self { events }
    }

    pub fn observe(
        &self,
        source: &ConnectedSource,
        repository: &Repository,
        current: &[ChangeRequest],
        departed: &[ChangeRequest],
    ) -> Result<(), PersistenceFailure> {
        let previous = self
            .events
            .load_change_request_state(&source.id, &repository.id)?;
        let activity = if let Some(previous) = &previous {
            changed_events(source, repository, previous, current, departed)
        } else {
            current
                .iter()
                .map(|change_request| {
                    event(
                        source,
                        repository,
                        change_request,
                        ChangeRequestActivityKind::Opened,
                        &change_request.created_at,
                    )
                })
                .collect()
        };
        let current_ids = current
            .iter()
            .map(|change_request| change_request.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let departed_ids = departed
            .iter()
            .map(|change_request| change_request.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut observed = current.to_vec();
        if let Some(previous) = previous {
            observed.extend(previous.into_iter().filter(|change_request| {
                !current_ids.contains(change_request.id.as_str())
                    && !departed_ids.contains(change_request.id.as_str())
            }));
        }
        self.events.save_change_request_observation(
            &source.id,
            &repository.id,
            &observed,
            &activity,
        )
    }

    pub fn previous(
        &self,
        source_id: &str,
        repository_id: &str,
    ) -> Result<Option<Vec<ChangeRequest>>, PersistenceFailure> {
        self.events
            .load_change_request_state(source_id, repository_id)
    }
}

fn changed_events(
    source: &ConnectedSource,
    repository: &Repository,
    previous: &[ChangeRequest],
    current: &[ChangeRequest],
    departed: &[ChangeRequest],
) -> Vec<ChangeRequestActivityEvent> {
    let previous = previous
        .iter()
        .map(|change_request| (change_request.id.as_str(), change_request))
        .collect::<HashMap<_, _>>();
    let mut activity = Vec::new();
    for change_request in current {
        let Some(old) = previous.get(change_request.id.as_str()) else {
            activity.push(event(
                source,
                repository,
                change_request,
                ChangeRequestActivityKind::Opened,
                &change_request.created_at,
            ));
            continue;
        };
        if old.draft && !change_request.draft {
            activity.push(updated_event(
                source,
                repository,
                change_request,
                ChangeRequestActivityKind::ReadyForReview,
            ));
        }
        if old.review_status != change_request.review_status {
            let kind = match change_request.review_status {
                ChangeRequestReviewStatus::Approved => {
                    Some(ChangeRequestActivityKind::ReviewApproved)
                }
                ChangeRequestReviewStatus::ChangesRequested => {
                    Some(ChangeRequestActivityKind::ChangesRequested)
                }
                _ => None,
            };
            if let Some(kind) = kind {
                activity.push(updated_event(source, repository, change_request, kind));
            }
        }
        if old.check_status != change_request.check_status {
            let kind = match change_request.check_status {
                ChangeRequestCheckStatus::Failing => Some(ChangeRequestActivityKind::ChecksFailed),
                ChangeRequestCheckStatus::Passed
                    if old.check_status == ChangeRequestCheckStatus::Failing =>
                {
                    Some(ChangeRequestActivityKind::ChecksRecovered)
                }
                _ => None,
            };
            if let Some(kind) = kind {
                activity.push(updated_event(source, repository, change_request, kind));
            }
        }
        if old.merge_status != change_request.merge_status {
            let kind = match change_request.merge_status {
                ChangeRequestMergeStatus::Conflicting => {
                    Some(ChangeRequestActivityKind::ConflictDetected)
                }
                ChangeRequestMergeStatus::Ready | ChangeRequestMergeStatus::Blocked
                    if old.merge_status == ChangeRequestMergeStatus::Conflicting =>
                {
                    Some(ChangeRequestActivityKind::ConflictResolved)
                }
                _ => None,
            };
            if let Some(kind) = kind {
                activity.push(updated_event(source, repository, change_request, kind));
            }
        }
    }
    for change_request in departed {
        let kind = match change_request.state {
            ChangeRequestState::Merged => Some(ChangeRequestActivityKind::Merged),
            ChangeRequestState::Closed => Some(ChangeRequestActivityKind::Closed),
            ChangeRequestState::Open => None,
        };
        if let Some(kind) = kind {
            activity.push(updated_event(source, repository, change_request, kind));
        }
    }
    activity
}

fn updated_event(
    source: &ConnectedSource,
    repository: &Repository,
    change_request: &ChangeRequest,
    kind: ChangeRequestActivityKind,
) -> ChangeRequestActivityEvent {
    event(
        source,
        repository,
        change_request,
        kind,
        &change_request.updated_at,
    )
}

fn event(
    source: &ConnectedSource,
    repository: &Repository,
    change_request: &ChangeRequest,
    kind: ChangeRequestActivityKind,
    occurred_at: &str,
) -> ChangeRequestActivityEvent {
    ChangeRequestActivityEvent {
        id: format!(
            "{}:{}:{}:{kind:?}:{occurred_at}",
            source.id, repository.id, change_request.id
        ),
        kind,
        occurred_at: occurred_at.to_owned(),
        source_id: source.id.clone(),
        source_name: source.label.clone(),
        source_abbreviation: source.descriptor.abbreviation.clone(),
        repository_id: repository.id.clone(),
        repository_owner: repository.owner.clone(),
        repository_name: repository.name.clone(),
        change_request: change_request.clone(),
    }
}

#[derive(Clone)]
pub struct ListActivity {
    connections: Arc<dyn super::ConnectionRepository>,
    settings: Arc<dyn super::SettingsRepository>,
    events: Arc<dyn ActivityEventRepository>,
    selections: Arc<dyn RepositorySelectionRepository>,
}

impl ListActivity {
    #[must_use]
    pub fn new(
        events: Arc<dyn ActivityEventRepository>,
        selections: Arc<dyn RepositorySelectionRepository>,
        settings: Arc<dyn super::SettingsRepository>,
        connections: Arc<dyn super::ConnectionRepository>,
    ) -> Self {
        Self {
            events,
            selections,
            settings,
            connections,
        }
    }

    pub fn execute(&self) -> Result<Vec<ChangeRequestActivityEvent>, PersistenceFailure> {
        let settings = self.settings.load_settings()?;
        if !settings.pull_requests_enabled {
            return Ok(Vec::new());
        }
        let only_my_work = settings.only_my_work;
        let accounts = self
            .connections
            .list()?
            .into_iter()
            .map(|connection| (connection.id, connection.account.external_id))
            .collect::<HashMap<_, _>>();
        let selected = self
            .selections
            .list()?
            .into_iter()
            .collect::<std::collections::HashSet<RepositorySelection>>();
        let events = self.events.list_change_request_events()?;
        let mut relevance = HashMap::new();
        if only_my_work {
            // Use the newest event for closed PRs, then current observations for open PRs.
            // This also enriches historical events created before personal scope existed.
            let mut chronological = events.iter().collect::<Vec<_>>();
            chronological.sort_by(|left, right| left.occurred_at.cmp(&right.occurred_at));
            for event in chronological {
                relevance.insert(
                    (
                        event.source_id.clone(),
                        event.repository_id.clone(),
                        event.change_request.id.clone(),
                    ),
                    event.change_request.relevance.clone(),
                );
            }
            for selection in &selected {
                if let Some(current) = self
                    .events
                    .load_change_request_state(&selection.source_id, &selection.repository_id)?
                {
                    for item in current {
                        relevance.insert(
                            (
                                selection.source_id.clone(),
                                selection.repository_id.clone(),
                                item.id,
                            ),
                            item.relevance,
                        );
                    }
                }
            }
        }
        Ok(events
            .into_iter()
            .filter(|event| {
                !only_my_work
                    || relevance
                        .get(&(
                            event.source_id.clone(),
                            event.repository_id.clone(),
                            event.change_request.id.clone(),
                        ))
                        .is_some_and(|relevance| {
                            relevance.matches()
                                && relevance.account_id.as_ref().is_some_and(|account| {
                                    accounts.get(&event.source_id) == Some(account)
                                })
                        })
            })
            .filter(|event| {
                selected.contains(&RepositorySelection {
                    source_id: event.source_id.clone(),
                    repository_id: event.repository_id.clone(),
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests;
