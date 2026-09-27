use crate::domain::{Repository, RunLifecycle, RunOutcome, Workflow, WorkflowRun};
use async_trait::async_trait;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationSeverity {
    Failure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub severity: NotificationSeverity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotificationDeliveryFailure;

impl Display for NotificationDeliveryFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("notification delivery failed")
    }
}

impl Error for NotificationDeliveryFailure {}

#[async_trait]
pub trait NotificationSink: Send + Sync {
    async fn send(&self, notification: &Notification) -> Result<(), NotificationDeliveryFailure>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopNotificationSink;

#[async_trait]
impl NotificationSink for NoopNotificationSink {
    async fn send(&self, _notification: &Notification) -> Result<(), NotificationDeliveryFailure> {
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LatestRunNotificationState {
    pub workflow_id: String,
    pub run_id: Option<String>,
    pub attempt: Option<u64>,
    pub failed: bool,
}

pub trait NotificationStateRepository: Send + Sync {
    /// `None` means this repository has never been observed. `Some([])` is a
    /// valid baseline for a repository with no workflows.
    fn load(
        &self,
        source_id: &str,
        repository_id: &str,
    ) -> Result<Option<Vec<LatestRunNotificationState>>, super::PersistenceFailure>;

    fn replace(
        &self,
        source_id: &str,
        repository_id: &str,
        states: &[LatestRunNotificationState],
    ) -> Result<(), super::PersistenceFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotifyRepositoryFailuresFailure {
    StorageUnavailable,
    DeliveryUnavailable,
}

impl Display for NotifyRepositoryFailuresFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::StorageUnavailable => "notification state storage unavailable",
            Self::DeliveryUnavailable => "notification delivery unavailable",
        })
    }
}

impl Error for NotifyRepositoryFailuresFailure {}

#[derive(Clone)]
pub struct NotifyRepositoryFailures {
    states: Arc<dyn NotificationStateRepository>,
    sink: Arc<dyn NotificationSink>,
}

impl NotifyRepositoryFailures {
    #[must_use]
    pub fn new(
        states: Arc<dyn NotificationStateRepository>,
        sink: Arc<dyn NotificationSink>,
    ) -> Self {
        Self { states, sink }
    }

    pub async fn observe(
        &self,
        source_id: &str,
        repository: &Repository,
        workflows: &[Workflow],
        runs: &[WorkflowRun],
    ) -> Result<(), NotifyRepositoryFailuresFailure> {
        let previous = self
            .states
            .load(source_id, &repository.id)
            .map_err(|_| NotifyRepositoryFailuresFailure::StorageUnavailable)?;
        let current = latest_states(workflows, runs);
        let newly_failed = previous.as_ref().map(|previous| {
            let previous = previous
                .iter()
                .map(|state| (state.workflow_id.as_str(), state))
                .collect::<HashMap<_, _>>();
            current
                .iter()
                .filter(|state| {
                    state.failed
                        && previous.get(state.workflow_id.as_str()).copied() != Some(*state)
                })
                .filter_map(|state| {
                    let workflow = workflows
                        .iter()
                        .find(|workflow| workflow.id == state.workflow_id)?;
                    Some((workflow, state))
                })
                .collect::<Vec<_>>()
        });

        // Record the observation before invoking an external delivery adapter.
        // This provides at-most-once behavior if a delivery response is lost.
        self.states
            .replace(source_id, &repository.id, &current)
            .map_err(|_| NotifyRepositoryFailuresFailure::StorageUnavailable)?;

        let Some(newly_failed) = newly_failed.filter(|failures| !failures.is_empty()) else {
            return Ok(());
        };
        let notification = failure_notification(repository, &newly_failed);
        self.sink
            .send(&notification)
            .await
            .map_err(|_| NotifyRepositoryFailuresFailure::DeliveryUnavailable)
    }
}

