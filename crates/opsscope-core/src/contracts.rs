//! Data transfer objects shared by HTTP and desktop IPC.

use crate::application::{
    ConnectSourceFailure, ConnectionState, ConnectionValidationFailure, DiscoveredWorkflow,
    ListRepositoriesFailure, ListWorkflowsFailure, MAX_RECENT_RUNS_PER_WORKFLOW,
    MAX_SYNCHRONIZATION_INTERVAL_SECONDS, MIN_RECENT_RUNS_PER_WORKFLOW,
    MIN_SYNCHRONIZATION_INTERVAL_SECONDS, MonitoringSettings, RepositoryCatalog, RepositoryState,
    SaveRepositorySelectionFailure, SettingsFailure, SourceRepositorySelection, SourceState,
    SynchronizationFailure, SynchronizationStatus, SynchronizationSummary, WorkflowInventory,
    WorkflowRunLogsFailure,
};
use crate::domain::{
    RepositoryVisibility as DomainRepositoryVisibility, RunLifecycle as DomainRunLifecycle,
    RunOutcome as DomainRunOutcome, WorkflowRun as DomainWorkflowRun,
    WorkflowRunLogs as DomainWorkflowRunLogs, WorkflowState as DomainWorkflowState,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::{Config, TS};

pub const CONTRACT_VERSION: u8 = 10;
pub const APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const HEALTH_HTTP_PATH: &str = "/api/health";
pub const WORKFLOWS_HTTP_PATH: &str = "/api/workflows";
pub const WORKFLOW_RUN_LOGS_HTTP_PATH: &str = "/api/workflow-run-logs";
pub const SOURCES_HTTP_PATH: &str = "/api/sources";
pub const CONNECTIONS_HTTP_PATH: &str = "/api/connections";
pub const REPOSITORIES_HTTP_PATH: &str = "/api/repositories";
pub const REPOSITORY_SELECTIONS_HTTP_PATH: &str = "/api/repository-selections";
pub const SYNCHRONIZATION_HTTP_PATH: &str = "/api/sync";
pub const SETTINGS_HTTP_PATH: &str = "/api/settings";
pub const HEALTH_DESKTOP_COMMAND: &str = "health";
pub const LIST_WORKFLOWS_DESKTOP_COMMAND: &str = "list_workflows";
pub const WORKFLOW_RUN_LOGS_DESKTOP_COMMAND: &str = "workflow_run_logs";
pub const LIST_SOURCES_DESKTOP_COMMAND: &str = "list_sources";
pub const CONNECT_SOURCE_DESKTOP_COMMAND: &str = "connect_source";
pub const DISCONNECT_SOURCE_DESKTOP_COMMAND: &str = "disconnect_source";
pub const LIST_REPOSITORIES_DESKTOP_COMMAND: &str = "list_repositories";
pub const SAVE_REPOSITORY_SELECTION_DESKTOP_COMMAND: &str = "save_repository_selection";
pub const SYNCHRONIZE_SOURCES_DESKTOP_COMMAND: &str = "synchronize_sources";
pub const SYNCHRONIZATION_STATUS_DESKTOP_COMMAND: &str = "synchronization_status";
pub const GET_SETTINGS_DESKTOP_COMMAND: &str = "get_settings";
pub const UPDATE_SETTINGS_DESKTOP_COMMAND: &str = "update_settings";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub contract_version: u8,
}

impl HealthResponse {
    #[must_use]
    pub fn ready() -> Self {
        Self {
            status: "ok".to_owned(),
            service: "opsscope".to_owned(),
            contract_version: CONTRACT_VERSION,
        }
    }
}

/// A credential accepted for one validation attempt.
///
/// This request intentionally omits `Debug`, `Clone`, and `Serialize` in Rust.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectSourceRequest {
    pub source_id: String,
    pub connection_id: Option<String>,
    pub configuration: BTreeMap<String, String>,
    pub credential: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSummary {
    pub id: String,
    pub label: String,
    pub configuration: BTreeMap<String, String>,
    pub external_id: String,
    pub name: String,
    pub handle: Option<String>,
    pub profile_url: Option<String>,
    pub credential_stored: bool,
}

