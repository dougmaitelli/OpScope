use crate::domain::{Repository, RunLifecycle, RunOutcome, Workflow, WorkflowRun};
use async_trait::async_trait;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NotificationSeverity {
    Failure,
    Info,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notification {
    pub repository: Repository,
    pub events: NotificationEvents,
    pub severity: NotificationSeverity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationEvents {
    WorkItems(Vec<WorkItemEvent>),
    WorkflowFailures(Vec<WorkflowFailureEvent>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkItemEvent {
    pub kind: WorkItemKind,
    pub transition: WorkItemTransition,
    pub number: u64,
    pub title: String,
    pub url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkItemKind {
    PullRequest,
    Issue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkItemTransition {
    Opened,
    ReviewRequested,
    ChangesRequested,
    Merged,
    Closed,
    Assigned,
    Reopened,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowFailureEvent {
    pub workflow_id: String,
    pub name: String,
    pub run_id: Option<String>,
    pub attempt: Option<u64>,
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
pub enum NotificationProcessingFailure {
    StorageUnavailable,
    DeliveryUnavailable,
}

impl Display for NotificationProcessingFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::StorageUnavailable => "notification state storage unavailable",
            Self::DeliveryUnavailable => "notification delivery unavailable",
        })
    }
}

impl Error for NotificationProcessingFailure {}

#[derive(Clone)]
pub struct NotifyRepositoryFailures {
    settings: Option<Arc<dyn super::SettingsRepository>>,
    states: Arc<dyn NotificationStateRepository>,
    sink: Arc<dyn NotificationSink>,
}

impl NotifyRepositoryFailures {
    #[must_use]
    pub fn new(
        states: Arc<dyn NotificationStateRepository>,
        sink: Arc<dyn NotificationSink>,
    ) -> Self {
        Self {
            states,
            sink,
            settings: None,
        }
    }

    #[must_use]
    pub fn with_settings(mut self, settings: Arc<dyn super::SettingsRepository>) -> Self {
        self.settings = Some(settings);
        self
    }

    pub async fn observe(
        &self,
        account_id: &str,
        source_id: &str,
        repository: &Repository,
        workflows: &[Workflow],
        runs: &[WorkflowRun],
    ) -> Result<(), NotificationProcessingFailure> {
        let settings = self
            .settings
            .as_ref()
            .map(|settings| settings.load_settings())
            .transpose()
            .map_err(|_| NotificationProcessingFailure::StorageUnavailable)?
            .unwrap_or_default();
        let only_my_work = settings.only_my_work;
        let previous = self
            .states
            .load(source_id, &repository.id)
            .map_err(|_| NotificationProcessingFailure::StorageUnavailable)?;
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
                .filter(|state| {
                    !only_my_work
                        || runs.iter().any(|run| {
                            run.workflow_id == state.workflow_id
                                && Some(&run.id) == state.run_id.as_ref()
                                && Some(run.attempt) == state.attempt
                                && run.relationships.evaluate(account_id).matches()
                        })
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
            .map_err(|_| NotificationProcessingFailure::StorageUnavailable)?;

        let Some(newly_failed) = newly_failed.filter(|failures| !failures.is_empty()) else {
            return Ok(());
        };
        if !settings.notifications.workflow_failures {
            return Ok(());
        }
        let notification = failure_notification(repository, &newly_failed);
        self.sink
            .send(&notification)
            .await
            .map_err(|_| NotificationProcessingFailure::DeliveryUnavailable)
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
    Notification {
        repository: repository.clone(),
        events: NotificationEvents::WorkflowFailures(
            failures
                .iter()
                .map(|(workflow, state)| WorkflowFailureEvent {
                    workflow_id: workflow.id.clone(),
                    name: workflow.name.clone(),
                    run_id: state.run_id.clone(),
                    attempt: state.attempt,
                })
                .collect(),
        ),
        severity: NotificationSeverity::Failure,
    }
}

#[cfg(test)]
mod tests;
