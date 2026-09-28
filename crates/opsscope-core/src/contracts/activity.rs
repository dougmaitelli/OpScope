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
}

impl ListActivityResponse {
    #[must_use]
    pub fn from_domain(events: Vec<DomainChangeRequestActivityEvent>) -> Self {
        Self {
            change_request_events: events.into_iter().map(Into::into).collect(),
        }
    }
}
