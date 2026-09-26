//! Provider-independent monitoring concepts.

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
