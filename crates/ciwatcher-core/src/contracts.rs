//! Data transfer objects shared by HTTP and desktop IPC.

use crate::application::{
    ConnectSourceFailure, ConnectionValidationFailure, DiscoveredWorkflow, ListRepositoriesFailure,
    ListWorkflowsFailure, RepositoryCatalog, RepositoryState, SaveRepositorySelectionFailure,
    SourceRepositorySelection, SourceState, ValidatedAccount, WorkflowInventory,
};
use crate::domain::{
    RepositoryVisibility as DomainRepositoryVisibility, RunLifecycle as DomainRunLifecycle,
    RunOutcome as DomainRunOutcome, WorkflowRun as DomainWorkflowRun,
    WorkflowState as DomainWorkflowState,
};
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

pub const CONTRACT_VERSION: u8 = 5;
pub const HEALTH_HTTP_PATH: &str = "/api/health";
pub const WORKFLOWS_HTTP_PATH: &str = "/api/workflows";
pub const SOURCES_HTTP_PATH: &str = "/api/sources";
pub const CONNECTIONS_HTTP_PATH: &str = "/api/connections";
pub const REPOSITORIES_HTTP_PATH: &str = "/api/repositories";
pub const REPOSITORY_SELECTIONS_HTTP_PATH: &str = "/api/repository-selections";
pub const HEALTH_DESKTOP_COMMAND: &str = "health";
pub const LIST_WORKFLOWS_DESKTOP_COMMAND: &str = "list_workflows";
pub const LIST_SOURCES_DESKTOP_COMMAND: &str = "list_sources";
pub const CONNECT_SOURCE_DESKTOP_COMMAND: &str = "connect_source";
pub const DISCONNECT_SOURCE_DESKTOP_COMMAND: &str = "disconnect_source";
pub const LIST_REPOSITORIES_DESKTOP_COMMAND: &str = "list_repositories";
pub const SAVE_REPOSITORY_SELECTION_DESKTOP_COMMAND: &str = "save_repository_selection";

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
            service: "ciwatcher".to_owned(),
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
    pub credential: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSummary {
    pub external_id: String,
    pub name: String,
    pub handle: Option<String>,
    pub profile_url: Option<String>,
    pub credential_stored: bool,
}

impl From<ValidatedAccount> for ConnectionSummary {
    fn from(account: ValidatedAccount) -> Self {
        Self {
            external_id: account.external_id,
            name: account.name,
            handle: account.handle,
            profile_url: account.profile_url,
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
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub abbreviation: String,
    pub credential: CredentialFieldSummary,
    pub connection: Option<ConnectionSummary>,
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
            connection: source.account.map(ConnectionSummary::from),
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
            name: catalog.source.name,
            abbreviation: catalog.source.abbreviation,
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
            source_name: discovered.source.name,
            source_abbreviation: discovered.source.abbreviation,
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
    pub source_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectSourceResponse {
    pub disconnected: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionValidationErrorCode {
    InvalidCredentials,
    PermissionDenied,
    RateLimited,
    ProviderUnavailable,
    UnexpectedResponse,
    StorageUnavailable,
    UnknownSource,
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

#[must_use]
pub fn render_typescript_contract() -> String {
    let config = Config::default();
    let declarations = [
        HealthResponse::decl(&config),
        ConnectSourceRequest::decl(&config),
        ConnectionSummary::decl(&config),
        CredentialFieldSummary::decl(&config),
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
        WorkflowSummary::decl(&config),
        ListWorkflowsResponse::decl(&config),
        RepositorySelectionSourceRequest::decl(&config),
        SaveRepositorySelectionRequest::decl(&config),
        SaveRepositorySelectionResponse::decl(&config),
        RepositorySelectionErrorResponse::decl(&config),
    ]
    .join("\n\nexport ");
    format!(
        "// Generated from crates/ciwatcher-core/src/contracts.rs. Do not edit.\n\nexport {declarations}\n\nexport const httpRoutes = {{\n  health: \"{HEALTH_HTTP_PATH}\",\n  workflows: \"{WORKFLOWS_HTTP_PATH}\",\n  sources: \"{SOURCES_HTTP_PATH}\",\n  connections: \"{CONNECTIONS_HTTP_PATH}\",\n  repositories: \"{REPOSITORIES_HTTP_PATH}\",\n  repositorySelections: \"{REPOSITORY_SELECTIONS_HTTP_PATH}\",\n}} as const;\n\nexport const desktopCommands = {{\n  health: \"{HEALTH_DESKTOP_COMMAND}\",\n  listWorkflows: \"{LIST_WORKFLOWS_DESKTOP_COMMAND}\",\n  listSources: \"{LIST_SOURCES_DESKTOP_COMMAND}\",\n  connectSource: \"{CONNECT_SOURCE_DESKTOP_COMMAND}\",\n  disconnectSource: \"{DISCONNECT_SOURCE_DESKTOP_COMMAND}\",\n  listRepositories: \"{LIST_REPOSITORIES_DESKTOP_COMMAND}\",\n  saveRepositorySelection: \"{SAVE_REPOSITORY_SELECTION_DESKTOP_COMMAND}\",\n}} as const;\n\nexport interface ApplicationClient {{\n  health(): Promise<HealthResponse>;\n  listWorkflows(): Promise<ListWorkflowsResponse>;\n  listSources(): Promise<ListSourcesResponse>;\n  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary>;\n  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse>;\n  listRepositories(): Promise<ListRepositoriesResponse>;\n  saveRepositorySelection(request: SaveRepositorySelectionRequest): Promise<SaveRepositorySelectionResponse>;\n}}\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::SourceDescriptor;

    #[test]
    fn maps_domain_values_without_exposing_infrastructure() {
        let response = WorkflowSummary::from(DiscoveredWorkflow {
            source: SourceDescriptor {
                id: "source".to_owned(),
                name: "Source".to_owned(),
                description: "Source description".to_owned(),
                abbreviation: "SO".to_owned(),
                credential: crate::application::CredentialField {
                    label: "Token".to_owned(),
                    placeholder: "token".to_owned(),
                    help: "Help".to_owned(),
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
        let response = ConnectionSummary::from(ValidatedAccount {
            external_id: "42".to_owned(),
            name: "The Octocat".to_owned(),
            handle: Some("octocat".to_owned()),
            profile_url: Some("https://example.com/octocat".to_owned()),
        });
        let serialized = serde_json::to_string(&response).expect("response serializes");

        assert!(serialized.contains("octocat"));
        assert!(!serialized.contains("token"));
        assert!(!serialized.contains("secret"));
    }
}
