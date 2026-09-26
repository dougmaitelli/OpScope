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
