use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowState {
    Active,
    Disabled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RunLifecycle {
    Queued,
    Running,
    Completed,
    Unknown,
}

impl From<DomainRunLifecycle> for RunLifecycle {
    fn from(lifecycle: DomainRunLifecycle) -> Self {
        match lifecycle {
            DomainRunLifecycle::Queued => Self::Queued,
            DomainRunLifecycle::Running => Self::Running,
            DomainRunLifecycle::Completed => Self::Completed,
            DomainRunLifecycle::Unknown => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RunOutcome {
    Success,
    Warning,
    Failure,
    Cancelled,
    Skipped,
    Unknown,
}

impl From<DomainRunOutcome> for RunOutcome {
    fn from(outcome: DomainRunOutcome) -> Self {
        match outcome {
            DomainRunOutcome::Success => Self::Success,
            DomainRunOutcome::Warning => Self::Warning,
            DomainRunOutcome::Failure => Self::Failure,
            DomainRunOutcome::Cancelled => Self::Cancelled,
            DomainRunOutcome::Skipped => Self::Skipped,
            DomainRunOutcome::Unknown => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunSummary {
    pub id: String,
    #[ts(type = "number")]
    pub run_number: u64,
    #[ts(type = "number")]
    pub attempt: u64,
    pub title: String,
    pub lifecycle: RunLifecycle,
    pub outcome: RunOutcome,
    pub branch: Option<String>,
    pub commit_sha: String,
    pub actor: Option<String>,
    pub trigger: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub web_url: String,
}

impl From<DomainWorkflowRun> for WorkflowRunSummary {
    fn from(run: DomainWorkflowRun) -> Self {
        Self {
            id: run.id,
            run_number: run.run_number,
            attempt: run.attempt,
            title: run.title,
            lifecycle: run.lifecycle.into(),
            outcome: run.outcome.into(),
            branch: run.branch,
            commit_sha: run.commit_sha,
            actor: run.actor,
            trigger: run.trigger,
            created_at: run.created_at,
            started_at: run.started_at,
            updated_at: run.updated_at,
            web_url: run.web_url,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunLogsRequest {
    pub source_id: String,
    pub repository_id: String,
    pub run_id: String,
    #[ts(type = "number | null")]
    pub attempt: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunLogFile {
    pub name: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunLogsResponse {
    pub run: WorkflowRunSummary,
    pub files: Vec<WorkflowRunLogFile>,
    pub truncated: bool,
}

impl From<ResolvedWorkflowRunLogs> for WorkflowRunLogsResponse {
    fn from(resolved: ResolvedWorkflowRunLogs) -> Self {
        Self {
            run: resolved.run.into(),
            files: resolved
                .logs
                .files
                .into_iter()
                .map(|file| WorkflowRunLogFile {
                    name: file.name,
                    content: file.content,
                })
                .collect(),
            truncated: resolved.logs.truncated,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunLogsErrorResponse {
    pub message: String,
}

impl From<WorkflowRunLogsFailure> for WorkflowRunLogsErrorResponse {
    fn from(failure: WorkflowRunLogsFailure) -> Self {
        let message = match failure {
            WorkflowRunLogsFailure::UnknownSource => "This source module is not registered.",
            WorkflowRunLogsFailure::SourceNotConnected => "This source is no longer connected.",
            WorkflowRunLogsFailure::RepositoryNotFound => {
                "This repository is no longer available. Refresh the dashboard."
            }
            WorkflowRunLogsFailure::RunNotFound => {
                "This workflow run is no longer available. Refresh the dashboard."
            }
            WorkflowRunLogsFailure::LogsUnavailable => {
                "Logs are not available for this workflow run yet."
            }
            WorkflowRunLogsFailure::LogsTooLarge => {
                "These logs are too large to display in the application."
            }
            WorkflowRunLogsFailure::InvalidCredentials => {
                "The source rejected the stored credential."
            }
            WorkflowRunLogsFailure::PermissionDenied => {
                "The source credential does not have permission to read workflow logs."
            }
            WorkflowRunLogsFailure::RateLimited => {
                "The source rate limit was reached. Try again later."
            }
            WorkflowRunLogsFailure::ProviderUnavailable => {
                "The source could not be reached. Try again."
            }
            WorkflowRunLogsFailure::UnexpectedResponse => {
                "The source returned workflow logs in an unexpected format."
            }
            WorkflowRunLogsFailure::StorageUnavailable => {
                "The stored connection credential could not be read securely."
            }
        };
        Self {
            message: message.to_owned(),
        }
    }
}

impl From<DomainWorkflowState> for WorkflowState {
    fn from(state: DomainWorkflowState) -> Self {
        match state {
            DomainWorkflowState::Active => Self::Active,
            DomainWorkflowState::Disabled => Self::Disabled,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub state: WorkflowState,
    pub web_url: String,
    pub source_id: String,
    pub source_name: String,
    pub source_abbreviation: String,
    pub repository_id: String,
    pub repository_owner: String,
    pub repository_name: String,
    pub runs: Vec<WorkflowRunSummary>,
}

impl From<DiscoveredWorkflow> for WorkflowSummary {
    fn from(discovered: DiscoveredWorkflow) -> Self {
        Self {
            id: discovered.workflow.id,
            name: discovered.workflow.name,
            path: discovered.workflow.path,
            state: discovered.workflow.state.into(),
            web_url: discovered.workflow.web_url,
            source_id: discovered.source.id,
            source_name: discovered.source.label,
            source_abbreviation: discovered.source.descriptor.abbreviation,
            repository_id: discovered.repository.id,
            repository_owner: discovered.repository.owner,
            repository_name: discovered.repository.name,
            runs: discovered
                .runs
                .into_iter()
                .map(WorkflowRunSummary::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkflowsResponse {
    pub selected_repository_count: usize,
    pub workflows: Vec<WorkflowSummary>,
    #[ts(type = "number | null")]
    pub last_attempted_at: Option<u64>,
    #[ts(type = "number | null")]
    pub last_successful_at: Option<u64>,
    pub stale: bool,
    pub sync_error: Option<String>,
}

impl ListWorkflowsResponse {
    #[must_use]
    pub fn from_domain(inventory: WorkflowInventory) -> Self {
        Self {
            selected_repository_count: inventory.selected_repository_count,
            last_attempted_at: inventory.last_attempted_at,
            last_successful_at: inventory.last_successful_at,
            stale: inventory.stale,
            sync_error: inventory.sync_error.map(|failure| failure.to_string()),
            workflows: inventory
                .workflows
                .into_iter()
                .map(WorkflowSummary::from)
                .collect(),
        }
    }
}
