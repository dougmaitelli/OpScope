//! Transparent access to provider data with optional persistent read-through caching.

use crate::application::{
    ConnectionRepository, ConnectionValidationFailure, PersistenceFailure, SecretStore,
    SourceDescriptor, SourceRegistry,
};
use crate::domain::{Repository, Workflow};
use async_trait::async_trait;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_SOURCE_DATA_MAX_AGE: Duration = Duration::from_secs(60);

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
    fn sources(&self) -> Vec<SourceDescriptor>;

    async fn repositories(
        &self,
        source_id: &str,
    ) -> Result<Option<Vec<Repository>>, SourceDataFailure>;

    async fn workflows(
        &self,
        source_id: &str,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, SourceDataFailure>;
}

#[derive(Clone)]
pub struct ReadThroughSourceData {
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
    cache: Option<Arc<dyn SourceDataCache>>,
    max_age: Duration,
}

impl ReadThroughSourceData {
    #[must_use]
    pub fn cached(
        registry: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        secrets: Arc<dyn SecretStore>,
        cache: Arc<dyn SourceDataCache>,
        max_age: Duration,
    ) -> Self {
        Self {
            registry,
            connections,
            secrets,
            cache: Some(cache),
            max_age,
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
            max_age: Duration::ZERO,
        }
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn is_fresh(&self, refreshed_at: u64, now: u64) -> bool {
        now.saturating_sub(refreshed_at) < self.max_age.as_secs()
    }
}

#[async_trait]
impl SourceData for ReadThroughSourceData {
    fn sources(&self) -> Vec<SourceDescriptor> {
        self.registry.descriptors()
    }

    async fn repositories(
        &self,
        source_id: &str,
    ) -> Result<Option<Vec<Repository>>, SourceDataFailure> {
        let module = self
            .registry
            .get(source_id)
            .ok_or(SourceDataFailure::Source(
                ConnectionValidationFailure::UnexpectedResponse,
            ))?;
        let Some(connection) = self
            .connections
            .get(source_id)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?
        else {
            return Ok(None);
        };
        let now = Self::now();

        if let Some(cache) = &self.cache
            && let Ok(Some(snapshot)) =
                cache.repositories(source_id, &connection.account.external_id)
            && self.is_fresh(snapshot.refreshed_at, now)
        {
            return Ok(Some(snapshot.repositories));
        }

        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?;
        let repositories = module
            .list_repositories(&token)
            .await
            .map_err(SourceDataFailure::Source)?;

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
    ) -> Result<Vec<Workflow>, SourceDataFailure> {
        let module = self
            .registry
            .get(source_id)
            .ok_or(SourceDataFailure::Source(
                ConnectionValidationFailure::UnexpectedResponse,
            ))?;
        let connection = self
            .connections
            .get(source_id)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?
            .ok_or(SourceDataFailure::StorageUnavailable)?;
        let now = Self::now();

        if let Some(cache) = &self.cache
            && let Ok(Some(snapshot)) =
                cache.workflows(source_id, &connection.account.external_id, &repository.id)
            && self.is_fresh(snapshot.refreshed_at, now)
        {
            return Ok(snapshot.workflows);
        }

        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?;
        let workflows = module
            .list_workflows(&token, repository)
            .await
            .map_err(SourceDataFailure::Source)?;

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
}

#[cfg(test)]
mod tests;
