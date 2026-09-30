use crate::domain::{
    ChangeRequest, ChangeRequestDetails, Issue, IssueDetails, Repository, Workflow, WorkflowRun,
    WorkflowRunLogs,
};
use async_trait::async_trait;
use std::collections::BTreeMap;
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
    pub input_type: ConnectionFieldType,
    pub key: String,
    pub label: String,
    pub placeholder: String,
    pub help: String,
    pub default_value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionFieldType {
    Url,
    Text,
    Email,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCapability {
    Workflows,
    ChangeRequests,
    Issues,
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
    async fn action_options(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
        _repository: &Repository,
        _target: &super::ActionTarget,
    ) -> Result<super::ActionOptions, super::ActionFailure> {
        Ok(super::ActionOptions::default())
    }

    async fn execute_action(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
        _repository: &Repository,
        _target: &super::ActionTarget,
        _action: super::SourceAction,
        _revision: Option<&str>,
    ) -> Result<(), super::ActionFailure> {
        Err(super::ActionFailure::Unsupported)
    }

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

    async fn list_issues(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
        _repository: &Repository,
    ) -> Result<Option<Vec<Issue>>, ConnectionValidationFailure> {
        Ok(None)
    }

    async fn issue_details(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
        _repository: &Repository,
        _number: u64,
    ) -> Result<Option<IssueDetails>, ConnectionValidationFailure> {
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
    pub(super) modules: Vec<Arc<dyn SourceModule>>,
}

impl SourceRegistry {
    #[must_use]
    pub fn new(modules: Vec<Arc<dyn SourceModule>>) -> Self {
        Self {
            modules,
        }
    }

    pub(crate) fn get(&self, source_id: &str) -> Option<Arc<dyn SourceModule>> {
        self.modules
            .iter()
            .find(|module| module.descriptor().id == source_id)
            .cloned()
    }
}
