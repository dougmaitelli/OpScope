//! Application use cases and the ports they require.

mod sync;

pub use sync::{
    DEFAULT_SYNCHRONIZATION_INTERVAL, SynchronizationFailure, SynchronizationStatus,
    SynchronizationSummary, SynchronizeSources,
};

use crate::domain::{Repository, Workflow, WorkflowRun};
use crate::source_data::{RefreshMode, SourceData, SourceDataFailure};
use async_trait::async_trait;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use zeroize::Zeroize;

/// Default number of recent activities returned for each workflow.
///
/// This is the single application-level default until workspace settings make
/// the value configurable.
pub const RECENT_RUNS_PER_WORKFLOW: usize = 10;

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
pub struct SourceDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub abbreviation: String,
    pub credential: CredentialField,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionValidationFailure {
    InvalidCredentials,
    PermissionDenied,
    RateLimited,
    ProviderUnavailable,
    UnexpectedResponse,
}

impl Display for ConnectionValidationFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCredentials => "provider rejected the credential",
            Self::PermissionDenied => "provider credential lacks required permission",
            Self::RateLimited => "provider rate limit reached",
            Self::ProviderUnavailable => "provider unavailable",
            Self::UnexpectedResponse => "provider returned an unexpected response",
        })
    }
}

impl Error for ConnectionValidationFailure {}

#[async_trait]
pub trait SourceModule: Send + Sync {
    fn descriptor(&self) -> SourceDescriptor;

    async fn validate(
        &self,
        token: &ProviderToken,
    ) -> Result<ValidatedAccount, ConnectionValidationFailure>;

    async fn list_repositories(
        &self,
        token: &ProviderToken,
    ) -> Result<Vec<Repository>, ConnectionValidationFailure>;

    async fn list_workflows(
        &self,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, ConnectionValidationFailure>;

    async fn list_workflow_runs(
        &self,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, ConnectionValidationFailure>;
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

    pub(crate) fn descriptors(&self) -> Vec<SourceDescriptor> {
        self.modules
            .iter()
            .map(|module| module.descriptor())
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretReference(String);

impl SecretReference {
    #[must_use]
    pub fn for_source(source_id: &str, external_id: &str) -> Self {
        Self(format!("{source_id}:{external_id}"))
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
    pub source_id: String,
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
    fn get(&self, source_id: &str) -> Result<Option<StoredConnection>, PersistenceFailure>;
    fn delete(&self, source_id: &str) -> Result<(), PersistenceFailure>;
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
    Validation(ConnectionValidationFailure),
    StorageUnavailable,
}

impl Display for ConnectSourceFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSource => formatter.write_str("source module is not registered"),
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
        token: String,
    ) -> Result<ValidatedAccount, ConnectSourceFailure> {
        let module = self
            .registry
            .get(source_id)
            .ok_or(ConnectSourceFailure::UnknownSource)?;
        if token.trim().is_empty() {
            return Err(ConnectSourceFailure::Validation(
                ConnectionValidationFailure::InvalidCredentials,
            ));
        }

        let token = ProviderToken::new(token);
        let account = module
            .validate(&token)
            .await
            .map_err(ConnectSourceFailure::Validation)?;
        let old_connection = self
            .connections
            .get(source_id)
            .map_err(|_| ConnectSourceFailure::StorageUnavailable)?;
        let secret_reference = SecretReference::for_source(source_id, &account.external_id);

        self.secrets
            .store(&secret_reference, &token)
            .map_err(|_| ConnectSourceFailure::StorageUnavailable)?;

        let connection = StoredConnection {
            source_id: source_id.to_owned(),
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

        Ok(account)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceState {
    pub descriptor: SourceDescriptor,
    pub account: Option<ValidatedAccount>,
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
        self.registry
            .modules
            .iter()
            .map(|module| {
                let descriptor = module.descriptor();
                self.connections
                    .get(&descriptor.id)
                    .map(|connection| SourceState {
                        descriptor,
                        account: connection.map(|stored| stored.account),
                    })
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryCatalog {
    pub source: SourceDescriptor,
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
        for source in self.source_data.sources() {
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
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    selections: Arc<dyn RepositorySelectionRepository>,
}

impl SaveRepositorySelection {
    #[must_use]
    pub fn new(
        registry: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        selections: Arc<dyn RepositorySelectionRepository>,
    ) -> Self {
        Self {
            registry,
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
                || self.registry.get(&selection.source_id).is_none()
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
    pub source: SourceDescriptor,
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
}

impl ListWorkflows {
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

    pub async fn execute(&self) -> Result<WorkflowInventory, ListWorkflowsFailure> {
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

        for source in self.source_data.sources() {
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
                        .take(RECENT_RUNS_PER_WORKFLOW)
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
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
}

impl DisconnectSource {
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

    pub fn execute(&self, source_id: &str) -> Result<bool, PersistenceFailure> {
        if self.registry.get(source_id).is_none() {
            return Ok(false);
        }
        let Some(connection) = self.connections.get(source_id)? else {
            return Ok(false);
        };
        self.secrets.delete(&connection.secret_reference)?;
        self.connections.delete(source_id)?;
        Ok(true)
    }
}
