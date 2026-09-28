use crate::application::{DiscoveredIssue, GetIssueDetailsFailure, IssueInventory};
use crate::domain::{
    Issue as DomainIssue, IssueComment as DomainIssueComment, IssueDetails as DomainIssueDetails,
    IssueState as DomainIssueState,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum IssueState {
    Open,
    Closed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IssueSummary {
    pub id: String,
    #[ts(type = "number")]
    pub number: u64,
    pub title: String,
    pub author: Option<String>,
    pub state: IssueState,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    #[ts(type = "number")]
    pub comment_count: u64,
    pub created_at: String,
    pub updated_at: String,
    pub web_url: String,
    pub source_id: String,
    pub source_name: String,
    pub source_abbreviation: String,
    pub repository_id: String,
    pub repository_owner: String,
    pub repository_name: String,
}

impl From<DiscoveredIssue> for IssueSummary {
    fn from(discovered: DiscoveredIssue) -> Self {
        let DomainIssue {
            id,
            number,
            title,
            author,
            state,
            labels,
            assignees,
            comment_count,
            created_at,
            updated_at,
            web_url,
        } = discovered.issue;
        Self {
            id,
            number,
            title,
            author,
            state: match state {
                DomainIssueState::Open => IssueState::Open,
                DomainIssueState::Closed => IssueState::Closed,
            },
            labels,
            assignees,
            comment_count,
            created_at,
            updated_at,
            web_url,
            source_id: discovered.source.id,
            source_name: discovered.source.label,
            source_abbreviation: discovered.source.descriptor.abbreviation,
            repository_id: discovered.repository.id,
            repository_owner: discovered.repository.owner,
            repository_name: discovered.repository.name,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListIssuesResponse {
    pub selected_repository_count: usize,
    pub issues: Vec<IssueSummary>,
}

impl ListIssuesResponse {
    #[must_use]
    pub fn from_domain(inventory: IssueInventory) -> Self {
        Self {
            selected_repository_count: inventory.selected_repository_count,
            issues: inventory
                .issues
                .into_iter()
                .map(IssueSummary::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IssueDetailsRequest {
    pub source_id: String,
    pub repository_id: String,
    #[ts(type = "number")]
    pub number: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IssueCommentSummary {
    pub id: String,
    pub author: Option<String>,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<DomainIssueComment> for IssueCommentSummary {
    fn from(comment: DomainIssueComment) -> Self {
        Self {
            id: comment.id,
            author: comment.author,
            body: comment.body,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IssueDetailsResponse {
    pub title: String,
    pub state: IssueState,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    #[ts(type = "number")]
    pub comment_count: u64,
    pub updated_at: String,
    pub body: Option<String>,
    pub milestone: Option<String>,
    pub comments: Vec<IssueCommentSummary>,
}

impl From<DomainIssueDetails> for IssueDetailsResponse {
    fn from(details: DomainIssueDetails) -> Self {
        Self {
            title: details.issue.title,
            state: match details.issue.state {
                DomainIssueState::Open => IssueState::Open,
                DomainIssueState::Closed => IssueState::Closed,
            },
            labels: details.issue.labels,
            assignees: details.issue.assignees,
            comment_count: details.issue.comment_count,
            updated_at: details.issue.updated_at,
            body: details.body,
            milestone: details.milestone,
            comments: details.comments.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IssueDetailsErrorResponse {
    pub message: String,
}

impl From<GetIssueDetailsFailure> for IssueDetailsErrorResponse {
    fn from(failure: GetIssueDetailsFailure) -> Self {
        let message = match failure {
            GetIssueDetailsFailure::UnknownSource => "This source module is not registered.",
            GetIssueDetailsFailure::SourceNotConnected => "This source is no longer connected.",
            GetIssueDetailsFailure::Unsupported => "This source does not provide issue details.",
            GetIssueDetailsFailure::RepositoryNotFound => {
                "This repository is no longer available. Refresh the page."
            }
            GetIssueDetailsFailure::IssueNotFound => {
                "This issue is no longer available. Refresh the page."
            }
            GetIssueDetailsFailure::InvalidCredentials => {
                "The source rejected the stored credential."
            }
            GetIssueDetailsFailure::PermissionDenied => {
                "The source credential cannot read issue details."
            }
            GetIssueDetailsFailure::RateLimited => {
                "The source rate limit was reached. Try again later."
            }
            GetIssueDetailsFailure::ProviderUnavailable => {
                "The source could not be reached. Try again."
            }
            GetIssueDetailsFailure::UnexpectedResponse => {
                "The source returned issue details in an unexpected format."
            }
            GetIssueDetailsFailure::StorageUnavailable => {
                "The stored source data could not be read."
            }
        };
        Self {
            message: message.to_owned(),
        }
    }
}
