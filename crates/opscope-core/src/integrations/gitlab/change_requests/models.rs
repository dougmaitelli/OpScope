use super::*;

#[derive(Deserialize)]
pub(super) struct Person {
    pub id: Option<u64>,
    pub username: String,
}
#[derive(Deserialize)]
pub(super) struct MergeRequest {
    pub id: u64,
    pub iid: u64,
    pub title: String,
    pub state: String,
    pub author: Option<Person>,
    pub source_branch: String,
    pub target_branch: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub work_in_progress: bool,
    pub detailed_merge_status: Option<String>,
    pub head_pipeline: Option<Pipeline>,
    pub created_at: String,
    pub updated_at: String,
    pub web_url: String,
    pub description: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub reviewers: Option<Vec<Person>>,
    pub source_project_id: Option<u64>,
    pub sha: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct Pipeline {
    pub id: u64,
    pub project_id: Option<u64>,
    pub name: Option<String>,
    pub status: String,
    pub web_url: String,
}
#[derive(Deserialize)]
pub(super) struct Approval {
    pub user: Person,
}
#[derive(Deserialize)]
pub(super) struct Approvals {
    pub approvals_left: Option<u64>,
    pub approved_by: Vec<Approval>,
}
#[derive(Deserialize)]
pub(super) struct Commit {
    pub id: String,
    pub title: String,
    pub author_name: Option<String>,
    pub committed_date: String,
}

pub(super) fn pipeline_status(status: &str) -> Check {
    match status {
        "success" => Check::Passed,
        "failed" => Check::Failing,
        "running" | "pending" | "created" | "preparing" | "waiting_for_resource" => Check::Running,
        _ => Check::Unknown,
    }
}

impl MergeRequest {
    pub(super) fn relationships(&self, approvals: Option<&Approvals>) -> Relationships {
        use crate::domain::AccountSet;
        let approved: Vec<_> = approvals
            .into_iter()
            .flat_map(|a| &a.approved_by)
            .filter_map(|entry| entry.user.id.map(|id| id.to_string()))
            .collect();
        Relationships {
            authors: AccountSet::new(
                self.author
                    .iter()
                    .filter_map(|user| user.id.map(|id| id.to_string())),
                self.author.as_ref().and_then(|user| user.id).is_some(),
            ),
            reviewers: AccountSet::new(approved.clone(), false),
            completed_reviewers: AccountSet::new(
                approved,
                approvals
                    .is_some_and(|a| a.approved_by.iter().all(|entry| entry.user.id.is_some())),
            ),
            assigned_reviewers: AccountSet::new(
                self.reviewers
                    .iter()
                    .flatten()
                    .filter_map(|user| user.id.map(|id| id.to_string())),
                self.reviewers
                    .as_ref()
                    .is_some_and(|reviewers| reviewers.iter().all(|user| user.id.is_some())),
            ),
            ..Default::default()
        }
    }

    pub(super) fn summary(&self, approvals: Option<&Approvals>) -> Result<ChangeRequest, Failure> {
        let state = match self.state.as_str() {
            "opened" | "locked" => ChangeRequestState::Open,
            "closed" => ChangeRequestState::Closed,
            "merged" => ChangeRequestState::Merged,
            _ => return Err(Failure::UnexpectedResponse),
        };
        let draft = self.draft || self.work_in_progress;
        let merge_status = match self.detailed_merge_status.as_deref() {
            Some("conflict") => Merge::Conflicting,
            Some("mergeable") if !draft => Merge::Ready,
            Some(
                "draft_status"
                | "not_approved"
                | "requested_changes"
                | "ci_must_pass"
                | "ci_still_running"
                | "discussions_not_resolved"
                | "need_rebase"
                | "broken_status"
                | "blocked_status"
                | "external_status_checks"
                | "status_checks_must_pass"
                | "not_open",
            ) => Merge::Blocked,
            _ if draft => Merge::Blocked,
            _ => Merge::Unknown,
        };
        let review_status = if self.detailed_merge_status.as_deref() == Some("requested_changes") {
            Review::ChangesRequested
        } else if approvals
            .and_then(|a| a.approvals_left)
            .is_some_and(|count| count > 0)
            || self.detailed_merge_status.as_deref() == Some("not_approved")
        {
            Review::ReviewRequired
        } else if approvals
            .is_some_and(|a| !a.approved_by.is_empty() && a.approvals_left == Some(0))
        {
            Review::Approved
        } else if approvals.is_some_and(|a| a.approved_by.is_empty() && a.approvals_left == Some(0))
        {
            Review::None
        } else {
            Review::Unknown
        };
        Ok(ChangeRequest {
            relationships: Relationships::default(),
            id: self.id.to_string(),
            number: self.iid,
            title: self.title.clone(),
            author: self.author.as_ref().map(|person| person.username.clone()),
            source_branch: self.source_branch.clone(),
            target_branch: self.target_branch.clone(),
            state,
            draft,
            review_status,
            check_status: self
                .head_pipeline
                .as_ref()
                .map_or(Check::None, |pipeline| pipeline_status(&pipeline.status)),
            merge_status,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            web_url: self.web_url.clone(),
        })
    }
}
