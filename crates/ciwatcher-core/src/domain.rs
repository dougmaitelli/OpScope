//! Provider-independent monitoring concepts.

/// The current state of a configured monitor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonitorStatus {
    Unknown,
    Passing,
    Failing,
    Running,
}

/// A provider-independent monitor summary returned by a source integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub status: MonitorStatus,
}

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
