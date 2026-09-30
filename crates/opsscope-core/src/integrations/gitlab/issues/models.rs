use super::*;

#[derive(Deserialize)]
pub(super) struct Tracker {
    pub issues_access_level: Option<String>,
    pub issues_enabled: Option<bool>,
}

#[derive(Deserialize)]
pub(super) struct Person {
    pub id: u64,
    pub username: String,
}

#[derive(Deserialize)]
pub(super) struct Milestone {
    pub title: String,
}

#[derive(Deserialize)]
pub(super) struct ProviderIssue {
    pub id: u64,
    pub iid: u64,
    pub title: String,
    pub state: String,
    pub author: Option<Person>,
    pub assignees: Vec<Person>,
    pub labels: Vec<String>,
    pub user_notes_count: u64,
    pub description: Option<String>,
    pub milestone: Option<Milestone>,
    pub subscribed: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
    pub web_url: String,
}

#[derive(Deserialize)]
pub(super) struct Note {
    pub id: u64,
    pub author: Option<Person>,
    pub body: String,
    pub system: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl ProviderIssue {
    pub(super) fn summary(&self) -> Result<Issue, Failure> {
        let state = match self.state.as_str() {
            "opened" => IssueState::Open,
            "closed" => IssueState::Closed,
            _ => return Err(Failure::UnexpectedResponse),
        };
        Ok(Issue {
            relationships: Relationships::default(),
            id: self.id.to_string(),
            number: self.iid,
            title: self.title.clone(),
            state,
            author: self.author.as_ref().map(|user| user.username.clone()),
            labels: self.labels.clone(),
            assignees: self
                .assignees
                .iter()
                .map(|user| user.username.clone())
                .collect(),
            comment_count: self.user_notes_count,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            web_url: self.web_url.clone(),
        })
    }

    pub(super) fn relationships(
        &self,
        owner: Option<u64>,
        notes: Option<&[Note]>,
    ) -> Relationships {
        use crate::domain::AccountSet;
        Relationships {
            authors: AccountSet::new(
                self.author.iter().map(|user| user.id.to_string()),
                self.author.is_some(),
            ),
            subscribers: owner
                .map(|owner| AccountSet::viewer(&owner.to_string(), self.subscribed))
                .unwrap_or_default(),
            participants: AccountSet::new(
                notes
                    .into_iter()
                    .flatten()
                    .filter(|note| !note.system)
                    .filter_map(|note| note.author.as_ref().map(|user| user.id.to_string())),
                notes.is_some_and(|notes| {
                    notes
                        .iter()
                        .filter(|note| !note.system)
                        .all(|note| note.author.is_some())
                }),
            ),
            ..Default::default()
        }
    }
}

impl From<Note> for IssueComment {
    fn from(note: Note) -> Self {
        Self {
            id: note.id.to_string(),
            author: note.author.map(|user| user.username),
            body: note.body,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }
}
