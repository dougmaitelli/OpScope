use super::*;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub(super) struct Person {
    pub id: u64,
    pub login: String,
}
#[derive(Deserialize)]
pub(super) struct Branch {
    #[serde(rename = "ref")]
    pub name: String,
    pub sha: String,
}
#[derive(Deserialize)]
pub(super) struct Label {
    pub name: String,
}
#[derive(Deserialize)]
pub(super) struct PullRequest {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub state: String,
    pub user: Option<Person>,
    pub head: Option<Branch>,
    pub base: Branch,
    pub merged: bool,
    pub mergeable: Option<bool>,
    #[serde(default)]
    pub draft: bool,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    pub body: Option<String>,
    pub labels: Option<Vec<Label>>,
    pub requested_reviewers: Option<Vec<Person>>,
}
#[derive(Deserialize)]
pub(super) struct PullReview {
    pub id: u64,
    pub user: Option<Person>,
    pub state: String,
    #[serde(default)]
    pub dismissed: bool,
    #[serde(default)]
    pub stale: bool,
    pub submitted_at: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct CombinedStatus {
    #[serde(default)]
    pub state: String,
    pub total_count: usize,
    pub statuses: Vec<CommitStatus>,
}

impl CombinedStatus {
    pub(super) fn check_status(&self) -> Check {
        if self.total_count == 0 && self.statuses.is_empty() {
            Check::None
        } else {
            check_state(&self.state)
        }
    }
}
#[derive(Deserialize)]
pub(super) struct CommitStatus {
    pub context: String,
    pub status: String,
    pub target_url: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct Commit {
    pub sha: String,
    pub commit: CommitData,
}
#[derive(Deserialize)]
pub(super) struct CommitData {
    pub message: String,
    pub author: CommitAuthor,
    pub committer: CommitAuthor,
}
#[derive(Deserialize)]
pub(super) struct CommitAuthor {
    pub name: String,
    pub date: String,
}
#[derive(Deserialize)]
pub(super) struct Runs {
    pub total_count: usize,
    pub workflow_runs: Vec<RunLink>,
}
#[derive(Deserialize)]
pub(super) struct RunLink {
    pub id: u64,
    pub head_sha: String,
    pub html_url: String,
}

impl PullReview {
    fn status(&self) -> Review {
        if self.dismissed || self.stale {
            return Review::Unknown;
        }
        match self.state.as_str() {
            "APPROVED" => Review::Approved,
            "REQUEST_CHANGES" => Review::ChangesRequested,
            "REQUEST_REVIEW" => Review::ReviewRequired,
            _ => Review::Unknown,
        }
    }
}

pub(super) fn current_reviews(
    reviews: Vec<PullReview>,
    requested: &[Person],
) -> Vec<ChangeRequestReview> {
    let mut latest = BTreeMap::new();
    for review in reviews {
        if matches!(review.state.as_str(), "COMMENT" | "PENDING") {
            continue;
        }
        if let Some(user) = &review.user {
            let id = user.id;
            if latest
                .get(&id)
                .is_none_or(|previous: &PullReview| previous.id < review.id)
            {
                latest.insert(id, review);
            }
        }
    }
    let mut result = Vec::new();
    for review in latest.values() {
        result.push(ChangeRequestReview {
            reviewer: review.user.as_ref().map(|user| user.login.clone()),
            status: review.status(),
            submitted_at: review.submitted_at.clone(),
        });
    }
    // Explicit review requests override a previous decision from that reviewer.
    for user in requested {
        result.retain(|review| review.reviewer.as_deref() != Some(&user.login));
        result.push(ChangeRequestReview {
            reviewer: Some(user.login.clone()),
            status: Review::ReviewRequired,
            submitted_at: None,
        });
    }
    result
}

pub(super) fn check_state(state: &str) -> Check {
    match state {
        "success" => Check::Passed,
        "failure" | "error" => Check::Failing,
        "pending" => Check::Running,
        _ => Check::Unknown,
    }
}

// Gitea exposes individual reviews and outstanding reviewer requests, but no
// combined review decision. Summarize only the current decisions here.
pub(super) fn review_state(reviews: &[ChangeRequestReview]) -> Review {
    if reviews.is_empty() {
        return Review::None;
    }
    for status in [
        Review::ChangesRequested,
        Review::ReviewRequired,
        Review::Approved,
    ] {
        if reviews.iter().any(|review| review.status == status) {
            return status;
        }
    }
    Review::Unknown
}

impl CommitStatus {
    pub(super) fn into_check(self) -> ChangeRequestCheck {
        let status = check_state(&self.status);
        ChangeRequestCheck {
            name: self.context,
            status,
            web_url: self.target_url.filter(|url| !url.is_empty()),
            workflow_run_id: None,
        }
    }
}

impl PullRequest {
    pub(super) fn relationships(&self, reviews: &[PullReview]) -> Relationships {
        use crate::domain::AccountSet;
        Relationships {
            authors: AccountSet::new(
                self.user.iter().map(|user| user.id.to_string()),
                self.user.is_some(),
            ),
            reviewers: AccountSet::new(
                reviews
                    .iter()
                    .filter(|review| {
                        matches!(
                            review.state.as_str(),
                            "APPROVED" | "REQUEST_CHANGES" | "COMMENT" | "DISMISSED"
                        )
                    })
                    .filter_map(|review| review.user.as_ref().map(|user| user.id.to_string())),
                reviews.iter().all(|review| review.user.is_some()),
            ),
            requested_reviewers: AccountSet::new(
                self.requested_reviewers
                    .iter()
                    .flatten()
                    .map(|user| user.id.to_string()),
                self.requested_reviewers.is_some(),
            ),
            ..Default::default()
        }
    }

    pub(super) fn summary(
        &self,
        reviews: &[ChangeRequestReview],
        check_status: Check,
    ) -> Result<ChangeRequest, Failure> {
        let state = if self.merged {
            ChangeRequestState::Merged
        } else {
            match self.state.as_str() {
                "open" => ChangeRequestState::Open,
                "closed" => ChangeRequestState::Closed,
                _ => return Err(Failure::UnexpectedResponse),
            }
        };
        let review_status = review_state(reviews);
        // Gitea mergeable describes merge conflicts, not every branch protection
        // rule. A conflict-free branch alone is not proof of merge readiness.
        let merge_status = if self.mergeable == Some(false) && state == ChangeRequestState::Open {
            Merge::Conflicting
        } else if self.draft
            || review_status == Review::ChangesRequested
            || check_status == Check::Failing
        {
            Merge::Blocked
        } else {
            Merge::Unknown
        };
        Ok(ChangeRequest {
            relationships: Relationships::default(),
            id: self.id.to_string(),
            number: self.number,
            title: self.title.clone(),
            author: self.user.as_ref().map(|user| user.login.clone()),
            source_branch: self
                .head
                .as_ref()
                .map(|head| head.name.clone())
                .unwrap_or_default(),
            target_branch: self.base.name.clone(),
            state,
            draft: self.draft,
            review_status,
            check_status,
            merge_status,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            web_url: self.html_url.clone(),
        })
    }
}
