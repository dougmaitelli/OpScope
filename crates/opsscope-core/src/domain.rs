//! Provider-independent monitoring concepts.

mod relevance;
pub use relevance::{Relevance, RelevanceReason};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryVisibility {
    Public,
    Private,
}

/// A provider-independent repository available to a connected account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Repository {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub visibility: RepositoryVisibility,
    pub web_url: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeRequestReviewStatus {
    Approved,
    ChangesRequested,
    ReviewRequired,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeRequestCheckStatus {
    Passed,
    Failing,
    Running,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeRequestMergeStatus {
    Ready,
    Blocked,
    Conflicting,
    Unknown,
}

/// A provider-independent proposed change to a repository.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRequest {
    #[serde(default)]
    pub relevance: Relevance,
    pub id: String,
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
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRequestReview {
    pub reviewer: Option<String>,
    pub status: ChangeRequestReviewStatus,
    pub submitted_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRequestCheck {
    pub name: String,
    pub status: ChangeRequestCheckStatus,
    pub web_url: Option<String>,
    pub workflow_run_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRequestCommit {
    pub sha: String,
    pub title: String,
    pub author: Option<String>,
    pub committed_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRequestDetails {
    pub change_request: ChangeRequest,
    pub body: Option<String>,
    pub labels: Vec<String>,
    pub reviews: Vec<ChangeRequestReview>,
    pub checks: Vec<ChangeRequestCheck>,
    pub latest_commit: Option<ChangeRequestCommit>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IssueState {
    Open,
    Closed,
}

/// A provider-independent issue reported against a repository.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Issue {
    #[serde(default)]
    pub relevance: Relevance,
    pub id: String,
    pub number: u64,
    pub title: String,
    pub author: Option<String>,
    pub state: IssueState,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub comment_count: u64,
    pub created_at: String,
    pub updated_at: String,
    pub web_url: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IssueComment {
    pub id: String,
    pub author: Option<String>,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IssueDetails {
    pub issue: Issue,
    pub body: Option<String>,
    pub milestone: Option<String>,
    pub comments: Vec<IssueComment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowState {
    Active,
    Disabled,
}

/// A provider-independent automation workflow discovered in a repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub path: String,
    pub state: WorkflowState,
    pub web_url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunLifecycle {
    Queued,
    Running,
    Completed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunOutcome {
    Success,
    Warning,
    Failure,
    Cancelled,
    Skipped,
    Unknown,
}

/// A provider-independent execution of an automation workflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowRun {
    pub relevance: Relevance,
    pub id: String,
    pub workflow_id: String,
    pub run_number: u64,
    pub attempt: u64,
    pub title: String,
    pub lifecycle: RunLifecycle,
    pub outcome: RunOutcome,
    pub branch: Option<String>,
    pub commit_sha: String,
    pub actor: Option<String>,
    pub trigger: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub updated_at: String,
    pub web_url: String,
    pub provider_status: String,
    pub provider_conclusion: Option<String>,
}

/// One text file from a workflow run's provider-supplied log bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowRunLog {
    pub name: String,
    pub content: String,
}

/// Provider-independent logs for a single workflow run attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowRunLogs {
    pub files: Vec<WorkflowRunLog>,
    pub truncated: bool,
}
