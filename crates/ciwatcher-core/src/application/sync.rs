use super::{NotifyRepositoryFailures, RepositorySelectionRepository};
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
    source_data: Arc<dyn SourceData>,
    selections: Arc<dyn RepositorySelectionRepository>,
    failure_notifications: NotifyRepositoryFailures,
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
    ) -> Self {
        Self {
            source_data,
            selections,
            failure_notifications,
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

    pub async fn execute(&self) -> Result<SynchronizationSummary, SynchronizationFailure> {
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
                        summary.synchronized_repository_count += 1;
                        _ = self
                            .failure_notifications
                            .observe(&source.id, &repository, &workflows, &runs.runs)
                            .await;
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
mod tests {
    use super::*;
    use crate::application::{
        ConnectedSource, ConnectionValidationFailure, CredentialField, LatestRunNotificationState,
        NoopNotificationSink, NotificationStateRepository, PersistenceFailure, RepositorySelection,
        SourceDescriptor, SourceRepositorySelection,
    };
    use crate::domain::{Repository, RepositoryVisibility, Workflow, WorkflowRun, WorkflowRunLogs};
    use crate::source_data::{SourceDataFailure, WorkflowRunCollection};
    use async_trait::async_trait;

    struct TestSelections(Vec<RepositorySelection>);

    impl RepositorySelectionRepository for TestSelections {
        fn list(&self) -> Result<Vec<RepositorySelection>, PersistenceFailure> {
            Ok(self.0.clone())
        }

        fn replace_for_sources(
            &self,
            _selections: &[SourceRepositorySelection],
        ) -> Result<(), PersistenceFailure> {
            unreachable!("synchronization only reads selections")
        }
    }

    struct RecordingSourceData {
        refresh_modes: Mutex<Vec<RefreshMode>>,
    }

    #[derive(Default)]
    struct TestNotificationStates(Mutex<Option<Vec<LatestRunNotificationState>>>);

    impl NotificationStateRepository for TestNotificationStates {
        fn load(
            &self,
            _source_id: &str,
            _repository_id: &str,
        ) -> Result<Option<Vec<LatestRunNotificationState>>, PersistenceFailure> {
            Ok(self
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone())
        }

        fn replace(
            &self,
            _source_id: &str,
            _repository_id: &str,
            states: &[LatestRunNotificationState],
        ) -> Result<(), PersistenceFailure> {
            *self
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(states.to_vec());
            Ok(())
        }
    }

    impl RecordingSourceData {
        fn record(&self, refresh: RefreshMode) {
            self.refresh_modes
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(refresh);
        }
    }

    #[async_trait]
    impl SourceData for RecordingSourceData {
        fn sources(&self) -> Result<Vec<ConnectedSource>, SourceDataFailure> {
            Ok(vec![ConnectedSource {
                id: "source".to_owned(),
                label: "Source".to_owned(),
                descriptor: SourceDescriptor {
                    id: "module".to_owned(),
                    name: "Source".to_owned(),
                    description: "Test source".to_owned(),
                    abbreviation: "SO".to_owned(),
                    credential: CredentialField {
                        label: "Token".to_owned(),
                        placeholder: "token".to_owned(),
                        help: "Test token".to_owned(),
                    },
                    connection_fields: Vec::new(),
                },
            }])
        }

        async fn repositories(
            &self,
            _source_id: &str,
            refresh: RefreshMode,
        ) -> Result<Option<Vec<Repository>>, SourceDataFailure> {
            self.record(refresh);
            Ok(Some(vec![test_repository("one"), test_repository("two")]))
        }

        async fn workflows(
            &self,
            _source_id: &str,
            repository: &Repository,
            refresh: RefreshMode,
        ) -> Result<Vec<Workflow>, SourceDataFailure> {
            self.record(refresh);
            if repository.id == "two" {
                Err(SourceDataFailure::Source(
                    ConnectionValidationFailure::ProviderUnavailable,
                ))
            } else {
                Ok(Vec::new())
            }
        }

        async fn workflow_runs(
            &self,
            _source_id: &str,
            _repository: &Repository,
            refresh: RefreshMode,
        ) -> Result<WorkflowRunCollection, SourceDataFailure> {
            self.record(refresh);
            Ok(WorkflowRunCollection {
                runs: Vec::<WorkflowRun>::new(),
                last_attempted_at: 1,
                last_successful_at: 1,
                stale: false,
                error: None,
            })
        }

        async fn workflow_run_logs(
            &self,
            _source_id: &str,
            _repository: &Repository,
            _run: &WorkflowRun,
        ) -> Result<WorkflowRunLogs, crate::application::WorkflowRunLogsFailure> {
            unreachable!("synchronization does not load logs")
        }
    }

    fn test_repository(id: &str) -> Repository {
        Repository {
            id: id.to_owned(),
            owner: "owner".to_owned(),
            name: id.to_owned(),
            description: None,
            visibility: RepositoryVisibility::Private,
            web_url: format!("https://example.com/owner/{id}"),
        }
    }

    #[test]
    fn synchronization_permits_are_exclusive_per_source() {
        let active_sources = Arc::new(Mutex::new(HashSet::new()));
        let first =
            SourceSynchronizationPermit::acquire(active_sources.clone(), "source-a".to_owned())
                .expect("first synchronization starts");
        assert!(
            SourceSynchronizationPermit::acquire(active_sources.clone(), "source-a".to_owned(),)
                .is_none()
        );
        let other =
            SourceSynchronizationPermit::acquire(active_sources.clone(), "source-b".to_owned())
                .expect("another source can synchronize");

        drop(first);
        assert!(
            SourceSynchronizationPermit::acquire(active_sources, "source-a".to_owned()).is_some()
        );
        drop(other);
    }

    #[tokio::test]
    async fn synchronization_forces_each_selected_repository_and_continues_after_failure() {
        let source_data = Arc::new(RecordingSourceData {
            refresh_modes: Mutex::new(Vec::new()),
        });
        let selections = Arc::new(TestSelections(vec![
            RepositorySelection {
                source_id: "source".to_owned(),
                repository_id: "one".to_owned(),
            },
            RepositorySelection {
                source_id: "source".to_owned(),
                repository_id: "two".to_owned(),
            },
            RepositorySelection {
                source_id: "source".to_owned(),
                repository_id: "missing".to_owned(),
            },
        ]));
        let synchronize = SynchronizeSources::new(
            source_data.clone(),
            selections,
            NotifyRepositoryFailures::new(
                Arc::new(TestNotificationStates::default()),
                Arc::new(NoopNotificationSink),
            ),
        );
        assert_eq!(synchronize.status().last_completed_at, None);

        let summary = synchronize.execute().await.expect("synchronization runs");

        assert_eq!(summary.selected_repository_count, 3);
        assert_eq!(summary.synchronized_repository_count, 1);
        assert_eq!(summary.failed_repository_count, 2);
        assert_eq!(summary.skipped_repository_count, 0);
        assert!(synchronize.status().last_completed_at.is_some());
        assert!(
            source_data
                .refresh_modes
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .all(|refresh| *refresh == RefreshMode::Force)
        );
    }
}
