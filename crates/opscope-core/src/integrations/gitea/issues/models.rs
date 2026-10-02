use super::*;

#[derive(Deserialize)]
pub(super) struct Tracker {
    pub has_issues: Option<bool>,
    pub external_tracker: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub(super) struct Person {
    pub id: u64,
    pub login: String,
}
#[derive(Deserialize)]
pub(super) struct Label {
    pub name: String,
}
#[derive(Deserialize)]
pub(super) struct Milestone {
    pub title: String,
}
#[derive(Deserialize)]
pub(super) struct Subscription {
    pub subscribed: bool,
    pub ignored: bool,
}

#[derive(Deserialize)]
pub(super) struct ProviderIssue {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub state: String,
    pub user: Option<Person>,
    pub assignees: Option<Vec<Person>>,
    pub labels: Option<Vec<Label>>,
    pub comments: u64,
    pub body: Option<String>,
    pub milestone: Option<Milestone>,
    pub pull_request: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
}

#[derive(Deserialize)]
pub(super) struct Comment {
    pub id: u64,
    pub user: Option<Person>,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

impl ProviderIssue {
    pub(super) fn summary(&self) -> Result<Issue, Failure> {
        let state = match self.state.as_str() {
            "open" => IssueState::Open,
            "closed" => IssueState::Closed,
            _ => return Err(Failure::UnexpectedResponse),
        };
        Ok(Issue {
            relationships: Relationships::default(),
            id: self.id.to_string(),
            number: self.number,
            title: self.title.clone(),
            state,
            author: self.user.as_ref().map(|user| user.login.clone()),
            assignees: self
                .assignees
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|user| user.login.clone())
                .collect(),
            labels: self
                .labels
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|label| label.name.clone())
                .collect(),
            comment_count: self.comments,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            web_url: self.html_url.clone(),
        })
    }

    pub(super) fn relationships(
        &self,
        owner: Option<u64>,
        subscription: Option<&Subscription>,
        comments: Option<&[Comment]>,
    ) -> Relationships {
        use crate::domain::AccountSet;
        Relationships {
            authors: AccountSet::new(
                self.user.iter().map(|user| user.id.to_string()),
                self.user.is_some(),
            ),
            subscribers: owner
                .map(|owner| {
                    AccountSet::viewer(
                        &owner.to_string(),
                        subscription.map(|s| s.subscribed && !s.ignored),
                    )
                })
                .unwrap_or_default(),
            participants: AccountSet::new(
                comments
                    .into_iter()
                    .flatten()
                    .filter_map(|comment| comment.user.as_ref().map(|user| user.id.to_string())),
                comments
                    .is_some_and(|comments| comments.iter().all(|comment| comment.user.is_some())),
            ),
            ..Default::default()
        }
    }
}

impl From<Comment> for IssueComment {
    fn from(comment: Comment) -> Self {
        Self {
            id: comment.id.to_string(),
            author: comment.user.map(|user| user.login),
            body: comment.body,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }
    }
}
