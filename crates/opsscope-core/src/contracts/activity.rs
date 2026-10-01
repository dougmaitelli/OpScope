use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChangeRequestActivityKind {
    Opened,
    ReadyForReview,
    ReviewApproved,
    ChangesRequested,
    ChecksFailed,
    ChecksRecovered,
    ConflictDetected,
    ConflictResolved,
    Merged,
    Closed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRequestActivitySummary {
    pub id: String,
    pub kind: ChangeRequestActivityKind,
    pub occurred_at: String,
    pub change_request: ChangeRequestSummary,
}

impl From<DomainChangeRequestActivityEvent> for ChangeRequestActivitySummary {
    fn from(event: DomainChangeRequestActivityEvent) -> Self {
        let change_request = event.change_request;
        Self {
            id: event.id,
            kind: match event.kind {
                DomainChangeRequestActivityKind::Opened => ChangeRequestActivityKind::Opened,
                DomainChangeRequestActivityKind::ReadyForReview => {
                    ChangeRequestActivityKind::ReadyForReview
                }
                DomainChangeRequestActivityKind::ReviewApproved => {
                    ChangeRequestActivityKind::ReviewApproved
                }
                DomainChangeRequestActivityKind::ChangesRequested => {
                    ChangeRequestActivityKind::ChangesRequested
                }
                DomainChangeRequestActivityKind::ChecksFailed => {
                    ChangeRequestActivityKind::ChecksFailed
                }
                DomainChangeRequestActivityKind::ChecksRecovered => {
                    ChangeRequestActivityKind::ChecksRecovered
                }
                DomainChangeRequestActivityKind::ConflictDetected => {
                    ChangeRequestActivityKind::ConflictDetected
                }
                DomainChangeRequestActivityKind::ConflictResolved => {
                    ChangeRequestActivityKind::ConflictResolved
                }
                DomainChangeRequestActivityKind::Merged => ChangeRequestActivityKind::Merged,
                DomainChangeRequestActivityKind::Closed => ChangeRequestActivityKind::Closed,
            },
            occurred_at: event.occurred_at,
            change_request: ChangeRequestSummary {
                id: change_request.id,
                number: change_request.number,
                title: change_request.title,
                author: change_request.author,
                source_branch: change_request.source_branch,
                target_branch: change_request.target_branch,
                state: match change_request.state {
                    DomainChangeRequestState::Open => ChangeRequestState::Open,
                    DomainChangeRequestState::Closed => ChangeRequestState::Closed,
                    DomainChangeRequestState::Merged => ChangeRequestState::Merged,
                },
                draft: change_request.draft,
                review_status: match change_request.review_status {
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
                    DomainChangeRequestReviewStatus::Unknown => ChangeRequestReviewStatus::Unknown,
                },
                check_status: match change_request.check_status {
                    DomainChangeRequestCheckStatus::None => ChangeRequestCheckStatus::None,
                    DomainChangeRequestCheckStatus::Passed => ChangeRequestCheckStatus::Passed,
                    DomainChangeRequestCheckStatus::Failing => ChangeRequestCheckStatus::Failing,
                    DomainChangeRequestCheckStatus::Running => ChangeRequestCheckStatus::Running,
                    DomainChangeRequestCheckStatus::Unknown => ChangeRequestCheckStatus::Unknown,
                },
                merge_status: match change_request.merge_status {
                    DomainChangeRequestMergeStatus::Ready => ChangeRequestMergeStatus::Ready,
                    DomainChangeRequestMergeStatus::Blocked => ChangeRequestMergeStatus::Blocked,
                    DomainChangeRequestMergeStatus::Conflicting => {
                        ChangeRequestMergeStatus::Conflicting
                    }
                    DomainChangeRequestMergeStatus::Unknown => ChangeRequestMergeStatus::Unknown,
                },
                created_at: change_request.created_at,
                updated_at: change_request.updated_at,
                web_url: change_request.web_url,
                source_id: event.source_id,
                source_name: event.source_name,
                source_abbreviation: event.source_abbreviation,
                repository_id: event.repository_id,
                repository_owner: event.repository_owner,
                repository_name: event.repository_name,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListActivityResponse {
    pub change_request_events: Vec<ChangeRequestActivitySummary>,
    pub issue_events: Vec<IssueActivitySummary>,
}

impl ListActivityResponse {
    #[must_use]
    pub fn from_domain(events: crate::application::ActivityInventory) -> Self {
        Self {
            change_request_events: events
                .change_request_events
                .into_iter()
                .map(Into::into)
                .collect(),
            issue_events: events.issue_events.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum IssueActivityKind {
    Opened,
    Updated,
    Closed,
    Reopened,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct IssueActivitySummary {
    pub id: String,
    pub kind: IssueActivityKind,
    pub occurred_at: String,
    pub issue: IssueSummary,
}

impl From<crate::application::IssueActivityEvent> for IssueActivitySummary {
    fn from(event: crate::application::IssueActivityEvent) -> Self {
        let issue = event.issue;
        Self {
            id: event.id,
            occurred_at: event.occurred_at,
            kind: match event.kind {
                crate::application::IssueActivityKind::Opened => IssueActivityKind::Opened,
                crate::application::IssueActivityKind::Updated => IssueActivityKind::Updated,
                crate::application::IssueActivityKind::Closed => IssueActivityKind::Closed,
                crate::application::IssueActivityKind::Reopened => IssueActivityKind::Reopened,
            },
            issue: IssueSummary {
                id: issue.id,
                number: issue.number,
                title: issue.title,
                author: issue.author,
                state: match issue.state {
                    crate::domain::IssueState::Open => IssueState::Open,
                    crate::domain::IssueState::Closed => IssueState::Closed,
                },
                labels: issue.labels,
                assignees: issue.assignees,
                comment_count: issue.comment_count,
                created_at: issue.created_at,
                updated_at: issue.updated_at,
                web_url: issue.web_url,
                source_id: event.source_id,
                source_name: event.source_name,
                source_abbreviation: event.source_abbreviation,
                repository_id: event.repository_id,
                repository_owner: event.repository_owner,
                repository_name: event.repository_name,
            },
        }
    }
}
