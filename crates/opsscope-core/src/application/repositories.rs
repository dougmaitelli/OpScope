use super::{
    ConnectedSource, ConnectionRepository, ConnectionValidationFailure, PersistenceFailure,
};
use crate::domain::Repository;
use crate::source_data::{RefreshMode, SourceData, SourceDataFailure};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

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
