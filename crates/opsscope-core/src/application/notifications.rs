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
mod tests;
