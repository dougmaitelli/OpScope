//! Transparent access to provider data with optional persistent read-through caching.

use crate::application::{
    ConnectedSource, ConnectionRepository, ConnectionValidationFailure, PersistenceFailure,
    SecretStore, SourceRegistry, StoredConnection, WorkflowRunLogsFailure,
};
use crate::domain::{Repository, Workflow, WorkflowRun, WorkflowRunLogs};
use async_trait::async_trait;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_REPOSITORY_CACHE_MAX_AGE: Duration = Duration::from_secs(15 * 60);
pub const DEFAULT_WORKFLOW_CACHE_MAX_AGE: Duration = Duration::from_secs(60);
pub const DEFAULT_WORKFLOW_RUN_CACHE_MAX_AGE: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceDataCachePolicy {
    pub repositories: Duration,
    pub workflows: Duration,
    pub workflow_runs: Duration,
}

impl SourceDataCachePolicy {
    pub const DEFAULT: Self = Self {
        repositories: DEFAULT_REPOSITORY_CACHE_MAX_AGE,
        workflows: DEFAULT_WORKFLOW_CACHE_MAX_AGE,
        workflow_runs: DEFAULT_WORKFLOW_RUN_CACHE_MAX_AGE,
    };

    #[must_use]
    pub const fn uniform(max_age: Duration) -> Self {
        Self {
            repositories: max_age,
            workflows: max_age,
            workflow_runs: max_age,
        }
    }
}

