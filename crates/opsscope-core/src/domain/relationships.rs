use serde::{Deserialize, Serialize};

use super::{Relevance, RelevanceReason};

/// Provider account IDs are meaningful only within the owning connection/provider instance.
/// `scope` restricts viewer-only observations to the account that fetched them.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AccountSet {
    pub ids: Vec<String>,
    pub complete: bool,
    pub scope: Option<String>,
}

impl AccountSet {
    pub fn new(ids: impl IntoIterator<Item = String>, complete: bool) -> Self {
        let mut ids: Vec<_> = ids.into_iter().filter(|id| !id.is_empty()).collect();
        ids.sort();
        ids.dedup();
        Self {
            ids,
            complete,
            scope: None,
        }
    }

    pub fn viewer(account: &str, value: Option<bool>) -> Self {
        Self {
            ids: if value == Some(true) {
                vec![account.to_owned()]
            } else {
                Vec::new()
            },
            complete: value.is_some(),
            scope: Some(account.to_owned()),
        }
    }

    pub fn contains(&self, account: &str) -> Option<bool> {
        if self.scope.as_deref().is_some_and(|scope| scope != account) {
            return None;
        }
        if self.ids.iter().any(|id| id == account) {
            Some(true)
        } else if self.complete {
            Some(false)
        } else {
            None
        }
    }
}

/// Facts cached with a resource, not a decision about who should see it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Relationships {
    pub authors: AccountSet,
    pub reviewers: AccountSet,
    pub requested_reviewers: AccountSet,
    pub assigned_reviewers: AccountSet,
    pub completed_reviewers: AccountSet,
    pub subscribers: AccountSet,
    pub participants: AccountSet,
    pub commit_authors: AccountSet,
    pub change_request_authors: AccountSet,
    pub linked_change_requests: Vec<ChangeRequestReference>,
    pub commit_author_email: Option<String>,
    pub identities: Vec<AccountEmails>,
    pub viewers: Vec<ViewerRelationships>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRequestReference {
    pub id: Option<String>,
    pub number: Option<u64>,
    pub web_url: Option<String>,
    pub author_id: Option<String>,
}

/// Facts that the provider exposes only relative to the authenticated viewer.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ViewerRelationships {
    pub account_id: String,
    pub authored: Option<bool>,
    pub reviewed: Option<bool>,
    pub review_requested: Option<bool>,
    pub subscribed: Option<bool>,
    pub discussed: Option<bool>,
    pub commit_authored: Option<bool>,
    pub change_request_authored: Option<bool>,
}

/// Provider-confirmed email aliases; never derived from a display name or trigger actor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AccountEmails {
    pub account_id: String,
    pub emails: Vec<String>,
    pub complete: bool,
}

impl Relationships {
    pub fn review_requested(&self, account: &str) -> Option<bool> {
        if let Some(value) = self
            .viewers
            .iter()
            .find(|viewer| viewer.account_id == account)
            .and_then(|viewer| viewer.review_requested)
            .or_else(|| self.requested_reviewers.contains(account))
        {
            return Some(value);
        }
        match self.assigned_reviewers.contains(account) {
            Some(false) => Some(false),
            Some(true) => self
                .completed_reviewers
                .contains(account)
                .map(|completed| !completed),
            None => None,
        }
    }

    pub fn evaluate(&self, account: &str) -> Relevance {
        let mut result = Relevance::default();
        for (fact, reason) in [
            (&self.authors, RelevanceReason::Authored),
            (&self.reviewers, RelevanceReason::Reviewed),
            (&self.subscribers, RelevanceReason::Subscribed),
            (&self.participants, RelevanceReason::Discussed),
            (&self.commit_authors, RelevanceReason::CommitAuthored),
            (
                &self.change_request_authors,
                RelevanceReason::ChangeRequestAuthored,
            ),
        ] {
            if fact.contains(account) == Some(true) {
                result.reasons.push(reason);
            }
        }
        if let Some(email) = self.commit_author_email.as_deref()
            && self.identities.iter().any(|identity| {
                identity.account_id == account
                    && identity
                        .emails
                        .iter()
                        .any(|known| known.eq_ignore_ascii_case(email))
            })
            && !result.reasons.contains(&RelevanceReason::CommitAuthored)
        {
            result.reasons.push(RelevanceReason::CommitAuthored);
        }
        for viewer in self
            .viewers
            .iter()
            .filter(|viewer| viewer.account_id == account)
        {
            for (value, reason) in [
                (viewer.authored, RelevanceReason::Authored),
                (viewer.reviewed, RelevanceReason::Reviewed),
                (viewer.review_requested, RelevanceReason::ReviewRequested),
                (viewer.subscribed, RelevanceReason::Subscribed),
                (viewer.discussed, RelevanceReason::Discussed),
                (viewer.commit_authored, RelevanceReason::CommitAuthored),
                (
                    viewer.change_request_authored,
                    RelevanceReason::ChangeRequestAuthored,
                ),
            ] {
                if value == Some(true) && !result.reasons.contains(&reason) {
                    result.reasons.push(reason);
                }
            }
        }
        if self.review_requested(account) == Some(true)
            && !result.reasons.contains(&RelevanceReason::ReviewRequested)
        {
            result.reasons.push(RelevanceReason::ReviewRequested);
        }
        result.reasons.sort();
        result
    }
}

#[cfg(test)]
mod tests;
