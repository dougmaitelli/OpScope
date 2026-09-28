use serde::{Deserialize, Serialize};

/// Relationship to the token owner, scoped by the connection/account cache key.
/// An incomplete observation must not be interpreted as a confirmed non-match.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Relevance {
    pub account_id: Option<String>,
    pub reasons: Vec<RelevanceReason>,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RelevanceReason {
    Authored,
    Reviewed,
    ReviewRequested,
    Subscribed,
    Discussed,
    CommitAuthored,
    ChangeRequestAuthored,
}

impl Relevance {
    #[must_use]
    pub fn matches(&self) -> bool {
        !self.reasons.is_empty()
    }
}
