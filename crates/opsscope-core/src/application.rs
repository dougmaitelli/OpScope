//! Application use cases and the ports they require.

mod activity;
mod change_request_details;
mod notifications;
mod run_logs;
mod settings;
mod sync;
mod updates;

pub use notifications::{
    LatestRunNotificationState, NoopNotificationSink, Notification, NotificationDeliveryFailure,
    NotificationSeverity, NotificationSink, NotificationStateRepository, NotifyRepositoryFailures,
    NotifyRepositoryFailuresFailure,
};
pub use run_logs::{GetWorkflowRunLogs, ResolvedWorkflowRunLogs};
pub use settings::{
    DEFAULT_RECENT_RUNS_PER_WORKFLOW, DEFAULT_SYNCHRONIZATION_INTERVAL_SECONDS,
    GetMonitoringSettings, MAX_RECENT_RUNS_PER_WORKFLOW, MAX_SYNCHRONIZATION_INTERVAL_SECONDS,
    MIN_RECENT_RUNS_PER_WORKFLOW, MIN_SYNCHRONIZATION_INTERVAL_SECONDS, MonitoringSettings,
    SettingsFailure, SettingsRepository, UpdateMonitoringSettings,
};

pub use sync::{
    DEFAULT_SYNCHRONIZATION_INTERVAL, SynchronizationFailure, SynchronizationStatus,
    SynchronizationSummary, SynchronizeSources,
};
pub use updates::{CheckForUpdates, ReleaseUpdate, UpdateCheckFailure};

use crate::domain::{
    ChangeRequest, ChangeRequestDetails, Repository, Workflow, WorkflowRun, WorkflowRunLogs,
};
use crate::source_data::{RefreshMode, SourceData, SourceDataFailure};
use async_trait::async_trait;
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use zeroize::Zeroize;

/// A provider credential whose contents are cleared when it leaves scope.
///
/// It deliberately implements neither `Clone` nor `Debug` so ordinary state
/// inspection cannot duplicate or print the credential.
pub struct ProviderToken(String);

impl ProviderToken {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Drop for ProviderToken {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedAccount {
    pub external_id: String,
    pub name: String,
    pub handle: Option<String>,
    pub profile_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialField {
    pub label: String,
    pub placeholder: String,
    pub help: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionField {
    pub key: String,
    pub label: String,
    pub placeholder: String,
    pub help: String,
    pub default_value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCapability {
    Workflows,
    ChangeRequests,
}

pub type ConnectionConfiguration = BTreeMap<String, String>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredSource {
    pub unique_key: String,
    pub label: String,
    pub configuration: ConnectionConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub abbreviation: String,
    pub capabilities: Vec<SourceCapability>,
    pub credential: CredentialField,
    pub connection_fields: Vec<ConnectionField>,
}

impl SourceDescriptor {
    #[must_use]
    pub fn supports(&self, capability: SourceCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionValidationFailure {
    InvalidConfiguration,
    InvalidCredentials,
    PermissionDenied,
    RateLimited,
    ProviderUnavailable,
    UnexpectedResponse,
}

impl Display for ConnectionValidationFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidConfiguration => "source configuration is invalid",
            Self::InvalidCredentials => "provider rejected the credential",
            Self::PermissionDenied => "provider credential lacks required permission",
            Self::RateLimited => "provider rate limit reached",
            Self::ProviderUnavailable => "provider unavailable",
            Self::UnexpectedResponse => "provider returned an unexpected response",
        })
    }
}

impl Error for ConnectionValidationFailure {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowRunLogsFailure {
    UnknownSource,
    SourceNotConnected,
    RepositoryNotFound,
    RunNotFound,
    LogsUnavailable,
    LogsTooLarge,
    InvalidCredentials,
    PermissionDenied,
    RateLimited,
    ProviderUnavailable,
    UnexpectedResponse,
    StorageUnavailable,
}

impl Display for WorkflowRunLogsFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownSource => "source module is not registered",
            Self::SourceNotConnected => "source is not connected",
            Self::RepositoryNotFound => "repository is not available",
            Self::RunNotFound => "workflow run is no longer available",
            Self::LogsUnavailable => "workflow run logs are not available",
            Self::LogsTooLarge => "workflow run logs exceed the viewing limit",
            Self::InvalidCredentials => "provider rejected the credential",
            Self::PermissionDenied => "provider credential lacks permission to read logs",
            Self::RateLimited => "provider rate limit reached",
            Self::ProviderUnavailable => "provider unavailable",
            Self::UnexpectedResponse => "provider returned unexpected workflow run logs",
            Self::StorageUnavailable => "stored connection credential could not be read",
        })
    }
}

