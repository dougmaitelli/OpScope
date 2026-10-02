/// A local policy result. Only the underlying relationships are persisted.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Relevance {
    pub reasons: Vec<RelevanceReason>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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
