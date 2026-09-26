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