fn latest_states(workflows: &[Workflow], runs: &[WorkflowRun]) -> Vec<LatestRunNotificationState> {
    let mut states = workflows
        .iter()
        .map(|workflow| LatestRunNotificationState {
            workflow_id: workflow.id.clone(),
            run_id: None,
            attempt: None,
            failed: false,
        })
        .collect::<Vec<_>>();

    for state in &mut states {
        let latest = runs
            .iter()
            .filter(|run| run.workflow_id == state.workflow_id)
            .max_by(|left, right| {
                (left.run_number, left.attempt, left.updated_at.as_str()).cmp(&(
                    right.run_number,
                    right.attempt,
                    right.updated_at.as_str(),
                ))
            });
        if let Some(run) = latest {
            state.run_id = Some(run.id.clone());
            state.attempt = Some(run.attempt);
            state.failed =
                run.lifecycle == RunLifecycle::Completed && run.outcome == RunOutcome::Failure;
        }
    }
    states.sort_by(|left, right| left.workflow_id.cmp(&right.workflow_id));
    states
}

fn failure_notification(
    repository: &Repository,
    failures: &[(&Workflow, &LatestRunNotificationState)],
) -> Notification {
    let repository_name = format!("{}/{}", repository.owner, repository.name);
    let title = if failures.len() == 1 {
        format!("Workflow failed in {repository_name}")
    } else {
        format!("{} workflows failed in {repository_name}", failures.len())
    };
    let body = failures
        .iter()
        .map(|(workflow, state)| match state.attempt {
            Some(attempt) if attempt > 1 => format!("{} (attempt {attempt})", workflow.name),
            _ => workflow.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    Notification {
        title,
        body,
        severity: NotificationSeverity::Failure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::PersistenceFailure;
    use crate::domain::{RepositoryVisibility, WorkflowState};
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryStates(Mutex<Option<Vec<LatestRunNotificationState>>>);

    impl NotificationStateRepository for MemoryStates {
        fn load(
            &self,
            _source_id: &str,
            _repository_id: &str,
        ) -> Result<Option<Vec<LatestRunNotificationState>>, PersistenceFailure> {
            Ok(self.0.lock().expect("state lock").clone())
        }

        fn replace(
            &self,
            _source_id: &str,
            _repository_id: &str,
            states: &[LatestRunNotificationState],
        ) -> Result<(), PersistenceFailure> {
            *self.0.lock().expect("state lock") = Some(states.to_vec());
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingSink(Mutex<Vec<Notification>>);

    #[async_trait]
    impl NotificationSink for RecordingSink {
        async fn send(
            &self,
            notification: &Notification,
        ) -> Result<(), NotificationDeliveryFailure> {
            self.0
                .lock()
                .expect("notification lock")
                .push(notification.clone());
            Ok(())
        }
    }

    fn repository() -> Repository {
        Repository {
            id: "repository-1".to_owned(),
            owner: "owner".to_owned(),
            name: "project".to_owned(),
            description: None,
            visibility: RepositoryVisibility::Private,
            web_url: "https://example.com/owner/project".to_owned(),
        }
    }

    fn workflows() -> Vec<Workflow> {
        [("workflow-1", "Build"), ("workflow-2", "Test")]
            .into_iter()
            .map(|(id, name)| Workflow {
                id: id.to_owned(),
                name: name.to_owned(),
                path: format!("{id}.yml"),
                state: WorkflowState::Active,
                web_url: format!("https://example.com/{id}"),
            })
            .collect()
    }

    fn run(workflow_id: &str, id: &str, number: u64, outcome: RunOutcome) -> WorkflowRun {
        WorkflowRun {
            id: id.to_owned(),
            workflow_id: workflow_id.to_owned(),
            run_number: number,
            attempt: 1,
            title: id.to_owned(),
            lifecycle: RunLifecycle::Completed,
            outcome,
            branch: Some("main".to_owned()),
            commit_sha: "abcdef123456".to_owned(),
            actor: None,
            trigger: "push".to_owned(),
            created_at: format!("2026-09-26T18:{number:02}:00Z"),
            started_at: None,
            updated_at: format!("2026-09-26T18:{number:02}:30Z"),
            web_url: format!("https://example.com/runs/{id}"),
            provider_status: "completed".to_owned(),
            provider_conclusion: None,
        }
    }

    #[tokio::test]
    async fn establishes_a_baseline_without_notifying_about_an_existing_failure() {
        let states = Arc::new(MemoryStates::default());
        let sink = Arc::new(RecordingSink::default());
        let notifier = NotifyRepositoryFailures::new(states, sink.clone());

        notifier
            .observe(
                "source",
                &repository(),
                &workflows(),
                &[run("workflow-1", "run-1", 1, RunOutcome::Failure)],
            )
            .await
            .expect("baseline succeeds");

        assert!(sink.0.lock().expect("notification lock").is_empty());
    }

    #[tokio::test]
    async fn groups_new_latest_failures_once_per_repository() {
        let states = Arc::new(MemoryStates::default());
        let sink = Arc::new(RecordingSink::default());
        let notifier = NotifyRepositoryFailures::new(states, sink.clone());
        let repository = repository();
        let workflows = workflows();
        notifier
            .observe("source", &repository, &workflows, &[])
            .await
            .expect("baseline succeeds");
        let failed_runs = vec![
            run("workflow-1", "run-1", 1, RunOutcome::Failure),
            run("workflow-2", "run-2", 2, RunOutcome::Failure),
        ];

        notifier
            .observe("source", &repository, &workflows, &failed_runs)
            .await
            .expect("notification succeeds");
        notifier
            .observe("source", &repository, &workflows, &failed_runs)
            .await
            .expect("repeated observation succeeds");

        let sent = sink.0.lock().expect("notification lock");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].title, "2 workflows failed in owner/project");
        assert_eq!(sent[0].body, "Build, Test");
        assert_eq!(sent[0].severity, NotificationSeverity::Failure);
    }

    #[tokio::test]
    async fn notifies_when_a_new_latest_run_fails_after_an_earlier_failure() {
        let states = Arc::new(MemoryStates::default());
        let sink = Arc::new(RecordingSink::default());
        let notifier = NotifyRepositoryFailures::new(states, sink.clone());
        let repository = repository();
        let workflows = workflows();
        notifier
            .observe("source", &repository, &workflows, &[])
            .await
            .expect("baseline succeeds");

        for (id, number) in [("run-1", 1), ("run-2", 2)] {
            notifier
                .observe(
                    "source",
                    &repository,
                    &workflows,
                    &[run("workflow-1", id, number, RunOutcome::Failure)],
                )
                .await
                .expect("notification succeeds");
        }

        assert_eq!(sink.0.lock().expect("notification lock").len(), 2);
    }

    #[tokio::test]
    async fn ignores_failed_history_and_notifies_when_the_latest_run_enters_failure() {
        let states = Arc::new(MemoryStates::default());
        let sink = Arc::new(RecordingSink::default());
        let notifier = NotifyRepositoryFailures::new(states, sink.clone());
        let repository = repository();
        let workflows = workflows();
        let mut latest = run("workflow-1", "run-2", 2, RunOutcome::Unknown);
        latest.lifecycle = RunLifecycle::Running;
        let history = run("workflow-1", "run-1", 1, RunOutcome::Failure);

        notifier
            .observe(
                "source",
                &repository,
                &workflows,
                &[history.clone(), latest.clone()],
            )
            .await
            .expect("baseline succeeds");
        notifier
            .observe(
                "source",
                &repository,
                &workflows,
                &[history.clone(), latest],
            )
            .await
            .expect("unchanged latest run succeeds");
        assert!(sink.0.lock().expect("notification lock").is_empty());

        notifier
            .observe(
                "source",
                &repository,
                &workflows,
                &[history, run("workflow-1", "run-2", 2, RunOutcome::Failure)],
            )
            .await
            .expect("failure transition succeeds");

        assert_eq!(sink.0.lock().expect("notification lock").len(), 1);
    }
}
