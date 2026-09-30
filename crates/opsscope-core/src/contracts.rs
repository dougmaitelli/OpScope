//! Data transfer objects shared by HTTP and desktop IPC.

mod activity;
mod change_requests;
mod issues;
mod notification_preferences;
pub use notification_preferences::NotificationPreferencesContract;
mod repositories;
mod sources;
mod workflows;
pub use activity::*;
pub use change_requests::*;
pub use repositories::*;
pub use sources::*;
pub use workflows::*;

pub use issues::{
    IssueCommentSummary, IssueDetailsErrorResponse, IssueDetailsRequest, IssueDetailsResponse,
    IssueState, IssueSummary, ListIssuesResponse,
};

use crate::application::{
    ChangeRequestActivityEvent as DomainChangeRequestActivityEvent,
    ChangeRequestActivityKind as DomainChangeRequestActivityKind, ChangeRequestInventory,
    ConnectSourceFailure, ConnectionState, ConnectionValidationFailure, DiscoveredChangeRequest,
    DiscoveredWorkflow, GetChangeRequestDetailsFailure, ListChangeRequestsFailure,
    ListIssuesFailure, ListRepositoriesFailure, ListWorkflowsFailure, MAX_RECENT_RUNS_PER_WORKFLOW,
    MAX_SYNCHRONIZATION_INTERVAL_SECONDS, MIN_RECENT_RUNS_PER_WORKFLOW,
    MIN_SYNCHRONIZATION_INTERVAL_SECONDS, MonitoringSettings, ReleaseUpdate, RepositoryCatalog,
    RepositoryState, ResolvedWorkflowRunLogs, SaveRepositorySelectionFailure, SettingsFailure,
    SourceCapability as DomainSourceCapability, SourceRepositorySelection, SourceState,
    SynchronizationFailure, SynchronizationStatus, SynchronizationSummary, WorkflowInventory,
    WorkflowRunLogsFailure,
};
use crate::domain::{
    ChangeRequest as DomainChangeRequest,
    ChangeRequestCheckStatus as DomainChangeRequestCheckStatus,
    ChangeRequestDetails as DomainChangeRequestDetails,
    ChangeRequestMergeStatus as DomainChangeRequestMergeStatus,
    ChangeRequestReviewStatus as DomainChangeRequestReviewStatus,
    ChangeRequestState as DomainChangeRequestState,
    RepositoryVisibility as DomainRepositoryVisibility, RunLifecycle as DomainRunLifecycle,
    RunOutcome as DomainRunOutcome, WorkflowRun as DomainWorkflowRun,
    WorkflowState as DomainWorkflowState,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::{Config, TS};

pub const CONTRACT_VERSION: u8 = 18;
pub const APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const HEALTH_HTTP_PATH: &str = "/api/health";
pub const UPDATE_STATUS_HTTP_PATH: &str = "/api/update-status";
pub const WORKFLOWS_HTTP_PATH: &str = "/api/workflows";
pub const ACTIVITY_HTTP_PATH: &str = "/api/activity";
pub const CHANGE_REQUESTS_HTTP_PATH: &str = "/api/change-requests";
pub const CHANGE_REQUEST_DETAILS_HTTP_PATH: &str = "/api/change-request-details";
pub const ISSUES_HTTP_PATH: &str = "/api/issues";
pub const ISSUE_DETAILS_HTTP_PATH: &str = "/api/issue-details";
pub const WORKFLOW_RUN_LOGS_HTTP_PATH: &str = "/api/workflow-run-logs";
pub const SOURCES_HTTP_PATH: &str = "/api/sources";
pub const CONNECTIONS_HTTP_PATH: &str = "/api/connections";
pub const REPOSITORIES_HTTP_PATH: &str = "/api/repositories";
pub const REPOSITORY_SELECTIONS_HTTP_PATH: &str = "/api/repository-selections";
pub const SYNCHRONIZATION_HTTP_PATH: &str = "/api/sync";
pub const SETTINGS_HTTP_PATH: &str = "/api/settings";
pub const HEALTH_DESKTOP_COMMAND: &str = "health";
pub const UPDATE_STATUS_DESKTOP_COMMAND: &str = "update_status";
pub const LIST_WORKFLOWS_DESKTOP_COMMAND: &str = "list_workflows";
pub const LIST_ACTIVITY_DESKTOP_COMMAND: &str = "list_activity";
pub const LIST_CHANGE_REQUESTS_DESKTOP_COMMAND: &str = "list_change_requests";
pub const CHANGE_REQUEST_DETAILS_DESKTOP_COMMAND: &str = "change_request_details";
pub const LIST_ISSUES_DESKTOP_COMMAND: &str = "list_issues";
pub const ISSUE_DETAILS_DESKTOP_COMMAND: &str = "issue_details";
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatusResponse {
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub update_available: bool,
}

impl From<ReleaseUpdate> for UpdateStatusResponse {
    fn from(update: ReleaseUpdate) -> Self {
        Self {
            current_version: update.current_version,
            latest_version: update.latest_version,
            release_url: update.release_url,
            update_available: update.update_available,
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
    pub pull_requests_enabled: bool,
    pub issues_enabled: bool,
    pub notifications: NotificationPreferencesContract,
    pub only_my_work: bool,
    #[ts(type = "number")]
    pub synchronization_interval_seconds: u64,
    pub recent_runs_per_workflow: usize,
}

impl From<MonitoringSettings> for MonitoringSettingsResponse {
    fn from(settings: MonitoringSettings) -> Self {
        Self {
            pull_requests_enabled: settings.pull_requests_enabled,
            issues_enabled: settings.issues_enabled,
            notifications: settings.notifications.into(),
            only_my_work: settings.only_my_work,
            synchronization_interval_seconds: settings.synchronization_interval_seconds,
            recent_runs_per_workflow: settings.recent_runs_per_workflow,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMonitoringSettingsRequest {
    #[serde(default = "enabled_by_default")]
    pub pull_requests_enabled: bool,
    #[serde(default = "enabled_by_default")]
    pub issues_enabled: bool,
    #[serde(default)]
    pub notifications: NotificationPreferencesContract,
    #[serde(default)]
    pub only_my_work: bool,
    #[ts(type = "number")]
    pub synchronization_interval_seconds: u64,
    pub recent_runs_per_workflow: usize,
}

impl From<UpdateMonitoringSettingsRequest> for MonitoringSettings {
    fn from(request: UpdateMonitoringSettingsRequest) -> Self {
        Self {
            pull_requests_enabled: request.pull_requests_enabled,
            issues_enabled: request.issues_enabled,
            notifications: request.notifications.into(),
            only_my_work: request.only_my_work,
            synchronization_interval_seconds: request.synchronization_interval_seconds,
            recent_runs_per_workflow: request.recent_runs_per_workflow,
        }
    }
}

const fn enabled_by_default() -> bool {
    true
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

impl From<ListChangeRequestsFailure> for ConnectionValidationErrorResponse {
    fn from(failure: ListChangeRequestsFailure) -> Self {
        match failure {
            ListChangeRequestsFailure::Source(failure) => failure.into(),
            ListChangeRequestsFailure::StorageUnavailable => Self {
                code: ConnectionValidationErrorCode::StorageUnavailable,
                message: "The cached change requests could not be read.".to_owned(),
            },
        }
    }
}

impl From<ListIssuesFailure> for ConnectionValidationErrorResponse {
    fn from(failure: ListIssuesFailure) -> Self {
        match failure {
            ListIssuesFailure::Source(ConnectionValidationFailure::PermissionDenied) => Self {
                code: ConnectionValidationErrorCode::PermissionDenied,
                message: "The source credential does not have permission to read issues."
                    .to_owned(),
            },
            ListIssuesFailure::Source(failure) => failure.into(),
            ListIssuesFailure::StorageUnavailable => Self {
                code: ConnectionValidationErrorCode::StorageUnavailable,
                message: "The cached issues could not be read.".to_owned(),
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
        UpdateStatusResponse::decl(&config),
        ConnectSourceRequest::decl(&config),
        ConnectionSummary::decl(&config),
        CredentialFieldSummary::decl(&config),
        ConnectionFieldSummary::decl(&config),
        ConnectionFieldType::decl(&config),
        SourceCapability::decl(&config),
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
        ChangeRequestState::decl(&config),
        ChangeRequestReviewStatus::decl(&config),
        ChangeRequestCheckStatus::decl(&config),
        ChangeRequestMergeStatus::decl(&config),
        ChangeRequestSummary::decl(&config),
        ListChangeRequestsResponse::decl(&config),
        ChangeRequestActivityKind::decl(&config),
        ChangeRequestActivitySummary::decl(&config),
        ListActivityResponse::decl(&config),
        ChangeRequestDetailsRequest::decl(&config),
        ChangeRequestReviewSummary::decl(&config),
        ChangeRequestCheckSummary::decl(&config),
        ChangeRequestCommitSummary::decl(&config),
        ChangeRequestDetailsResponse::decl(&config),
        ChangeRequestDetailsErrorResponse::decl(&config),
        IssueState::decl(&config),
        IssueSummary::decl(&config),
        ListIssuesResponse::decl(&config),
        IssueDetailsRequest::decl(&config),
        IssueCommentSummary::decl(&config),
        IssueDetailsResponse::decl(&config),
        IssueDetailsErrorResponse::decl(&config),
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
        NotificationPreferencesContract::decl(&config),
        UpdateMonitoringSettingsRequest::decl(&config),
        MonitoringSettingsErrorResponse::decl(&config),
        RepositorySelectionSourceRequest::decl(&config),
        SaveRepositorySelectionRequest::decl(&config),
        SaveRepositorySelectionResponse::decl(&config),
        RepositorySelectionErrorResponse::decl(&config),
    ]
    .join("\n\nexport ");
    format!(
        "// Generated from crates/opsscope-core/src/contracts.rs. Do not edit.\n\nexport {declarations}\n\nexport const applicationVersion = \"{APPLICATION_VERSION}\" as const;\n\nexport const settingsLimits = {{\n  synchronizationIntervalSeconds: {{ min: {MIN_SYNCHRONIZATION_INTERVAL_SECONDS}, max: {MAX_SYNCHRONIZATION_INTERVAL_SECONDS} }},\n  recentRunsPerWorkflow: {{ min: {MIN_RECENT_RUNS_PER_WORKFLOW}, max: {MAX_RECENT_RUNS_PER_WORKFLOW} }},\n}} as const;\n\nexport const httpRoutes = {{\n  health: \"{HEALTH_HTTP_PATH}\",\n  updateStatus: \"{UPDATE_STATUS_HTTP_PATH}\",\n  workflows: \"{WORKFLOWS_HTTP_PATH}\",\n  activity: \"{ACTIVITY_HTTP_PATH}\",\n  changeRequests: \"{CHANGE_REQUESTS_HTTP_PATH}\",\n  changeRequestDetails: \"{CHANGE_REQUEST_DETAILS_HTTP_PATH}\",\n  issues: \"{ISSUES_HTTP_PATH}\",\n  issueDetails: \"{ISSUE_DETAILS_HTTP_PATH}\",\n  workflowRunLogs: \"{WORKFLOW_RUN_LOGS_HTTP_PATH}\",\n  synchronization: \"{SYNCHRONIZATION_HTTP_PATH}\",\n  settings: \"{SETTINGS_HTTP_PATH}\",\n  sources: \"{SOURCES_HTTP_PATH}\",\n  connections: \"{CONNECTIONS_HTTP_PATH}\",\n  repositories: \"{REPOSITORIES_HTTP_PATH}\",\n  repositorySelections: \"{REPOSITORY_SELECTIONS_HTTP_PATH}\",\n}} as const;\n\nexport const desktopCommands = {{\n  health: \"{HEALTH_DESKTOP_COMMAND}\",\n  updateStatus: \"{UPDATE_STATUS_DESKTOP_COMMAND}\",\n  listWorkflows: \"{LIST_WORKFLOWS_DESKTOP_COMMAND}\",\n  listActivity: \"{LIST_ACTIVITY_DESKTOP_COMMAND}\",\n  listChangeRequests: \"{LIST_CHANGE_REQUESTS_DESKTOP_COMMAND}\",\n  changeRequestDetails: \"{CHANGE_REQUEST_DETAILS_DESKTOP_COMMAND}\",\n  listIssues: \"{LIST_ISSUES_DESKTOP_COMMAND}\",\n  issueDetails: \"{ISSUE_DETAILS_DESKTOP_COMMAND}\",\n  workflowRunLogs: \"{WORKFLOW_RUN_LOGS_DESKTOP_COMMAND}\",\n  synchronizeSources: \"{SYNCHRONIZE_SOURCES_DESKTOP_COMMAND}\",\n  synchronizationStatus: \"{SYNCHRONIZATION_STATUS_DESKTOP_COMMAND}\",\n  getSettings: \"{GET_SETTINGS_DESKTOP_COMMAND}\",\n  updateSettings: \"{UPDATE_SETTINGS_DESKTOP_COMMAND}\",\n  listSources: \"{LIST_SOURCES_DESKTOP_COMMAND}\",\n  connectSource: \"{CONNECT_SOURCE_DESKTOP_COMMAND}\",\n  disconnectSource: \"{DISCONNECT_SOURCE_DESKTOP_COMMAND}\",\n  listRepositories: \"{LIST_REPOSITORIES_DESKTOP_COMMAND}\",\n  saveRepositorySelection: \"{SAVE_REPOSITORY_SELECTION_DESKTOP_COMMAND}\",\n}} as const;\n\nexport interface ApplicationClient {{\n  health(): Promise<HealthResponse>;\n  updateStatus(): Promise<UpdateStatusResponse>;\n  listWorkflows(): Promise<ListWorkflowsResponse>;\n  listActivity(): Promise<ListActivityResponse>;\n  listChangeRequests(): Promise<ListChangeRequestsResponse>;\n  changeRequestDetails(request: ChangeRequestDetailsRequest): Promise<ChangeRequestDetailsResponse>;\n  listIssues(): Promise<ListIssuesResponse>;\n  issueDetails(request: IssueDetailsRequest): Promise<IssueDetailsResponse>;\n  workflowRunLogs(request: WorkflowRunLogsRequest): Promise<WorkflowRunLogsResponse>;\n  synchronizeSources(): Promise<SynchronizationResponse>;\n  synchronizationStatus(): Promise<SynchronizationStatusResponse>;\n  getSettings(): Promise<MonitoringSettingsResponse>;\n  updateSettings(request: UpdateMonitoringSettingsRequest): Promise<MonitoringSettingsResponse>;\n  listSources(): Promise<ListSourcesResponse>;\n  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary>;\n  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse>;\n  listRepositories(): Promise<ListRepositoriesResponse>;\n  saveRepositorySelection(request: SaveRepositorySelectionRequest): Promise<SaveRepositorySelectionResponse>;\n}}\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        ConnectedSource, SourceCapability as DomainSourceCapability, SourceDescriptor,
        ValidatedAccount,
    };

    #[test]
    fn maps_domain_values_without_exposing_infrastructure() {
        let response = WorkflowSummary::from(DiscoveredWorkflow {
            source: ConnectedSource {
                account_id: "42".into(),
                id: "source".to_owned(),
                label: "Source instance".to_owned(),
                descriptor: SourceDescriptor {
                    id: "module".to_owned(),
                    name: "Source".to_owned(),
                    description: "Source description".to_owned(),
                    abbreviation: "SO".to_owned(),
                    capabilities: vec![DomainSourceCapability::Workflows],
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