impl From<ConnectionState> for ConnectionSummary {
    fn from(connection: ConnectionState) -> Self {
        Self {
            id: connection.id,
            label: connection.label,
            configuration: connection.configuration,
            external_id: connection.account.external_id,
            name: connection.account.name,
            handle: connection.account.handle,
            profile_url: connection.account.profile_url,
            credential_stored: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CredentialFieldSummary {
    pub label: String,
    pub placeholder: String,
    pub help: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionFieldSummary {
    pub key: String,
    pub label: String,
    pub placeholder: String,
    pub help: String,
    pub default_value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub abbreviation: String,
    pub credential: CredentialFieldSummary,
    pub connection_fields: Vec<ConnectionFieldSummary>,
    pub connections: Vec<ConnectionSummary>,
}

impl From<SourceState> for SourceSummary {
    fn from(source: SourceState) -> Self {
        Self {
            id: source.descriptor.id,
            name: source.descriptor.name,
            description: source.descriptor.description,
            abbreviation: source.descriptor.abbreviation,
            credential: CredentialFieldSummary {
                label: source.descriptor.credential.label,
                placeholder: source.descriptor.credential.placeholder,
                help: source.descriptor.credential.help,
            },
            connection_fields: source
                .descriptor
                .connection_fields
                .into_iter()
                .map(|field| ConnectionFieldSummary {
                    key: field.key,
                    label: field.label,
                    placeholder: field.placeholder,
                    help: field.help,
                    default_value: field.default_value,
                })
                .collect(),
            connections: source
                .connections
                .into_iter()
                .map(ConnectionSummary::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListSourcesResponse {
    pub sources: Vec<SourceSummary>,
}

impl ListSourcesResponse {
    #[must_use]
    pub fn from_domain(sources: Vec<SourceState>) -> Self {
        Self {
            sources: sources.into_iter().map(SourceSummary::from).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryVisibility {
    Public,
    Private,
}

impl From<DomainRepositoryVisibility> for RepositoryVisibility {
    fn from(visibility: DomainRepositoryVisibility) -> Self {
        match visibility {
            DomainRepositoryVisibility::Public => Self::Public,
            DomainRepositoryVisibility::Private => Self::Private,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySummary {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub visibility: RepositoryVisibility,
    pub web_url: String,
    pub selected: bool,
}

impl From<RepositoryState> for RepositorySummary {
    fn from(state: RepositoryState) -> Self {
        let repository = state.repository;
        Self {
            id: repository.id,
            owner: repository.owner,
            name: repository.name,
            description: repository.description,
            visibility: repository.visibility.into(),
            web_url: repository.web_url,
            selected: state.selected,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySelectionSourceRequest {
    pub source_id: String,
    pub repository_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveRepositorySelectionRequest {
    pub sources: Vec<RepositorySelectionSourceRequest>,
}

impl SaveRepositorySelectionRequest {
    #[must_use]
    pub fn into_domain(self) -> Vec<SourceRepositorySelection> {
        self.sources
            .into_iter()
            .map(|source| SourceRepositorySelection {
                source_id: source.source_id,
                repository_ids: source.repository_ids,
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SaveRepositorySelectionResponse {
    pub selected_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySelectionErrorResponse {
    pub message: String,
}

impl From<SaveRepositorySelectionFailure> for RepositorySelectionErrorResponse {
    fn from(failure: SaveRepositorySelectionFailure) -> Self {
        let message = match failure {
            SaveRepositorySelectionFailure::InvalidSelection => {
                "The repository selection is invalid. Refresh and try again."
            }
            SaveRepositorySelectionFailure::SourceNotConnected => {
                "A selected source is no longer connected. Refresh and try again."
            }
            SaveRepositorySelectionFailure::StorageUnavailable => {
                "The repository selection could not be stored."
            }
        };
        Self {
            message: message.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySourceSummary {
    pub id: String,
    pub name: String,
    pub abbreviation: String,
    pub repositories: Vec<RepositorySummary>,
}

impl From<RepositoryCatalog> for RepositorySourceSummary {
    fn from(catalog: RepositoryCatalog) -> Self {
        Self {
            id: catalog.source.id,
            name: catalog.source.label,
            abbreviation: catalog.source.descriptor.abbreviation,
            repositories: catalog
                .repositories
                .into_iter()
                .map(RepositorySummary::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListRepositoriesResponse {
    pub sources: Vec<RepositorySourceSummary>,
}

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
    #[ts(type = "number")]
    pub attempt: u64,
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
    pub files: Vec<WorkflowRunLogFile>,
    pub truncated: bool,
}

impl From<DomainWorkflowRunLogs> for WorkflowRunLogsResponse {
    fn from(logs: DomainWorkflowRunLogs) -> Self {
        Self {
            files: logs
                .files
                .into_iter()
                .map(|file| WorkflowRunLogFile {
                    name: file.name,
                    content: file.content,
                })
                .collect(),
            truncated: logs.truncated,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SynchronizationResponse {
    pub selected_repository_count: usize,
    pub synchronized_repository_count: usize,
    pub failed_repository_count: usize,
    pub skipped_repository_count: usize,
    pub already_running: bool,
}

impl From<SynchronizationSummary> for SynchronizationResponse {
    fn from(summary: SynchronizationSummary) -> Self {
        Self {
            selected_repository_count: summary.selected_repository_count,
            synchronized_repository_count: summary.synchronized_repository_count,
            failed_repository_count: summary.failed_repository_count,
            skipped_repository_count: summary.skipped_repository_count,
            already_running: summary.already_running(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SynchronizationStatusResponse {
    pub running: bool,
    pub active_source_count: usize,
    #[ts(type = "number | null")]
    pub last_completed_at: Option<u64>,
    pub last_failed_repository_count: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringSettingsResponse {
    #[ts(type = "number")]
    pub synchronization_interval_seconds: u64,
    pub recent_runs_per_workflow: usize,
}

impl From<MonitoringSettings> for MonitoringSettingsResponse {
    fn from(settings: MonitoringSettings) -> Self {
        Self {
            synchronization_interval_seconds: settings.synchronization_interval_seconds,
            recent_runs_per_workflow: settings.recent_runs_per_workflow,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMonitoringSettingsRequest {
    #[ts(type = "number")]
    pub synchronization_interval_seconds: u64,
    pub recent_runs_per_workflow: usize,
}

impl From<UpdateMonitoringSettingsRequest> for MonitoringSettings {
    fn from(request: UpdateMonitoringSettingsRequest) -> Self {
        Self {
            synchronization_interval_seconds: request.synchronization_interval_seconds,
            recent_runs_per_workflow: request.recent_runs_per_workflow,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringSettingsErrorResponse {
    pub message: String,
}

impl From<SettingsFailure> for MonitoringSettingsErrorResponse {
    fn from(failure: SettingsFailure) -> Self {
        let message = match failure {
            SettingsFailure::InvalidSettings => {
                "The monitoring settings are outside the allowed range."
            }
            SettingsFailure::StorageUnavailable => {
                "The monitoring settings could not be loaded or saved."
            }
        };
        Self {
            message: message.to_owned(),
        }
    }
}

impl From<SynchronizationStatus> for SynchronizationStatusResponse {
    fn from(status: SynchronizationStatus) -> Self {
        Self {
            running: status.is_running(),
            active_source_count: status.active_source_count,
            last_completed_at: status.last_completed_at,
            last_failed_repository_count: status.last_failed_repository_count,
        }
    }
}

impl ListRepositoriesResponse {
    #[must_use]
    pub fn from_domain(catalogs: Vec<RepositoryCatalog>) -> Self {
        Self {
            sources: catalogs
                .into_iter()
                .map(RepositorySourceSummary::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectSourceRequest {
    pub connection_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectSourceResponse {
    pub disconnected: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionValidationErrorCode {
    InvalidConfiguration,
    InvalidCredentials,
    PermissionDenied,
    RateLimited,
    ProviderUnavailable,
    UnexpectedResponse,
    StorageUnavailable,
    UnknownSource,
    UnknownConnection,
    DuplicateConnection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionValidationErrorResponse {
    pub code: ConnectionValidationErrorCode,
    pub message: String,
}

impl From<ConnectionValidationFailure> for ConnectionValidationErrorResponse {
    fn from(failure: ConnectionValidationFailure) -> Self {
        let (code, message) = match failure {
            ConnectionValidationFailure::InvalidConfiguration => (
                ConnectionValidationErrorCode::InvalidConfiguration,
                "The source configuration is invalid. Check the server URL and try again.",
            ),
            ConnectionValidationFailure::InvalidCredentials => (
                ConnectionValidationErrorCode::InvalidCredentials,
                "The source rejected this credential. Check it and try again.",
            ),
            ConnectionValidationFailure::PermissionDenied => (
                ConnectionValidationErrorCode::PermissionDenied,
                "The source credential does not have permission to read workflow metadata.",
            ),
            ConnectionValidationFailure::RateLimited => (
                ConnectionValidationErrorCode::RateLimited,
                "The source rate limit was reached. Try again later.",
            ),
            ConnectionValidationFailure::ProviderUnavailable => (
                ConnectionValidationErrorCode::ProviderUnavailable,
                "The source could not be reached. Try again.",
            ),
            ConnectionValidationFailure::UnexpectedResponse => (
                ConnectionValidationErrorCode::UnexpectedResponse,
                "The source returned an unexpected response.",
            ),
        };

        Self {
            code,
            message: message.to_owned(),
        }
    }
}

impl From<ConnectSourceFailure> for ConnectionValidationErrorResponse {
    fn from(failure: ConnectSourceFailure) -> Self {
        match failure {
            ConnectSourceFailure::UnknownSource => Self {
                code: ConnectionValidationErrorCode::UnknownSource,
                message: "This source module is not registered.".to_owned(),
            },
            ConnectSourceFailure::UnknownConnection => Self {
                code: ConnectionValidationErrorCode::UnknownConnection,
                message: "This source connection no longer exists. Refresh and try again."
                    .to_owned(),
            },
            ConnectSourceFailure::DuplicateConnection => Self {
                code: ConnectionValidationErrorCode::DuplicateConnection,
                message: "A connection for this source and server already exists.".to_owned(),
            },
            ConnectSourceFailure::Validation(failure) => failure.into(),
            ConnectSourceFailure::StorageUnavailable => Self {
                code: ConnectionValidationErrorCode::StorageUnavailable,
                message: "The connection was validated but could not be stored securely."
                    .to_owned(),
            },
        }
    }
}

impl From<ListRepositoriesFailure> for ConnectionValidationErrorResponse {
    fn from(failure: ListRepositoriesFailure) -> Self {
        match failure {
            ListRepositoriesFailure::Source(failure) => failure.into(),
            ListRepositoriesFailure::StorageUnavailable => Self {
                code: ConnectionValidationErrorCode::StorageUnavailable,
                message: "The stored connection credential could not be read securely.".to_owned(),
            },
        }
    }
}

impl From<ListWorkflowsFailure> for ConnectionValidationErrorResponse {
    fn from(failure: ListWorkflowsFailure) -> Self {
        match failure {
            ListWorkflowsFailure::Source(failure) => failure.into(),
            ListWorkflowsFailure::StorageUnavailable => Self {
                code: ConnectionValidationErrorCode::StorageUnavailable,
                message: "The selected repositories or stored credential could not be read."
                    .to_owned(),
            },
        }
    }
}

impl From<SynchronizationFailure> for ConnectionValidationErrorResponse {
    fn from(_failure: SynchronizationFailure) -> Self {
        Self {
            code: ConnectionValidationErrorCode::StorageUnavailable,
            message: "The selected repositories could not be read for synchronization.".to_owned(),
        }
    }
}

#[must_use]
pub fn render_typescript_contract() -> String {
    let config = Config::default();
    let declarations = [
        HealthResponse::decl(&config),
        ConnectSourceRequest::decl(&config),
        ConnectionSummary::decl(&config),
        CredentialFieldSummary::decl(&config),
        ConnectionFieldSummary::decl(&config),
        SourceSummary::decl(&config),
        ListSourcesResponse::decl(&config),
        DisconnectSourceRequest::decl(&config),
        DisconnectSourceResponse::decl(&config),
        ConnectionValidationErrorCode::decl(&config),
        ConnectionValidationErrorResponse::decl(&config),
        RepositoryVisibility::decl(&config),
        RepositorySummary::decl(&config),
        RepositorySourceSummary::decl(&config),
        ListRepositoriesResponse::decl(&config),
        WorkflowState::decl(&config),
        RunLifecycle::decl(&config),
        RunOutcome::decl(&config),
        WorkflowRunSummary::decl(&config),
        WorkflowRunLogsRequest::decl(&config),
        WorkflowRunLogFile::decl(&config),
        WorkflowRunLogsResponse::decl(&config),
        WorkflowRunLogsErrorResponse::decl(&config),
        WorkflowSummary::decl(&config),
        ListWorkflowsResponse::decl(&config),
        SynchronizationResponse::decl(&config),
        SynchronizationStatusResponse::decl(&config),
        MonitoringSettingsResponse::decl(&config),
        UpdateMonitoringSettingsRequest::decl(&config),
        MonitoringSettingsErrorResponse::decl(&config),
        RepositorySelectionSourceRequest::decl(&config),
        SaveRepositorySelectionRequest::decl(&config),
        SaveRepositorySelectionResponse::decl(&config),
        RepositorySelectionErrorResponse::decl(&config),
    ]
    .join("\n\nexport ");
    format!(
        "// Generated from crates/opsscope-core/src/contracts.rs. Do not edit.\n\nexport {declarations}\n\nexport const applicationVersion = \"{APPLICATION_VERSION}\" as const;\n\nexport const settingsLimits = {{\n  synchronizationIntervalSeconds: {{ min: {MIN_SYNCHRONIZATION_INTERVAL_SECONDS}, max: {MAX_SYNCHRONIZATION_INTERVAL_SECONDS} }},\n  recentRunsPerWorkflow: {{ min: {MIN_RECENT_RUNS_PER_WORKFLOW}, max: {MAX_RECENT_RUNS_PER_WORKFLOW} }},\n}} as const;\n\nexport const httpRoutes = {{\n  health: \"{HEALTH_HTTP_PATH}\",\n  workflows: \"{WORKFLOWS_HTTP_PATH}\",\n  workflowRunLogs: \"{WORKFLOW_RUN_LOGS_HTTP_PATH}\",\n  synchronization: \"{SYNCHRONIZATION_HTTP_PATH}\",\n  settings: \"{SETTINGS_HTTP_PATH}\",\n  sources: \"{SOURCES_HTTP_PATH}\",\n  connections: \"{CONNECTIONS_HTTP_PATH}\",\n  repositories: \"{REPOSITORIES_HTTP_PATH}\",\n  repositorySelections: \"{REPOSITORY_SELECTIONS_HTTP_PATH}\",\n}} as const;\n\nexport const desktopCommands = {{\n  health: \"{HEALTH_DESKTOP_COMMAND}\",\n  listWorkflows: \"{LIST_WORKFLOWS_DESKTOP_COMMAND}\",\n  workflowRunLogs: \"{WORKFLOW_RUN_LOGS_DESKTOP_COMMAND}\",\n  synchronizeSources: \"{SYNCHRONIZE_SOURCES_DESKTOP_COMMAND}\",\n  synchronizationStatus: \"{SYNCHRONIZATION_STATUS_DESKTOP_COMMAND}\",\n  getSettings: \"{GET_SETTINGS_DESKTOP_COMMAND}\",\n  updateSettings: \"{UPDATE_SETTINGS_DESKTOP_COMMAND}\",\n  listSources: \"{LIST_SOURCES_DESKTOP_COMMAND}\",\n  connectSource: \"{CONNECT_SOURCE_DESKTOP_COMMAND}\",\n  disconnectSource: \"{DISCONNECT_SOURCE_DESKTOP_COMMAND}\",\n  listRepositories: \"{LIST_REPOSITORIES_DESKTOP_COMMAND}\",\n  saveRepositorySelection: \"{SAVE_REPOSITORY_SELECTION_DESKTOP_COMMAND}\",\n}} as const;\n\nexport interface ApplicationClient {{\n  health(): Promise<HealthResponse>;\n  listWorkflows(): Promise<ListWorkflowsResponse>;\n  workflowRunLogs(request: WorkflowRunLogsRequest): Promise<WorkflowRunLogsResponse>;\n  synchronizeSources(): Promise<SynchronizationResponse>;\n  synchronizationStatus(): Promise<SynchronizationStatusResponse>;\n  getSettings(): Promise<MonitoringSettingsResponse>;\n  updateSettings(request: UpdateMonitoringSettingsRequest): Promise<MonitoringSettingsResponse>;\n  listSources(): Promise<ListSourcesResponse>;\n  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary>;\n  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse>;\n  listRepositories(): Promise<ListRepositoriesResponse>;\n  saveRepositorySelection(request: SaveRepositorySelectionRequest): Promise<SaveRepositorySelectionResponse>;\n}}\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{ConnectedSource, SourceDescriptor, ValidatedAccount};

    #[test]
    fn maps_domain_values_without_exposing_infrastructure() {
        let response = WorkflowSummary::from(DiscoveredWorkflow {
            source: ConnectedSource {
                id: "source".to_owned(),
                label: "Source instance".to_owned(),
                descriptor: SourceDescriptor {
                    id: "module".to_owned(),
                    name: "Source".to_owned(),
                    description: "Source description".to_owned(),
                    abbreviation: "SO".to_owned(),
                    credential: crate::application::CredentialField {
                        label: "Token".to_owned(),
                        placeholder: "token".to_owned(),
                        help: "Help".to_owned(),
                    },
                    connection_fields: Vec::new(),
                },
            },
            repository: crate::domain::Repository {
                id: "repository".to_owned(),
                owner: "owner".to_owned(),
                name: "project".to_owned(),
                description: None,
                visibility: DomainRepositoryVisibility::Private,
                web_url: "https://example.com/owner/project".to_owned(),
            },
            workflow: crate::domain::Workflow {
                id: "workflow".to_owned(),
                name: "Build".to_owned(),
                path: ".ci/build.yml".to_owned(),
                state: DomainWorkflowState::Active,
                web_url: "https://example.com/owner/project/workflows/build".to_owned(),
            },
            runs: Vec::new(),
        });

        assert_eq!(response.source_id, "source");
        assert_eq!(response.repository_name, "project");
        assert_eq!(response.state, WorkflowState::Active);
    }

    #[test]
    fn validation_response_contains_identity_and_status_but_no_secret() {
        let response = ConnectionSummary::from(ConnectionState {
            id: "connection".to_owned(),
            label: "Example".to_owned(),
            configuration: BTreeMap::new(),
            account: ValidatedAccount {
                external_id: "42".to_owned(),
                name: "The Octocat".to_owned(),
                handle: Some("octocat".to_owned()),
                profile_url: Some("https://example.com/octocat".to_owned()),
            },
        });
        let serialized = serde_json::to_string(&response).expect("response serializes");

        assert!(serialized.contains("octocat"));
        assert!(!serialized.contains("token"));
        assert!(!serialized.contains("secret"));
    }
}
