use super::*;
use crate::domain::{Issue, IssueState};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IssueActivityKind {
    Opened,
    Updated,
    Closed,
    Reopened,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IssueActivityEvent {
    pub id: String,
    pub kind: IssueActivityKind,
    pub occurred_at: String,
    pub source_id: String,
    pub source_name: String,
    pub source_abbreviation: String,
    pub repository_id: String,
    pub repository_owner: String,
    pub repository_name: String,
    pub issue: Issue,
}

pub struct ActivityInventory {
    pub change_request_events: Vec<ChangeRequestActivityEvent>,
    pub issue_events: Vec<IssueActivityEvent>,
}

#[derive(Clone)]
pub struct TrackIssueActivity {
    events: Arc<dyn ActivityEventRepository>,
}

impl TrackIssueActivity {
    pub fn new(events: Arc<dyn ActivityEventRepository>) -> Self {
        Self {
            events,
        }
    }

    pub fn previous(
        &self,
        source_id: &str,
        repository_id: &str,
    ) -> Result<Option<Vec<Issue>>, PersistenceFailure> {
        self.events.load_issue_state(source_id, repository_id)
    }

    pub fn observe(
        &self,
        source: &ConnectedSource,
        repository: &Repository,
        current: &[Issue],
        departed: &[Issue],
    ) -> Result<(), PersistenceFailure> {
        let previous = self
            .previous(&source.id, &repository.id)?
            .unwrap_or_default();
        let old = previous
            .iter()
            .map(|issue| (issue.id.as_str(), issue))
            .collect::<HashMap<_, _>>();
        let mut events = Vec::new();
        for issue in current.iter().chain(departed) {
            let kind = match old.get(issue.id.as_str()) {
                None => Some(IssueActivityKind::Opened),
                Some(old) if old.state != issue.state => {
                    Some(if issue.state == IssueState::Closed {
                        IssueActivityKind::Closed
                    } else {
                        IssueActivityKind::Reopened
                    })
                }
                Some(old)
                    if old.updated_at != issue.updated_at
                        || old.title != issue.title
                        || old.labels != issue.labels
                        || old.assignees != issue.assignees
                        || old.comment_count != issue.comment_count =>
                {
                    Some(IssueActivityKind::Updated)
                }
                _ => None,
            };
            if let Some(kind) = kind {
                let occurred_at = if kind == IssueActivityKind::Opened {
                    &issue.created_at
                } else {
                    &issue.updated_at
                };
                events.push(IssueActivityEvent {
                    id: format!(
                        "issue:{}:{}:{}:{kind:?}:{occurred_at}",
                        source.id, repository.id, issue.id
                    ),
                    kind,
                    occurred_at: occurred_at.clone(),
                    source_id: source.id.clone(),
                    source_name: source.label.clone(),
                    source_abbreviation: source.descriptor.abbreviation.clone(),
                    repository_id: repository.id.clone(),
                    repository_owner: repository.owner.clone(),
                    repository_name: repository.name.clone(),
                    issue: issue.clone(),
                });
            }
        }
        // Keep absent items until their provider state can be confirmed, and retain closed
        // observations so reopening an issue is distinguishable from its first appearance.
        let ids = current
            .iter()
            .chain(departed)
            .map(|issue| issue.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut observed = current.iter().chain(departed).cloned().collect::<Vec<_>>();
        observed.extend(
            previous
                .into_iter()
                .filter(|issue| !ids.contains(issue.id.as_str())),
        );
        self.events
            .save_issue_observation(&source.id, &repository.id, &observed, &events)
    }
}
