use super::{
    ConnectedSource, ConnectionValidationFailure, RepositorySelectionRepository,
    SettingsRepository, SourceCapability,
};
use crate::domain::{ChangeRequest, Repository};
use crate::source_data::{RefreshMode, SourceData, SourceDataFailure};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

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
    settings: Arc<dyn SettingsRepository>,
    source_data: Arc<dyn SourceData>,
    selections: Arc<dyn RepositorySelectionRepository>,
}

impl ListChangeRequests {
    #[must_use]
    pub fn new(
        source_data: Arc<dyn SourceData>,
        selections: Arc<dyn RepositorySelectionRepository>,
        settings: Arc<dyn SettingsRepository>,
    ) -> Self {
        Self {
            settings,
            source_data,
            selections,
        }
    }

    pub async fn execute(&self) -> Result<ChangeRequestInventory, ListChangeRequestsFailure> {
        let settings = self
            .settings
            .load_settings()
            .map_err(|_| ListChangeRequestsFailure::StorageUnavailable)?;
        let only_my_work = settings.only_my_work;
        let selections = self
            .selections
            .list()
            .map_err(|_| ListChangeRequestsFailure::StorageUnavailable)?;
        let selected_repository_count = selections.len();
        if !settings.pull_requests_enabled {
            return Ok(ChangeRequestInventory {
                selected_repository_count,
                change_requests: Vec::new(),
            });
        }
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
        change_requests.retain(|item| {
            !only_my_work
                || item
                    .change_request
                    .relationships
                    .evaluate(&item.source.account_id)
                    .matches()
        });
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
