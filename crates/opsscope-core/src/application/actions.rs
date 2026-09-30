use super::{ConnectionRepository, ConnectionValidationFailure, SecretStore, SourceRegistry};
use crate::source_data::{RefreshMode, SourceData};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq, Hash, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ActionTarget {
    WorkflowRun {
        run_id: String,
    },
    ChangeRequest {
        #[ts(type = "number")]
        number: u64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub enum SourceAction {
    RerunWorkflow,
    UpdateBranch,
    MergeChangeRequest,
    DependabotRebase,
    DependabotRecreate,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AvailableAction {
    pub action: SourceAction,
    pub label: String,
    pub confirmation: String,
    pub disabled_reason: Option<String>,
}

impl AvailableAction {
    pub fn new(
        action: SourceAction,
        label: &str,
        confirmation: &str,
        disabled_reason: Option<&str>,
    ) -> Self {
        Self {
            action,
            label: label.into(),
            confirmation: confirmation.into(),
            disabled_reason: disabled_reason.map(str::to_owned),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ActionOptions {
    pub actions: Vec<AvailableAction>,
    // Fresh provider revision, never a cached authorization decision.
    pub revision: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActionRefresh {
    pub run: Option<crate::domain::WorkflowRun>,
    pub change_request: Option<super::DiscoveredChangeRequest>,
    pub details: Option<crate::domain::ChangeRequestDetails>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionFailure {
    NotFound,
    Unsupported,
    Conflict,
    Busy,
    StorageUnavailable,
    Source(ConnectionValidationFailure),
    /// A write may have reached the provider: never automatically retry it.
    OutcomeUnknown,
}

impl std::fmt::Display for ActionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => f.write_str("The connection or resource is no longer available."),
            Self::Unsupported => f.write_str("This action is not supported for this resource."),
            Self::Conflict => f.write_str("The resource changed, has conflicts, or is not ready. Reload available actions and try again."),
            Self::Busy => f.write_str("An action is already running for this resource."),
            Self::StorageUnavailable => f.write_str("Stored connection data is unavailable."),
            Self::Source(failure) => std::fmt::Display::fmt(failure, f),
            Self::OutcomeUnknown => f.write_str("The provider did not confirm the result. Check the provider before retrying; the action may already have been accepted."),
        }
    }
}
impl std::error::Error for ActionFailure {}
impl From<ConnectionValidationFailure> for ActionFailure {
    fn from(value: ConnectionValidationFailure) -> Self {
        Self::Source(value)
    }
}

type ActionKey = (String, String, ActionTarget);

#[derive(Clone)]
pub struct SourceActions {
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
    data: Arc<dyn SourceData>,
    active: Arc<Mutex<HashSet<ActionKey>>>,
}

struct ActionPermit {
    active: Arc<Mutex<HashSet<ActionKey>>>,
    key: ActionKey,
}
impl ActionPermit {
    fn acquire(
        active: &Arc<Mutex<HashSet<ActionKey>>>,
        key: ActionKey,
    ) -> Result<Self, ActionFailure> {
        if !active
            .lock()
            .map_err(|_| ActionFailure::StorageUnavailable)?
            .insert(key.clone())
        {
            return Err(ActionFailure::Busy);
        }
        Ok(Self {
            active: active.clone(),
            key,
        })
    }
}
impl Drop for ActionPermit {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.key);
        }
    }
}

impl SourceActions {
    pub fn new(
        registry: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        secrets: Arc<dyn SecretStore>,
        data: Arc<dyn SourceData>,
    ) -> Self {
        Self {
            registry,
            connections,
            secrets,
            data,
            active: Arc::default(),
        }
    }

    async fn resolve(
        &self,
        source_id: &str,
        repository_id: &str,
    ) -> Result<
        (
            super::StoredConnection,
            Arc<dyn super::SourceModule>,
            super::ProviderToken,
            crate::domain::Repository,
        ),
        ActionFailure,
    > {
        let connection = self
            .connections
            .get(source_id)
            .map_err(|_| ActionFailure::StorageUnavailable)?
            .ok_or(ActionFailure::NotFound)?;
        let module = self
            .registry
            .get(&connection.source_id)
            .ok_or(ActionFailure::NotFound)?;
        let repository = self
            .data
            .repositories(source_id, RefreshMode::CacheFirst)
            .await
            .map_err(|failure| match failure {
                crate::source_data::SourceDataFailure::Source(failure) => {
                    ActionFailure::Source(failure)
                }
                crate::source_data::SourceDataFailure::StorageUnavailable => {
                    ActionFailure::StorageUnavailable
                }
            })?
            .and_then(|repos| repos.into_iter().find(|repo| repo.id == repository_id))
            .ok_or(ActionFailure::NotFound)?;
        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| ActionFailure::StorageUnavailable)?;
        Ok((connection, module, token, repository))
    }

    pub async fn options(
        &self,
        source_id: &str,
        repository_id: &str,
        target: &ActionTarget,
    ) -> Result<ActionOptions, ActionFailure> {
        let (connection, module, token, repository) =
            self.resolve(source_id, repository_id).await?;
        module
            .action_options(&connection.configuration, &token, &repository, target)
            .await
    }

    pub async fn execute(
        &self,
        source_id: &str,
        repository_id: &str,
        target: &ActionTarget,
        action: SourceAction,
        revision: Option<&str>,
    ) -> Result<ActionRefresh, ActionFailure> {
        let key = (
            source_id.to_owned(),
            repository_id.to_owned(),
            target.clone(),
        );
        let _permit = ActionPermit::acquire(&self.active, key)?;
        let (connection, module, token, repository) =
            self.resolve(source_id, repository_id).await?;
        let options = module
            .action_options(&connection.configuration, &token, &repository, target)
            .await?;
        let available = options
            .actions
            .iter()
            .find(|option| option.action == action)
            .ok_or(ActionFailure::Unsupported)?;
        if available.disabled_reason.is_some() || options.revision.as_deref() != revision {
            return Err(ActionFailure::Conflict);
        }
        let refresh_target = module
            .execute_action(
                &connection.configuration,
                &token,
                &repository,
                target,
                action,
                revision,
            )
            .await?;
        // A refresh failure must never turn an accepted write into a retryable failure.
        let refresh = match refresh_target.as_ref().unwrap_or(target) {
            ActionTarget::WorkflowRun {
                run_id,
            } => self
                .data
                .workflow_run(source_id, &repository, run_id)
                .await
                .map(|run| ActionRefresh {
                    run,
                    ..ActionRefresh::default()
                }),
            ActionTarget::ChangeRequest {
                number,
            } => self
                .data
                .change_request_details(source_id, &repository, *number, RefreshMode::Force)
                .await
                .map(|details| ActionRefresh {
                    change_request: details.as_ref().map(|details| {
                        super::DiscoveredChangeRequest {
                            source: super::ConnectedSource {
                                id: connection.id,
                                account_id: connection.account.external_id,
                                descriptor: module.descriptor(),
                                label: connection.label,
                            },
                            repository,
                            change_request: details.change_request.clone(),
                        }
                    }),
                    details,
                    ..ActionRefresh::default()
                }),
        };
        if refresh.is_err() {
            eprintln!("action accepted; source data refresh will be retried by synchronization");
        }
        Ok(refresh.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests;
