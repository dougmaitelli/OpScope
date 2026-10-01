use super::*;

#[derive(Deserialize)]
pub(super) struct Person {
    pub uuid: String,
    pub display_name: String,
}
#[derive(Deserialize)]
pub(super) struct Branch {
    pub name: String,
}
#[derive(Deserialize)]
pub(super) struct CommitRef {
    pub hash: String,
}
#[derive(Deserialize)]
pub(super) struct Endpoint {
    pub branch: Branch,
    pub commit: CommitRef,
}
#[derive(Deserialize)]
pub(super) struct Participant {
    pub user: Person,
    #[serde(default)]
    pub approved: bool,
    pub state: Option<String>,
    pub participated_on: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct Text {
    pub raw: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct PullRequest {
    pub id: u64,
    pub title: String,
    pub state: String,
    pub author: Option<Person>,
    pub source: Option<Endpoint>,
    pub destination: Endpoint,
    #[serde(default)]
    pub draft: bool,
    pub created_on: String,
    pub updated_on: String,
    pub links: Links,
    pub description: Option<String>,
    pub summary: Option<Text>,
    pub participants: Option<Vec<Participant>>,
    pub reviewers: Option<Vec<Person>>,
}
#[derive(Deserialize)]
pub(super) struct PullId {
    pub id: u64,
}
#[derive(Deserialize)]
pub(super) struct Status {
    pub key: String,
    pub name: Option<String>,
    pub state: String,
    pub url: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct Commit {
    pub hash: String,
    pub message: String,
    pub date: String,
    pub author: Option<CommitAuthor>,
}
#[derive(Deserialize)]
pub(super) struct CommitAuthor {
    pub raw: Option<String>,
    pub user: Option<Person>,
}
#[derive(Deserialize)]
pub(super) struct PipelineLink {
    pub uuid: String,
    pub build_number: u64,
    pub target: PipelineTarget,
}
#[derive(Deserialize)]
pub(super) struct PipelineTarget {
    pub commit: CommitRef,
}

impl Participant {
    fn status(&self) -> Review {
        match self.state.as_deref() {
            Some("changes_requested") => Review::ChangesRequested,
            Some("approved") => Review::Approved,
            _ if self.approved => Review::Approved,
            _ => Review::Unknown,
        }
    }
}

impl Status {
    pub(super) fn into_check(self) -> ChangeRequestCheck {
        ChangeRequestCheck {
            name: self
                .name
                .filter(|name| !name.is_empty())
                .unwrap_or(self.key),
            status: match self.state.as_str() {
                "SUCCESSFUL" => Check::Passed,
                "FAILED" => Check::Failing,
                "INPROGRESS" => Check::Running,
                _ => Check::Unknown,
            },
            web_url: self.url.filter(|url| !url.is_empty()),
            workflow_run_id: None,
        }
    }
}

impl PullRequest {
    pub(super) fn relationships(&self) -> Relationships {
        use crate::domain::AccountSet;
        let reviewed: Vec<_> = self
            .participants
            .iter()
            .flatten()
            .filter(|p| p.status() != Review::Unknown)
            .map(|p| p.user.uuid.clone())
            .collect();
        Relationships {
            authors: AccountSet::new(
                self.author.iter().map(|user| user.uuid.clone()),
                self.author.is_some(),
            ),
            reviewers: AccountSet::new(reviewed.clone(), false),
            completed_reviewers: AccountSet::new(reviewed, self.participants.is_some()),
            assigned_reviewers: AccountSet::new(
                self.reviewers
                    .iter()
                    .flatten()
                    .map(|user| user.uuid.clone()),
                self.reviewers.is_some(),
            ),
            ..Default::default()
        }
    }

    pub(super) fn reviews(&self) -> Vec<ChangeRequestReview> {
        let mut reviews = Vec::new();
        for participant in self.participants.as_deref().unwrap_or_default() {
            if participant.status() != Review::Unknown {
                reviews.push(ChangeRequestReview {
                    reviewer: Some(participant.user.display_name.clone()),
                    status: participant.status(),
                    submitted_at: participant.participated_on.clone(),
                });
            }
        }
        for reviewer in self.reviewers.as_deref().unwrap_or_default() {
            if !self
                .participants
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|p| p.user.uuid == reviewer.uuid && p.status() != Review::Unknown)
            {
                reviews.push(ChangeRequestReview {
                    reviewer: Some(reviewer.display_name.clone()),
                    status: Review::ReviewRequired,
                    submitted_at: None,
                });
            }
        }
        reviews
    }
    pub(super) fn summary(&self, checks: &[ChangeRequestCheck]) -> Result<ChangeRequest, Failure> {
        let state = match self.state.as_str() {
            "OPEN" => ChangeRequestState::Open,
            "MERGED" => ChangeRequestState::Merged,
            "DECLINED" | "SUPERSEDED" => ChangeRequestState::Closed,
            _ => return Err(Failure::UnexpectedResponse),
        };
        let mut review_status =
            aggregate::reviews(self.reviews().iter().map(|review| review.status));
        if review_status == Review::None
            && (self.participants.is_none() || self.reviewers.is_none())
        {
            review_status = Review::Unknown;
        }
        let check_status = aggregate::checks(checks.iter().map(|check| check.status));
        Ok(ChangeRequest {
            relationships: Relationships::default(),
            id: self.id.to_string(),
            number: self.id,
            title: self.title.clone(),
            author: self
                .author
                .as_ref()
                .map(|author| author.display_name.clone()),
            source_branch: self
                .source
                .as_ref()
                .map(|source| source.branch.name.clone())
                .unwrap_or_default(),
            target_branch: self.destination.branch.name.clone(),
            state,
            draft: self.draft,
            review_status,
            check_status,
            // Approvals and successful checks do not prove that workspace merge
            // policies or conflict checks are satisfied.
            merge_status: if self.draft
                || review_status == Review::ChangesRequested
                || check_status == Check::Failing
            {
                Merge::Blocked
            } else {
                Merge::Unknown
            },
            created_at: self.created_on.clone(),
            updated_at: self.updated_on.clone(),
            web_url: self.links.html.href.clone(),
        })
    }
}
