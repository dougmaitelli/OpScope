use super::{NotifyRepositoryFailures, RepositorySelectionRepository, TrackChangeRequestActivity};
use crate::domain::ChangeRequestState;
use crate::source_data::{RefreshMode, SourceData};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_SYNCHRONIZATION_INTERVAL: Duration =
    Duration::from_secs(super::DEFAULT_SYNCHRONIZATION_INTERVAL_SECONDS);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SynchronizationFailure {
    StorageUnavailable,
}

impl Display for SynchronizationFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("synchronization storage unavailable")
    }
}

impl Error for SynchronizationFailure {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SynchronizationStatus {
    pub active_source_count: usize,
    pub last_completed_at: Option<u64>,
    pub last_failed_repository_count: usize,
}

impl SynchronizationStatus {
    #[must_use]
    pub const fn is_running(self) -> bool {
        self.active_source_count > 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SynchronizationSummary {
    pub selected_repository_count: usize,
    pub synchronized_repository_count: usize,
    pub failed_repository_count: usize,
    pub skipped_repository_count: usize,
}

impl SynchronizationSummary {
    #[must_use]
    pub const fn already_running(self) -> bool {
        self.skipped_repository_count > 0
    }
}

#[derive(Clone)]
pub struct SynchronizeSources {
    work_item_notifications: Option<super::NotifyWorkItems>,
    source_data: Arc<dyn SourceData>,
    selections: Arc<dyn RepositorySelectionRepository>,
    failure_notifications: NotifyRepositoryFailures,
    change_request_activity: TrackChangeRequestActivity,
    active_sources: Arc<Mutex<HashSet<String>>>,
    last_completed_at: Arc<AtomicU64>,
    last_failed_repository_count: Arc<AtomicUsize>,
}

impl SynchronizeSources {
    #[must_use]
    pub fn new(
        source_data: Arc<dyn SourceData>,
        selections: Arc<dyn RepositorySelectionRepository>,
        failure_notifications: NotifyRepositoryFailures,
        change_request_activity: TrackChangeRequestActivity,
    ) -> Self {
        Self {
            work_item_notifications: None,
            source_data,
            selections,
            failure_notifications,
            change_request_activity,
            active_sources: Arc::new(Mutex::new(HashSet::new())),
            last_completed_at: Arc::new(AtomicU64::new(0)),
            last_failed_repository_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    #[must_use]
    pub fn status(&self) -> SynchronizationStatus {
        let last_completed_at = self.last_completed_at.load(Ordering::Acquire);
        SynchronizationStatus {
            active_source_count: lock_active_sources(&self.active_sources).len(),
            last_completed_at: nonzero_timestamp(last_completed_at),
            last_failed_repository_count: self.last_failed_repository_count.load(Ordering::Relaxed),
        }
    }

    #[must_use]
    pub fn with_work_item_notifications(
        mut self,
        notifications: Option<super::NotifyWorkItems>,
    ) -> Self {
        self.work_item_notifications = notifications;
        self
    }

    pub async fn execute(&self) -> Result<SynchronizationSummary, SynchronizationFailure> {
        let pull_requests_enabled = self
            .source_data
            .feature_enabled(super::SourceCapability::ChangeRequests)
            .map_err(|_| SynchronizationFailure::StorageUnavailable)?;
        let issues_enabled = self
            .source_data
            .feature_enabled(super::SourceCapability::Issues)
            .map_err(|_| SynchronizationFailure::StorageUnavailable)?;
        let selections = self
            .selections
            .list()
            .map_err(|_| SynchronizationFailure::StorageUnavailable)?;
        let mut summary = SynchronizationSummary {
            selected_repository_count: selections.len(),
            ..SynchronizationSummary::default()
        };

        for source in self
            .source_data
            .sources()
            .map_err(|_| SynchronizationFailure::StorageUnavailable)?
        {
            let selected_ids = selections
                .iter()
                .filter(|selection| selection.source_id == source.id)
                .map(|selection| selection.repository_id.clone())
                .collect::<HashSet<_>>();
            if selected_ids.is_empty() {
                continue;
            }

            let Some(_permit) = SourceSynchronizationPermit::acquire(
                self.active_sources.clone(),
                source.id.clone(),
            ) else {
                summary.skipped_repository_count += selected_ids.len();
                continue;
            };

            let repositories = match self
                .source_data
                .repositories(&source.id, RefreshMode::Force)
                .await
            {
                Ok(Some(repositories)) => repositories,
                Ok(None) | Err(_) => {
                    summary.failed_repository_count += selected_ids.len();
                    continue;
                }
            };
            let available_ids = repositories
                .iter()
                .map(|repository| repository.id.as_str())
                .collect::<HashSet<_>>();
            summary.failed_repository_count += selected_ids
                .iter()
                .filter(|id| !available_ids.contains(id.as_str()))
                .count();

            for repository in repositories
                .into_iter()
                .filter(|repository| selected_ids.contains(&repository.id))
            {
                let mut current_pull_requests = None;
                let mut current_issues = None;
                let mut item_sync_failed = false;
                if pull_requests_enabled
                    && source
                        .descriptor
                        .supports(super::SourceCapability::ChangeRequests)
                {
                    let previous = self
                        .change_request_activity
                        .previous(&source.id, &repository.id)
                        .ok()
                        .flatten();
                    if let Ok(Some(current)) = self
                        .source_data
                        .change_requests(&source.id, &repository, RefreshMode::Force)
                        .await
                    {
                        let current_ids = current
                            .iter()
                            .map(|change_request| change_request.id.as_str())
                            .collect::<HashSet<_>>();
                        let mut departed = Vec::new();
                        if let Some(previous) = &previous {
                            for change_request in previous.iter().filter(|change_request| {
                                !current_ids.contains(change_request.id.as_str())
                            }) {
                                if let Ok(Some(details)) = self
                                    .source_data
                                    .change_request_details(
                                        &source.id,
                                        &repository,
                                        change_request.number,
                                        RefreshMode::Force,
                                    )
                                    .await
                                    && details.change_request.state != ChangeRequestState::Open
                                {
                                    departed.push(details.change_request);
                                }
                            }
                        }
                        _ = self.change_request_activity.observe(
                            &source,
                            &repository,
                            &current,
                            &departed,
                        );
                        let mut observed = current;
                        observed.extend(departed);
                        current_pull_requests = Some(observed);
                    } else {
                        item_sync_failed = true;
                    }
                }
                if issues_enabled && source.descriptor.supports(super::SourceCapability::Issues) {
                    match self
                        .source_data
                        .issues(&source.id, &repository, RefreshMode::Force)
                        .await
                    {
                        Ok(items) => current_issues = items,
                        Err(failure) => {
                            eprintln!(
                                "failed to synchronize issues for {}/{}: {failure}",
                                repository.owner, repository.name
                            );
                            item_sync_failed = true;
                        }
                    }
                }
                if let Some(notifications) = &self.work_item_notifications
                    && let Err(failure) = notifications
                        .observe(
                            &source.id,
                            &repository,
                            self.source_data.as_ref(),
                            current_pull_requests.as_deref(),
                            current_issues.as_deref(),
                        )
                        .await
                {
                    eprintln!(
                        "failed to process notifications for {}/{}: {failure}",
                        repository.owner, repository.name
                    );
                    item_sync_failed = true;
                }
                let workflows = self
                    .source_data
                    .workflows(&source.id, &repository, RefreshMode::Force)
                    .await;
                let runs = self
                    .source_data
                    .workflow_runs(&source.id, &repository, RefreshMode::Force)
                    .await;
                match (workflows, runs) {
                    (Ok(workflows), Ok(runs)) if !runs.stale => {
                        if let Err(failure) = self
                            .failure_notifications
                            .observe(&source.id, &repository, &workflows, &runs.runs)
                            .await
                        {
                            eprintln!(
                                "failed to process workflow notifications for {}/{}: {failure}",
                                repository.owner, repository.name
                            );
                            item_sync_failed = true;
                        }
                        if item_sync_failed {
                            summary.failed_repository_count += 1;
                        } else {
                            summary.synchronized_repository_count += 1;
                        }
                    }
                    _ => summary.failed_repository_count += 1,
                }
            }
        }

        if summary.synchronized_repository_count + summary.failed_repository_count > 0 {
            self.last_failed_repository_count
                .store(summary.failed_repository_count, Ordering::Relaxed);
            self.last_completed_at.store(now(), Ordering::Release);
        }
        Ok(summary)
    }
}

struct SourceSynchronizationPermit {
    active_sources: Arc<Mutex<HashSet<String>>>,
    source_id: String,
}

impl SourceSynchronizationPermit {
    fn acquire(active_sources: Arc<Mutex<HashSet<String>>>, source_id: String) -> Option<Self> {
        let inserted = lock_active_sources(&active_sources).insert(source_id.clone());
        inserted.then_some(Self {
            active_sources,
            source_id,
        })
    }
}

impl Drop for SourceSynchronizationPermit {
    fn drop(&mut self) {
        lock_active_sources(&self.active_sources).remove(&self.source_id);
    }
}

fn lock_active_sources(
    active_sources: &Mutex<HashSet<String>>,
) -> std::sync::MutexGuard<'_, HashSet<String>> {
    match active_sources.lock() {
        Ok(active_sources) => active_sources,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

const fn nonzero_timestamp(timestamp: u64) -> Option<u64> {
    if timestamp == 0 {
        None
    } else {
        Some(timestamp)
    }
}

#[cfg(test)]
mod tests;
