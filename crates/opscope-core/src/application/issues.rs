use super::{
    ConnectedSource, ConnectionValidationFailure, RepositorySelectionRepository,
    SettingsRepository, SourceCapability,
};
use crate::domain::{Issue, IssueDetails, Repository};
use crate::source_data::{RefreshMode, SourceData, SourceDataFailure};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredIssue {
    pub source: ConnectedSource,
    pub repository: Repository,
    pub issue: Issue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueInventory {
    pub selected_repository_count: usize,
    pub issues: Vec<DiscoveredIssue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListIssuesFailure {
    Source(ConnectionValidationFailure),
    StorageUnavailable,
}

impl Display for ListIssuesFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(failure) => Display::fmt(failure, formatter),
            Self::StorageUnavailable => formatter.write_str("issue discovery storage unavailable"),
        }
    }
}

impl Error for ListIssuesFailure {}

#[derive(Clone)]
pub struct ListIssues {
    settings: Arc<dyn SettingsRepository>,
    source_data: Arc<dyn SourceData>,
    selections: Arc<dyn RepositorySelectionRepository>,
}

impl ListIssues {
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

    pub async fn execute(&self) -> Result<IssueInventory, ListIssuesFailure> {
        let settings = self
            .settings
            .load_settings()
            .map_err(|_| ListIssuesFailure::StorageUnavailable)?;
        let only_my_work = settings.only_my_work;
        let selections = self
            .selections
            .list()
            .map_err(|_| ListIssuesFailure::StorageUnavailable)?;
        let selected_repository_count = selections.len();
        if !settings.issues_enabled {
            return Ok(IssueInventory {
                selected_repository_count,
                issues: Vec::new(),
            });
        }
        let mut issues = Vec::new();
        for source in self.source_data.sources().map_err(list_issues_failure)? {
            if !source.descriptor.supports(SourceCapability::Issues) {
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
                .map_err(list_issues_failure)?
                .ok_or(ListIssuesFailure::StorageUnavailable)?;
            for repository in repositories
                .into_iter()
                .filter(|repository| selected_ids.contains(&repository.id))
            {
                let Some(repository_issues) = self
                    .source_data
                    .issues(&source.id, &repository, RefreshMode::CacheFirst)
                    .await
                    .map_err(list_issues_failure)?
                else {
                    continue;
                };
                issues.extend(repository_issues.into_iter().map(|issue| DiscoveredIssue {
                    source: source.clone(),
                    repository: repository.clone(),
                    issue,
                }));
            }
        }
        issues.retain(|item| {
            !only_my_work
                || item
                    .issue
                    .relationships
                    .evaluate(&item.source.account_id)
                    .matches()
        });
        issues.sort_by(|left, right| right.issue.updated_at.cmp(&left.issue.updated_at));
        Ok(IssueInventory {
            selected_repository_count,
            issues,
        })
    }
}

fn list_issues_failure(failure: SourceDataFailure) -> ListIssuesFailure {
    match failure {
        SourceDataFailure::Source(failure) => ListIssuesFailure::Source(failure),
        SourceDataFailure::StorageUnavailable => ListIssuesFailure::StorageUnavailable,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GetIssueDetailsFailure {
    UnknownSource,
    SourceNotConnected,
    Unsupported,
    RepositoryNotFound,
    IssueNotFound,
    InvalidCredentials,
    PermissionDenied,
    RateLimited,
    ProviderUnavailable,
    UnexpectedResponse,
    StorageUnavailable,
}

impl Display for GetIssueDetailsFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnknownSource => "source module is not registered",
            Self::SourceNotConnected => "source is not connected",
            Self::Unsupported => "source does not support issue details",
            Self::RepositoryNotFound => "repository is not available",
            Self::IssueNotFound => "issue is no longer available",
            Self::InvalidCredentials => "provider rejected the credential",
            Self::PermissionDenied => "provider credential lacks permission to read issues",
            Self::RateLimited => "provider rate limit reached",
            Self::ProviderUnavailable => "provider unavailable",
            Self::UnexpectedResponse => "provider returned unexpected issue details",
            Self::StorageUnavailable => "stored source data could not be read",
        })
    }
}

impl Error for GetIssueDetailsFailure {}

#[derive(Clone)]
pub struct GetIssueDetails {
    source_data: Arc<dyn SourceData>,
}

impl GetIssueDetails {
    #[must_use]
    pub fn new(source_data: Arc<dyn SourceData>) -> Self {
        Self {
            source_data,
        }
    }

    pub async fn execute(
        &self,
        source_id: &str,
        repository_id: &str,
        number: u64,
    ) -> Result<IssueDetails, GetIssueDetailsFailure> {
        let source = self
            .source_data
            .sources()
            .map_err(issue_details_failure)?
            .into_iter()
            .find(|source| source.id == source_id)
            .ok_or(GetIssueDetailsFailure::UnknownSource)?;
        if !source.descriptor.supports(SourceCapability::Issues) {
            return Err(GetIssueDetailsFailure::Unsupported);
        }
        let repositories = self
            .source_data
            .repositories(source_id, RefreshMode::CacheFirst)
            .await
            .map_err(issue_details_failure)?
            .ok_or(GetIssueDetailsFailure::SourceNotConnected)?;
        let repository = repositories
            .into_iter()
            .find(|repository| repository.id == repository_id)
            .ok_or(GetIssueDetailsFailure::RepositoryNotFound)?;
        self.source_data
            .issue_details(source_id, &repository, number, RefreshMode::IfStale)
            .await
            .map_err(issue_details_failure)?
            .ok_or(GetIssueDetailsFailure::IssueNotFound)
    }
}

fn issue_details_failure(failure: SourceDataFailure) -> GetIssueDetailsFailure {
    match failure {
        SourceDataFailure::StorageUnavailable => GetIssueDetailsFailure::StorageUnavailable,
        SourceDataFailure::Source(failure) => match failure {
            ConnectionValidationFailure::InvalidConfiguration => {
                GetIssueDetailsFailure::UnexpectedResponse
            }
            ConnectionValidationFailure::InvalidCredentials => {
                GetIssueDetailsFailure::InvalidCredentials
            }
            ConnectionValidationFailure::PermissionDenied => {
                GetIssueDetailsFailure::PermissionDenied
            }
            ConnectionValidationFailure::RateLimited => GetIssueDetailsFailure::RateLimited,
            ConnectionValidationFailure::ProviderUnavailable => {
                GetIssueDetailsFailure::ProviderUnavailable
            }
            ConnectionValidationFailure::UnexpectedResponse => {
                GetIssueDetailsFailure::UnexpectedResponse
            }
        },
    }
}
