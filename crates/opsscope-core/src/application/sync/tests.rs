use super::*;
use crate::application::{
    ActivityEventRepository, ChangeRequestActivityEvent, ConnectedSource,
    ConnectionValidationFailure, CredentialField, LatestRunNotificationState, NoopNotificationSink,
    NotificationStateRepository, PersistenceFailure, RepositorySelection, SourceCapability,
    SourceDescriptor, SourceRepositorySelection, TrackChangeRequestActivity,
};
use crate::domain::{
    ChangeRequest, ChangeRequestDetails, Repository, RepositoryVisibility, Workflow, WorkflowRun,
    WorkflowRunLogs,
};
use crate::source_data::{SourceDataFailure, WorkflowRunCollection};
use async_trait::async_trait;

struct TestSelections(Vec<RepositorySelection>);

struct TestActivityEvents;

impl ActivityEventRepository for TestActivityEvents {
    fn load_change_request_state(
        &self,
        _source_id: &str,
        _repository_id: &str,
    ) -> Result<Option<Vec<ChangeRequest>>, PersistenceFailure> {
        Ok(None)
    }

    fn save_change_request_observation(
        &self,
        _source_id: &str,
        _repository_id: &str,
        _observed: &[ChangeRequest],
        _events: &[ChangeRequestActivityEvent],
    ) -> Result<(), PersistenceFailure> {
        Ok(())
    }

    fn list_change_request_events(
        &self,
    ) -> Result<Vec<ChangeRequestActivityEvent>, PersistenceFailure> {
        Ok(Vec::new())
    }
}

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
            account_id: "42".into(),
            id: "source".to_owned(),
            label: "Source".to_owned(),
            descriptor: SourceDescriptor {
                id: "module".to_owned(),
                name: "Source".to_owned(),
                description: "Test source".to_owned(),
                abbreviation: "SO".to_owned(),
                capabilities: vec![SourceCapability::Workflows],
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

    async fn change_requests(
        &self,
        _source_id: &str,
        _repository: &Repository,
        refresh: RefreshMode,
    ) -> Result<Option<Vec<ChangeRequest>>, SourceDataFailure> {
        self.record(refresh);
        Ok(None)
    }

    async fn change_request_details(
        &self,
        _source_id: &str,
        _repository: &Repository,
        _number: u64,
        refresh: RefreshMode,
    ) -> Result<Option<ChangeRequestDetails>, SourceDataFailure> {
        self.record(refresh);
        Ok(None)
    }

    async fn workflow_run(
        &self,
        _source_id: &str,
        _repository: &Repository,
        _run_id: &str,
    ) -> Result<Option<WorkflowRun>, SourceDataFailure> {
        unreachable!("synchronization does not load individual runs")
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
    let first = SourceSynchronizationPermit::acquire(active_sources.clone(), "source-a".to_owned())
        .expect("first synchronization starts");
    assert!(
        SourceSynchronizationPermit::acquire(active_sources.clone(), "source-a".to_owned(),)
            .is_none()
    );
    let other = SourceSynchronizationPermit::acquire(active_sources.clone(), "source-b".to_owned())
        .expect("another source can synchronize");
    assert!(
        SourceSynchronizationPermit::acquire(active_sources.clone(), "source-a".to_owned())
            .is_none()
    );
    assert_eq!(lock_active_sources(&active_sources).len(), 2);

    drop(first);
    assert!(SourceSynchronizationPermit::acquire(active_sources, "source-a".to_owned()).is_some());
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
        TrackChangeRequestActivity::new(Arc::new(TestActivityEvents)),
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