impl Error for WorkflowRunLogsFailure {}

#[async_trait]
pub trait SourceModule: Send + Sync {
    fn descriptor(&self) -> SourceDescriptor;

    fn configure(
        &self,
        configuration: &ConnectionConfiguration,
    ) -> Result<ConfiguredSource, ConnectionValidationFailure>;

    async fn validate(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<ValidatedAccount, ConnectionValidationFailure>;

    async fn list_repositories(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<Vec<Repository>, ConnectionValidationFailure>;

    async fn list_workflows(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, ConnectionValidationFailure>;

    async fn list_workflow_runs(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, ConnectionValidationFailure>;

    async fn workflow_run(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
        _repository: &Repository,
        _run_id: &str,
    ) -> Result<Option<WorkflowRun>, ConnectionValidationFailure> {
        Ok(None)
    }

    async fn list_change_requests(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
        _repository: &Repository,
    ) -> Result<Option<Vec<ChangeRequest>>, ConnectionValidationFailure> {
        Ok(None)
    }

    async fn change_request_details(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
        _repository: &Repository,
        _number: u64,
    ) -> Result<Option<ChangeRequestDetails>, ConnectionValidationFailure> {
        Ok(None)
    }

    async fn workflow_run_logs(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure>;
}

#[derive(Clone)]
pub struct SourceRegistry {
    modules: Vec<Arc<dyn SourceModule>>,
}

impl SourceRegistry {
    #[must_use]
    pub fn new(modules: Vec<Arc<dyn SourceModule>>) -> Self {
        Self { modules }
    }

    pub(crate) fn get(&self, source_id: &str) -> Option<Arc<dyn SourceModule>> {
        self.modules
            .iter()
            .find(|module| module.descriptor().id == source_id)
            .cloned()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretReference(String);

impl SecretReference {
    #[must_use]
    pub fn for_connection(connection_id: &str, nonce: &str) -> Self {
        Self(format!("connection:{connection_id}:{nonce}"))
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredConnection {
    pub id: String,
    pub source_id: String,
    pub unique_key: String,
    pub label: String,
    pub configuration: ConnectionConfiguration,
    pub account: ValidatedAccount,
    pub secret_reference: SecretReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistenceFailure;

impl Display for PersistenceFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("connection storage unavailable")
    }
}

impl Error for PersistenceFailure {}

pub trait ConnectionRepository: Send + Sync {
    fn save(&self, connection: &StoredConnection) -> Result<(), PersistenceFailure>;
    fn get(&self, connection_id: &str) -> Result<Option<StoredConnection>, PersistenceFailure>;
    fn find(
        &self,
        source_id: &str,
        unique_key: &str,
    ) -> Result<Option<StoredConnection>, PersistenceFailure>;
    fn list(&self) -> Result<Vec<StoredConnection>, PersistenceFailure>;
    fn delete(&self, connection_id: &str) -> Result<(), PersistenceFailure>;
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RepositorySelection {
    pub source_id: String,
    pub repository_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRepositorySelection {
    pub source_id: String,
    pub repository_ids: Vec<String>,
}

pub trait RepositorySelectionRepository: Send + Sync {
    fn list(&self) -> Result<Vec<RepositorySelection>, PersistenceFailure>;
    fn replace_for_sources(
        &self,
        selections: &[SourceRepositorySelection],
    ) -> Result<(), PersistenceFailure>;
}

pub trait SecretStore: Send + Sync {
    fn store(
        &self,
        reference: &SecretReference,
        token: &ProviderToken,
    ) -> Result<(), PersistenceFailure>;
    fn retrieve(&self, reference: &SecretReference) -> Result<ProviderToken, PersistenceFailure>;
    fn delete(&self, reference: &SecretReference) -> Result<(), PersistenceFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectSourceFailure {
    UnknownSource,
    UnknownConnection,
    DuplicateConnection,
    Validation(ConnectionValidationFailure),
    StorageUnavailable,
}

impl Display for ConnectSourceFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSource => formatter.write_str("source module is not registered"),
            Self::UnknownConnection => formatter.write_str("source connection does not exist"),
            Self::DuplicateConnection => formatter.write_str("source connection already exists"),
            Self::Validation(failure) => Display::fmt(failure, formatter),
            Self::StorageUnavailable => formatter.write_str("connection storage unavailable"),
        }
    }
}

impl Error for ConnectSourceFailure {}

#[derive(Clone)]
pub struct ConnectSource {
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
}

impl ConnectSource {
    #[must_use]
    pub fn new(
        registry: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            registry,
            connections,
            secrets,
        }
    }

    pub async fn execute(
        &self,
        source_id: &str,
        connection_id: Option<&str>,
        configuration: &ConnectionConfiguration,
        token: String,
    ) -> Result<ConnectionState, ConnectSourceFailure> {
        let module = self
            .registry
            .get(source_id)
            .ok_or(ConnectSourceFailure::UnknownSource)?;
        let configured = module
            .configure(configuration)
            .map_err(ConnectSourceFailure::Validation)?;
        if token.trim().is_empty() {
            return Err(ConnectSourceFailure::Validation(
                ConnectionValidationFailure::InvalidCredentials,
            ));
        }

        let old_connection = if let Some(connection_id) = connection_id {
            let connection = self
                .connections
                .get(connection_id)
                .map_err(|_| ConnectSourceFailure::StorageUnavailable)?
                .ok_or(ConnectSourceFailure::UnknownConnection)?;
            if connection.source_id != source_id {
                return Err(ConnectSourceFailure::UnknownConnection);
            }
            Some(connection)
        } else {
            None
        };
        if old_connection
            .as_ref()
            .is_some_and(|connection| connection.unique_key != configured.unique_key)
        {
            return Err(ConnectSourceFailure::Validation(
                ConnectionValidationFailure::InvalidConfiguration,
            ));
        }
        if self
            .connections
            .find(source_id, &configured.unique_key)
            .map_err(|_| ConnectSourceFailure::StorageUnavailable)?
            .is_some_and(|existing| Some(existing.id.as_str()) != connection_id)
        {
            return Err(ConnectSourceFailure::DuplicateConnection);
        }

        let token = ProviderToken::new(token);
        let account = module
            .validate(&configured.configuration, &token)
            .await
            .map_err(ConnectSourceFailure::Validation)?;
        let connection_id = old_connection
            .as_ref()
            .map_or_else(random_connection_id, |connection| Ok(connection.id.clone()))
            .map_err(|_| ConnectSourceFailure::StorageUnavailable)?;
        let secret_nonce =
            random_identifier().map_err(|_| ConnectSourceFailure::StorageUnavailable)?;
        let secret_reference = SecretReference::for_connection(&connection_id, &secret_nonce);

        self.secrets
            .store(&secret_reference, &token)
            .map_err(|_| ConnectSourceFailure::StorageUnavailable)?;

        let connection = StoredConnection {
            id: connection_id,
            source_id: source_id.to_owned(),
            unique_key: configured.unique_key,
            label: configured.label,
            configuration: configured.configuration,
            account: account.clone(),
            secret_reference: secret_reference.clone(),
        };
        if self.connections.save(&connection).is_err() {
            if old_connection
                .as_ref()
                .is_none_or(|old| old.secret_reference != secret_reference)
            {
                _ = self.secrets.delete(&secret_reference);
            }
            return Err(ConnectSourceFailure::StorageUnavailable);
        }

        if let Some(old) = old_connection
            && old.secret_reference != secret_reference
        {
            _ = self.secrets.delete(&old.secret_reference);
        }

        Ok(ConnectionState::from(connection))
    }
}

fn random_connection_id() -> Result<String, PersistenceFailure> {
    random_identifier().map(|identifier| format!("source-{identifier}"))
}

fn random_identifier() -> Result<String, PersistenceFailure> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| PersistenceFailure)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionState {
    pub id: String,
    pub label: String,
    pub configuration: ConnectionConfiguration,
    pub account: ValidatedAccount,
}

impl From<StoredConnection> for ConnectionState {
    fn from(connection: StoredConnection) -> Self {
        Self {
            id: connection.id,
            label: connection.label,
            configuration: connection.configuration,
            account: connection.account,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceState {
    pub descriptor: SourceDescriptor,
    pub connections: Vec<ConnectionState>,
}

#[derive(Clone)]
pub struct ListSources {
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
}

impl ListSources {
    #[must_use]
    pub fn new(registry: SourceRegistry, connections: Arc<dyn ConnectionRepository>) -> Self {
        Self {
            registry,
            connections,
        }
    }

    pub fn execute(&self) -> Result<Vec<SourceState>, PersistenceFailure> {
        let connections = self.connections.list()?;
        Ok(self
            .registry
            .modules
            .iter()
            .map(|module| {
                let descriptor = module.descriptor();
                let source_connections = connections
                    .iter()
                    .filter(|connection| connection.source_id == descriptor.id)
                    .cloned()
                    .map(ConnectionState::from)
                    .collect();
                SourceState {
                    descriptor,
                    connections: source_connections,
                }
            })
            .collect())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectedSource {
    pub id: String,
    pub descriptor: SourceDescriptor,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryCatalog {
    pub source: ConnectedSource,
    pub repositories: Vec<RepositoryState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryState {
    pub repository: Repository,
    pub selected: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListRepositoriesFailure {
    Source(ConnectionValidationFailure),
    StorageUnavailable,
}

impl Display for ListRepositoriesFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(failure) => Display::fmt(failure, formatter),
            Self::StorageUnavailable => formatter.write_str("connection storage unavailable"),
        }
    }
}

impl Error for ListRepositoriesFailure {}

#[derive(Clone)]
pub struct ListRepositories {
    source_data: Arc<dyn SourceData>,
    selections: Arc<dyn RepositorySelectionRepository>,
}

impl ListRepositories {
    #[must_use]
    pub fn new(
        source_data: Arc<dyn SourceData>,
        selections: Arc<dyn RepositorySelectionRepository>,
    ) -> Self {
        Self {
            source_data,
            selections,
        }
    }

    pub async fn execute(&self) -> Result<Vec<RepositoryCatalog>, ListRepositoriesFailure> {
        let mut catalogs = Vec::new();
        let selected = self
            .selections
            .list()
            .map_err(|_| ListRepositoriesFailure::StorageUnavailable)?
            .into_iter()
            .collect::<HashSet<_>>();
        for source in self
            .source_data
            .sources()
            .map_err(list_repositories_failure)?
        {
            let Some(repositories) = self
                .source_data
                .repositories(&source.id, RefreshMode::IfStale)
                .await
                .map_err(list_repositories_failure)?
            else {
                continue;
            };
            let source_id = source.id.clone();
            catalogs.push(RepositoryCatalog {
                source,
                repositories: repositories
                    .into_iter()
                    .map(|repository| {
                        let is_selected = selected.contains(&RepositorySelection {
                            source_id: source_id.clone(),
                            repository_id: repository.id.clone(),
                        });
                        RepositoryState {
                            repository,
                            selected: is_selected,
                        }
                    })
                    .collect(),
            });
        }
        Ok(catalogs)
    }
}

fn list_repositories_failure(failure: SourceDataFailure) -> ListRepositoriesFailure {
    match failure {
        SourceDataFailure::Source(failure) => ListRepositoriesFailure::Source(failure),
        SourceDataFailure::StorageUnavailable => ListRepositoriesFailure::StorageUnavailable,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveRepositorySelectionFailure {
    InvalidSelection,
    SourceNotConnected,
    StorageUnavailable,
}

impl Display for SaveRepositorySelectionFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSelection => "repository selection is invalid",
            Self::SourceNotConnected => "source is not connected",
            Self::StorageUnavailable => "repository selection storage unavailable",
        })
    }
}

impl Error for SaveRepositorySelectionFailure {}

#[derive(Clone)]
pub struct SaveRepositorySelection {
    connections: Arc<dyn ConnectionRepository>,
    selections: Arc<dyn RepositorySelectionRepository>,
}

impl SaveRepositorySelection {
    #[must_use]
    pub fn new(
        connections: Arc<dyn ConnectionRepository>,
        selections: Arc<dyn RepositorySelectionRepository>,
    ) -> Self {
        Self {
            connections,
            selections,
        }
    }

    pub fn execute(
        &self,
        selections: &[SourceRepositorySelection],
    ) -> Result<usize, SaveRepositorySelectionFailure> {
        let mut source_ids = HashSet::new();
        for selection in selections {
            if !source_ids.insert(&selection.source_id)
                || selection.repository_ids.iter().any(String::is_empty)
                || selection
                    .repository_ids
                    .iter()
                    .collect::<HashSet<_>>()
                    .len()
                    != selection.repository_ids.len()
            {
                return Err(SaveRepositorySelectionFailure::InvalidSelection);
            }
            if self
                .connections
                .get(&selection.source_id)
                .map_err(|_| SaveRepositorySelectionFailure::StorageUnavailable)?
                .is_none()
            {
                return Err(SaveRepositorySelectionFailure::SourceNotConnected);
            }
        }

        self.selections
            .replace_for_sources(selections)
            .map_err(|_| SaveRepositorySelectionFailure::StorageUnavailable)?;
        Ok(selections
            .iter()
            .map(|selection| selection.repository_ids.len())
            .sum())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredWorkflow {
    pub source: ConnectedSource,
    pub repository: Repository,
    pub workflow: Workflow,
    pub runs: Vec<WorkflowRun>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowInventory {
    pub selected_repository_count: usize,
    pub workflows: Vec<DiscoveredWorkflow>,
    pub last_attempted_at: Option<u64>,
    pub last_successful_at: Option<u64>,
    pub stale: bool,
    pub sync_error: Option<ConnectionValidationFailure>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredChangeRequest {
    pub source: ConnectedSource,
    pub repository: Repository,
    pub change_request: ChangeRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangeRequestInventory {
    pub selected_repository_count: usize,
    pub change_requests: Vec<DiscoveredChangeRequest>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListChangeRequestsFailure {
    Source(ConnectionValidationFailure),
    StorageUnavailable,
}

impl Display for ListChangeRequestsFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(failure) => Display::fmt(failure, formatter),
            Self::StorageUnavailable => {
                formatter.write_str("change request discovery storage unavailable")
            }
        }
    }
}

impl Error for ListChangeRequestsFailure {}

#[derive(Clone)]
pub struct ListChangeRequests {
    source_data: Arc<dyn SourceData>,
    selections: Arc<dyn RepositorySelectionRepository>,
}

impl ListChangeRequests {
    #[must_use]
    pub fn new(
        source_data: Arc<dyn SourceData>,
        selections: Arc<dyn RepositorySelectionRepository>,
    ) -> Self {
        Self {
            source_data,
            selections,
        }
    }

    pub async fn execute(&self) -> Result<ChangeRequestInventory, ListChangeRequestsFailure> {
        let selections = self
            .selections
            .list()
            .map_err(|_| ListChangeRequestsFailure::StorageUnavailable)?;
        let selected_repository_count = selections.len();
        let mut change_requests = Vec::new();
        for source in self
            .source_data
            .sources()
            .map_err(list_change_requests_failure)?
        {
            if !source.descriptor.supports(SourceCapability::ChangeRequests) {
                continue;
            }
            let selected_ids = selections
                .iter()
                .filter(|selection| selection.source_id == source.id)
                .map(|selection| selection.repository_id.clone())
                .collect::<HashSet<_>>();
            if selected_ids.is_empty() {
                continue;
            }
            let repositories = self
                .source_data
                .repositories(&source.id, RefreshMode::CacheFirst)
                .await
                .map_err(list_change_requests_failure)?
                .ok_or(ListChangeRequestsFailure::StorageUnavailable)?;
            for repository in repositories
                .into_iter()
                .filter(|repository| selected_ids.contains(&repository.id))
            {
                let Some(repository_change_requests) = self
                    .source_data
                    .change_requests(&source.id, &repository, RefreshMode::CacheFirst)
                    .await
                    .map_err(list_change_requests_failure)?
                else {
                    continue;
                };
                change_requests.extend(repository_change_requests.into_iter().map(
                    |change_request| DiscoveredChangeRequest {
                        source: source.clone(),
                        repository: repository.clone(),
                        change_request,
                    },
                ));
            }
        }
        change_requests.sort_by(|left, right| {
            right
                .change_request
                .updated_at
                .cmp(&left.change_request.updated_at)
        });
        Ok(ChangeRequestInventory {
            selected_repository_count,
            change_requests,
        })
    }
}

fn list_change_requests_failure(failure: SourceDataFailure) -> ListChangeRequestsFailure {
    match failure {
        SourceDataFailure::Source(failure) => ListChangeRequestsFailure::Source(failure),
        SourceDataFailure::StorageUnavailable => ListChangeRequestsFailure::StorageUnavailable,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListWorkflowsFailure {
    Source(ConnectionValidationFailure),
    StorageUnavailable,
}

impl Display for ListWorkflowsFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(failure) => Display::fmt(failure, formatter),
            Self::StorageUnavailable => {
                formatter.write_str("workflow discovery storage unavailable")
            }
        }
    }
}

impl Error for ListWorkflowsFailure {}

#[derive(Clone)]
pub struct ListWorkflows {
    source_data: Arc<dyn SourceData>,
    selections: Arc<dyn RepositorySelectionRepository>,
    settings: Arc<dyn SettingsRepository>,
}

impl ListWorkflows {
    #[must_use]
    pub fn new(
        source_data: Arc<dyn SourceData>,
        selections: Arc<dyn RepositorySelectionRepository>,
        settings: Arc<dyn SettingsRepository>,
    ) -> Self {
        Self {
            source_data,
            selections,
            settings,
        }
    }

    pub async fn execute(&self) -> Result<WorkflowInventory, ListWorkflowsFailure> {
        let recent_runs_per_workflow = self
            .settings
            .load_settings()
            .map_err(|_| ListWorkflowsFailure::StorageUnavailable)?
            .recent_runs_per_workflow;
        let selections = self
            .selections
            .list()
            .map_err(|_| ListWorkflowsFailure::StorageUnavailable)?;
        let selected_repository_count = selections.len();
        let mut workflows = Vec::new();
        let mut last_attempted_at = None;
        let mut last_successful_at = None;
        let mut stale = false;
        let mut sync_error = None;

        for source in self.source_data.sources().map_err(list_workflows_failure)? {
            let selected_ids = selections
                .iter()
                .filter(|selection| selection.source_id == source.id)
                .map(|selection| selection.repository_id.clone())
                .collect::<HashSet<_>>();
            if selected_ids.is_empty() {
                continue;
            }

            let repositories = self
                .source_data
                .repositories(&source.id, RefreshMode::CacheFirst)
                .await
                .map_err(list_workflows_failure)?
                .ok_or(ListWorkflowsFailure::StorageUnavailable)?;

            for repository in repositories
                .into_iter()
                .filter(|repository| selected_ids.contains(&repository.id))
            {
                let repository_workflows = self
                    .source_data
                    .workflows(&source.id, &repository, RefreshMode::CacheFirst)
                    .await
                    .map_err(list_workflows_failure)?;
                let repository_runs = self
                    .source_data
                    .workflow_runs(&source.id, &repository, RefreshMode::CacheFirst)
                    .await
                    .map_err(list_workflows_failure)?;
                last_attempted_at = Some(
                    last_attempted_at.map_or(repository_runs.last_attempted_at, |current: u64| {
                        current.max(repository_runs.last_attempted_at)
                    }),
                );
                last_successful_at = Some(
                    last_successful_at
                        .map_or(repository_runs.last_successful_at, |current: u64| {
                            current.min(repository_runs.last_successful_at)
                        }),
                );
                stale |= repository_runs.stale;
                sync_error = sync_error.or(repository_runs.error);
                workflows.extend(repository_workflows.into_iter().map(|workflow| {
                    let runs = repository_runs
                        .runs
                        .iter()
                        .filter(|run| run.workflow_id == workflow.id)
                        .take(recent_runs_per_workflow)
                        .cloned()
                        .collect();
                    DiscoveredWorkflow {
                        source: source.clone(),
                        repository: repository.clone(),
                        workflow,
                        runs,
                    }
                }));
            }
        }

        Ok(WorkflowInventory {
            selected_repository_count,
            workflows,
            last_attempted_at,
            last_successful_at,
            stale,
            sync_error,
        })
    }
}

fn list_workflows_failure(failure: SourceDataFailure) -> ListWorkflowsFailure {
    match failure {
        SourceDataFailure::Source(failure) => ListWorkflowsFailure::Source(failure),
        SourceDataFailure::StorageUnavailable => ListWorkflowsFailure::StorageUnavailable,
    }
}

#[derive(Clone)]
pub struct DisconnectSource {
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
}

impl DisconnectSource {
    #[must_use]
    pub fn new(connections: Arc<dyn ConnectionRepository>, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            connections,
            secrets,
        }
    }

    pub fn execute(&self, connection_id: &str) -> Result<bool, PersistenceFailure> {
        let Some(connection) = self.connections.get(connection_id)? else {
            return Ok(false);
        };
        self.secrets.delete(&connection.secret_reference)?;
        self.connections.delete(connection_id)?;
        Ok(true)
    }
}
pub use activity::{
    ActivityEventRepository, ChangeRequestActivityEvent, ChangeRequestActivityKind, ListActivity,
    TrackChangeRequestActivity,
};
pub use change_request_details::{GetChangeRequestDetails, GetChangeRequestDetailsFailure};
