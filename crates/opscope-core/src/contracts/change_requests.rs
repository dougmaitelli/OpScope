use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChangeRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChangeRequestReviewStatus {
    None,
    Approved,
    ChangesRequested,
    ReviewRequired,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChangeRequestCheckStatus {
    None,
    Passed,
    Failing,
    Running,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChangeRequestMergeStatus {
    Ready,
    Blocked,
    Conflicting,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequestSummary {
    pub id: String,
    #[ts(type = "number")]
    pub number: u64,
    pub title: String,
    pub author: Option<String>,
    pub source_branch: String,
    pub target_branch: String,
    pub state: ChangeRequestState,
    pub draft: bool,
    pub review_status: ChangeRequestReviewStatus,
    pub check_status: ChangeRequestCheckStatus,
    pub merge_status: ChangeRequestMergeStatus,
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

impl From<DiscoveredChangeRequest> for ChangeRequestSummary {
    fn from(discovered: DiscoveredChangeRequest) -> Self {
        let DomainChangeRequest {
            id,
            number,
            title,
            author,
            source_branch,
            target_branch,
            state,
            draft,
            review_status,
            check_status,
            merge_status,
            created_at,
            updated_at,
            web_url,
            ..
        } = discovered.change_request;
        Self {
            id,
            number,
            title,
            author,
            source_branch,
            target_branch,
            state: match state {
                DomainChangeRequestState::Open => ChangeRequestState::Open,
                DomainChangeRequestState::Closed => ChangeRequestState::Closed,
                DomainChangeRequestState::Merged => ChangeRequestState::Merged,
            },
            draft,
            review_status: match review_status {
                DomainChangeRequestReviewStatus::None => ChangeRequestReviewStatus::None,
                DomainChangeRequestReviewStatus::Approved => ChangeRequestReviewStatus::Approved,
                DomainChangeRequestReviewStatus::ChangesRequested => {
                    ChangeRequestReviewStatus::ChangesRequested
                }
                DomainChangeRequestReviewStatus::ReviewRequired => {
                    ChangeRequestReviewStatus::ReviewRequired
                }
                DomainChangeRequestReviewStatus::Unknown => ChangeRequestReviewStatus::Unknown,
            },
            check_status: match check_status {
                DomainChangeRequestCheckStatus::None => ChangeRequestCheckStatus::None,
                DomainChangeRequestCheckStatus::Passed => ChangeRequestCheckStatus::Passed,
                DomainChangeRequestCheckStatus::Failing => ChangeRequestCheckStatus::Failing,
                DomainChangeRequestCheckStatus::Running => ChangeRequestCheckStatus::Running,
                DomainChangeRequestCheckStatus::Unknown => ChangeRequestCheckStatus::Unknown,
            },
            merge_status: match merge_status {
                DomainChangeRequestMergeStatus::Ready => ChangeRequestMergeStatus::Ready,
                DomainChangeRequestMergeStatus::Blocked => ChangeRequestMergeStatus::Blocked,
                DomainChangeRequestMergeStatus::Conflicting => {
                    ChangeRequestMergeStatus::Conflicting
                }
                DomainChangeRequestMergeStatus::Unknown => ChangeRequestMergeStatus::Unknown,
            },
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
pub struct ListChangeRequestsResponse {
    pub selected_repository_count: usize,
    pub change_requests: Vec<ChangeRequestSummary>,
}

impl ListChangeRequestsResponse {
    #[must_use]
    pub fn from_domain(inventory: ChangeRequestInventory) -> Self {
        Self {
            selected_repository_count: inventory.selected_repository_count,
            change_requests: inventory
                .change_requests
                .into_iter()
                .map(ChangeRequestSummary::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequestDetailsRequest {
    pub source_id: String,
    pub repository_id: String,
    #[ts(type = "number")]
    pub number: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequestReviewSummary {
    pub reviewer: Option<String>,
    pub status: ChangeRequestReviewStatus,
    pub submitted_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequestCheckSummary {
    pub name: String,
    pub status: ChangeRequestCheckStatus,
    pub web_url: Option<String>,
    pub workflow_run_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequestCommitSummary {
    pub sha: String,
    pub title: String,
    pub author: Option<String>,
    pub committed_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequestDetailsResponse {
    pub body: Option<String>,
    pub labels: Vec<String>,
    pub reviews: Vec<ChangeRequestReviewSummary>,
    pub checks: Vec<ChangeRequestCheckSummary>,
    pub latest_commit: Option<ChangeRequestCommitSummary>,
}

impl From<DomainChangeRequestDetails> for ChangeRequestDetailsResponse {
    fn from(details: DomainChangeRequestDetails) -> Self {
        Self {
            body: details.body,
            labels: details.labels,
            reviews: details
                .reviews
                .into_iter()
                .map(|review| ChangeRequestReviewSummary {
                    reviewer: review.reviewer,
                    status: match review.status {
                        DomainChangeRequestReviewStatus::None => ChangeRequestReviewStatus::None,
                        DomainChangeRequestReviewStatus::Approved => {
                            ChangeRequestReviewStatus::Approved
                        }
                        DomainChangeRequestReviewStatus::ChangesRequested => {
                            ChangeRequestReviewStatus::ChangesRequested
                        }
                        DomainChangeRequestReviewStatus::ReviewRequired => {
                            ChangeRequestReviewStatus::ReviewRequired
                        }
                        DomainChangeRequestReviewStatus::Unknown => {
                            ChangeRequestReviewStatus::Unknown
                        }
                    },
                    submitted_at: review.submitted_at,
                })
                .collect(),
            checks: details
                .checks
                .into_iter()
                .map(|check| ChangeRequestCheckSummary {
                    name: check.name,
                    status: match check.status {
                        DomainChangeRequestCheckStatus::None => ChangeRequestCheckStatus::None,
                        DomainChangeRequestCheckStatus::Passed => ChangeRequestCheckStatus::Passed,
                        DomainChangeRequestCheckStatus::Failing => {
                            ChangeRequestCheckStatus::Failing
                        }
                        DomainChangeRequestCheckStatus::Running => {
                            ChangeRequestCheckStatus::Running
                        }
                        DomainChangeRequestCheckStatus::Unknown => {
                            ChangeRequestCheckStatus::Unknown
                        }
                    },
                    web_url: check.web_url,
                    workflow_run_id: check.workflow_run_id,
                })
                .collect(),
            latest_commit: details
                .latest_commit
                .map(|commit| ChangeRequestCommitSummary {
                    sha: commit.sha,
                    title: commit.title,
                    author: commit.author,
                    committed_at: commit.committed_at,
                }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequestDetailsErrorResponse {
    pub message: String,
}

impl From<GetChangeRequestDetailsFailure> for ChangeRequestDetailsErrorResponse {
    fn from(failure: GetChangeRequestDetailsFailure) -> Self {
        let message = match failure {
            GetChangeRequestDetailsFailure::UnknownSource => {
                "This source module is not registered."
            }
            GetChangeRequestDetailsFailure::SourceNotConnected => {
                "This source is no longer connected."
            }
            GetChangeRequestDetailsFailure::Unsupported => {
                "This source does not provide change request details."
            }
            GetChangeRequestDetailsFailure::RepositoryNotFound => {
                "This repository is no longer available. Refresh the page."
            }
            GetChangeRequestDetailsFailure::ChangeRequestNotFound => {
                "This change request is no longer available. Refresh the page."
            }
            GetChangeRequestDetailsFailure::InvalidCredentials => {
                "The source rejected the stored credential."
            }
            GetChangeRequestDetailsFailure::PermissionDenied => {
                "The source credential cannot read change request details."
            }
            GetChangeRequestDetailsFailure::RateLimited => {
                "The source rate limit was reached. Try again later."
            }
            GetChangeRequestDetailsFailure::ProviderUnavailable => {
                "The source could not be reached. Try again."
            }
            GetChangeRequestDetailsFailure::UnexpectedResponse => {
                "The source returned change request details in an unexpected format."
            }
            GetChangeRequestDetailsFailure::StorageUnavailable => {
                "The stored source data could not be read."
            }
        };
        Self {
            message: message.to_owned(),
        }
    }
}