impl Default for SourceDataCachePolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositorySnapshot {
    pub refreshed_at: u64,
    pub repositories: Vec<Repository>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowSnapshot {
    pub refreshed_at: u64,
    pub workflows: Vec<Workflow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowRunSnapshot {
    pub last_attempted_at: u64,
    pub last_successful_at: Option<u64>,
    pub last_error: Option<ConnectionValidationFailure>,
    pub runs: Vec<WorkflowRun>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowRunCollection {
    pub runs: Vec<WorkflowRun>,
    pub last_attempted_at: u64,
    pub last_successful_at: u64,
    pub stale: bool,
    pub error: Option<ConnectionValidationFailure>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshMode {
    CacheFirst,
    IfStale,
    Force,
}

pub trait SourceDataCache: Send + Sync {
    fn repositories(
        &self,
        source_id: &str,
        account_id: &str,
    ) -> Result<Option<RepositorySnapshot>, PersistenceFailure>;

    fn replace_repositories(
        &self,
        source_id: &str,
        account_id: &str,
        snapshot: &RepositorySnapshot,
    ) -> Result<(), PersistenceFailure>;

    fn workflows(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
    ) -> Result<Option<WorkflowSnapshot>, PersistenceFailure>;

    fn replace_workflows(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        snapshot: &WorkflowSnapshot,
    ) -> Result<(), PersistenceFailure>;

    fn workflow_runs(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
    ) -> Result<Option<WorkflowRunSnapshot>, PersistenceFailure>;

    fn replace_workflow_runs(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        snapshot: &WorkflowRunSnapshot,
    ) -> Result<(), PersistenceFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceDataFailure {
    Source(ConnectionValidationFailure),
    StorageUnavailable,
}

impl Display for SourceDataFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(failure) => Display::fmt(failure, formatter),
            Self::StorageUnavailable => formatter.write_str("source data storage unavailable"),
        }
    }
}

impl Error for SourceDataFailure {}

#[async_trait]
pub trait SourceData: Send + Sync {
    fn sources(&self) -> Result<Vec<ConnectedSource>, SourceDataFailure>;

    async fn repositories(
        &self,
        source_id: &str,
        refresh: RefreshMode,
    ) -> Result<Option<Vec<Repository>>, SourceDataFailure>;

    async fn workflows(
        &self,
        source_id: &str,
        repository: &Repository,
        refresh: RefreshMode,
    ) -> Result<Vec<Workflow>, SourceDataFailure>;

    async fn workflow_runs(
        &self,
        source_id: &str,
        repository: &Repository,
        refresh: RefreshMode,
    ) -> Result<WorkflowRunCollection, SourceDataFailure>;

    async fn workflow_run_logs(
        &self,
        source_id: &str,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure>;
}

#[derive(Clone)]
pub struct ReadThroughSourceData {
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
    cache: Option<Arc<dyn SourceDataCache>>,
    cache_policy: SourceDataCachePolicy,
}

impl ReadThroughSourceData {
    #[must_use]
    pub fn cached(
        registry: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        secrets: Arc<dyn SecretStore>,
        cache: Arc<dyn SourceDataCache>,
        cache_policy: SourceDataCachePolicy,
    ) -> Self {
        Self {
            registry,
            connections,
            secrets,
            cache: Some(cache),
            cache_policy,
        }
    }

    #[must_use]
    pub fn uncached(
        registry: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            registry,
            connections,
            secrets,
            cache: None,
            cache_policy: SourceDataCachePolicy::uniform(Duration::ZERO),
        }
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn is_fresh(refreshed_at: u64, now: u64, max_age: Duration) -> bool {
        now.saturating_sub(refreshed_at) < max_age.as_secs()
    }

    fn connection(
        &self,
        connection_id: &str,
    ) -> Result<(StoredConnection, Arc<dyn crate::application::SourceModule>), SourceDataFailure>
    {
        let connection = self
            .connections
            .get(connection_id)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?
            .ok_or(SourceDataFailure::StorageUnavailable)?;
        let module = self
            .registry
            .get(&connection.source_id)
            .ok_or(SourceDataFailure::Source(
                ConnectionValidationFailure::UnexpectedResponse,
            ))?;
        Ok((connection, module))
    }
}

#[async_trait]
impl SourceData for ReadThroughSourceData {
    fn sources(&self) -> Result<Vec<ConnectedSource>, SourceDataFailure> {
        self.connections
            .list()
            .map_err(|_| SourceDataFailure::StorageUnavailable)?
            .into_iter()
            .map(|connection| {
                let descriptor =
                    self.registry
                        .get(&connection.source_id)
                        .ok_or(SourceDataFailure::Source(
                            ConnectionValidationFailure::UnexpectedResponse,
                        ))?;
                Ok(ConnectedSource {
                    id: connection.id,
                    descriptor: descriptor.descriptor(),
                    label: connection.label,
                })
            })
            .collect()
    }

    async fn repositories(
        &self,
        source_id: &str,
        refresh: RefreshMode,
    ) -> Result<Option<Vec<Repository>>, SourceDataFailure> {
        let (connection, module) = match self.connection(source_id) {
            Ok(connection) => connection,
            Err(SourceDataFailure::StorageUnavailable) => return Ok(None),
            Err(failure) => return Err(failure),
        };
        let now = Self::now();

        let cached = self.cache.as_ref().and_then(|cache| {
            cache
                .repositories(source_id, &connection.account.external_id)
                .ok()
                .flatten()
        });
        if let Some(snapshot) = &cached
            && (refresh == RefreshMode::CacheFirst
                || refresh == RefreshMode::IfStale
                    && Self::is_fresh(snapshot.refreshed_at, now, self.cache_policy.repositories))
        {
            return Ok(Some(snapshot.repositories.clone()));
        }

        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?;
        let repositories = match module
            .list_repositories(&connection.configuration, &token)
            .await
        {
            Ok(repositories) => repositories,
            Err(_) if refresh == RefreshMode::IfStale && cached.is_some() => {
                return Ok(cached.map(|snapshot| snapshot.repositories));
            }
            Err(failure) => return Err(SourceDataFailure::Source(failure)),
        };

        if let Some(cache) = &self.cache {
            _ = cache.replace_repositories(
                source_id,
                &connection.account.external_id,
                &RepositorySnapshot {
                    refreshed_at: Self::now(),
                    repositories: repositories.clone(),
                },
            );
        }

        Ok(Some(repositories))
    }

    async fn workflows(
        &self,
        source_id: &str,
        repository: &Repository,
        refresh: RefreshMode,
    ) -> Result<Vec<Workflow>, SourceDataFailure> {
        let (connection, module) = self.connection(source_id)?;
        let now = Self::now();

        let cached = self.cache.as_ref().and_then(|cache| {
            cache
                .workflows(source_id, &connection.account.external_id, &repository.id)
                .ok()
                .flatten()
        });
        if let Some(snapshot) = &cached
            && (refresh == RefreshMode::CacheFirst
                || refresh == RefreshMode::IfStale
                    && Self::is_fresh(snapshot.refreshed_at, now, self.cache_policy.workflows))
        {
            return Ok(snapshot.workflows.clone());
        }

        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?;
        let workflows = match module
            .list_workflows(&connection.configuration, &token, repository)
            .await
        {
            Ok(workflows) => workflows,
            Err(_) if refresh == RefreshMode::IfStale && cached.is_some() => {
                return Ok(cached.map_or_else(Vec::new, |snapshot| snapshot.workflows));
            }
            Err(failure) => return Err(SourceDataFailure::Source(failure)),
        };

        if let Some(cache) = &self.cache {
            _ = cache.replace_workflows(
                source_id,
                &connection.account.external_id,
                &repository.id,
                &WorkflowSnapshot {
                    refreshed_at: Self::now(),
                    workflows: workflows.clone(),
                },
            );
        }

        Ok(workflows)
    }

    async fn workflow_runs(
        &self,
        source_id: &str,
        repository: &Repository,
        refresh: RefreshMode,
    ) -> Result<WorkflowRunCollection, SourceDataFailure> {
        let (connection, module) = self.connection(source_id)?;
        let now = Self::now();
        let cached = self.cache.as_ref().and_then(|cache| {
            cache
                .workflow_runs(source_id, &connection.account.external_id, &repository.id)
                .ok()
                .flatten()
        });

        if let Some(snapshot) = &cached
            && refresh != RefreshMode::Force
            && let Some(last_successful_at) = snapshot.last_successful_at
        {
            let fresh = Self::is_fresh(
                snapshot.last_attempted_at,
                now,
                self.cache_policy.workflow_runs,
            );
            if refresh == RefreshMode::CacheFirst || fresh {
                return Ok(WorkflowRunCollection {
                    runs: snapshot.runs.clone(),
                    last_attempted_at: snapshot.last_attempted_at,
                    last_successful_at,
                    stale: snapshot.last_error.is_some() || !fresh,
                    error: snapshot.last_error,
                });
            }
        }

        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?;
        match module
            .list_workflow_runs(&connection.configuration, &token, repository)
            .await
        {
            Ok(runs) => {
                let completed_at = Self::now();
                if let Some(cache) = &self.cache {
                    _ = cache.replace_workflow_runs(
                        source_id,
                        &connection.account.external_id,
                        &repository.id,
                        &WorkflowRunSnapshot {
                            last_attempted_at: completed_at,
                            last_successful_at: Some(completed_at),
                            last_error: None,
                            runs: runs.clone(),
                        },
                    );
                }
                Ok(WorkflowRunCollection {
                    runs,
                    last_attempted_at: completed_at,
                    last_successful_at: completed_at,
                    stale: false,
                    error: None,
                })
            }
            Err(failure) => {
                let attempted_at = Self::now();
                if let Some(cache) = &self.cache {
                    _ = cache.replace_workflow_runs(
                        source_id,
                        &connection.account.external_id,
                        &repository.id,
                        &WorkflowRunSnapshot {
                            last_attempted_at: attempted_at,
                            last_successful_at: cached
                                .as_ref()
                                .and_then(|snapshot| snapshot.last_successful_at),
                            last_error: Some(failure),
                            runs: cached
                                .as_ref()
                                .map_or_else(Vec::new, |snapshot| snapshot.runs.clone()),
                        },
                    );
                }
                if let Some(snapshot) = cached
                    && let Some(last_successful_at) = snapshot.last_successful_at
                {
                    return Ok(WorkflowRunCollection {
                        runs: snapshot.runs,
                        last_attempted_at: attempted_at,
                        last_successful_at,
                        stale: true,
                        error: Some(failure),
                    });
                }
                Err(SourceDataFailure::Source(failure))
            }
        }
    }

    async fn workflow_run_logs(
        &self,
        source_id: &str,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        let connection = self
            .connections
            .get(source_id)
            .map_err(|_| WorkflowRunLogsFailure::StorageUnavailable)?
            .ok_or(WorkflowRunLogsFailure::SourceNotConnected)?;
        let module = self
            .registry
            .get(&connection.source_id)
            .ok_or(WorkflowRunLogsFailure::UnknownSource)?;
        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| WorkflowRunLogsFailure::StorageUnavailable)?;

        module
            .workflow_run_logs(&connection.configuration, &token, repository, run)
            .await
    }
}

#[cfg(test)]
mod tests;
