use super::*;
use crate::application::MonitoringSettings;
use crate::domain::Relationships;

fn eligible(
    relationships: &Relationships,
    settings: &MonitoringSettings,
    account_id: &str,
) -> bool {
    !settings.only_my_work || relationships.evaluate(account_id).matches()
}

pub(super) fn pull_requests(
    previous: Option<&[ChangeRequest]>,
    current: &[ChangeRequest],
    settings: &MonitoringSettings,
    account_id: &str,
) -> Vec<WorkItemEvent> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let prior: HashMap<_, _> = previous
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();
    let mut events = Vec::new();
    let prefs = settings.notifications;
    for item in current {
        if !eligible(&item.relationships, settings, account_id) {
            continue;
        }
        let old = prior.get(item.id.as_str()).copied();
        let mut add = |enabled: bool, condition: bool, transition: WorkItemTransition| {
            if enabled && condition {
                events.push(WorkItemEvent {
                    kind: WorkItemKind::PullRequest,
                    transition,
                    number: item.number,
                    title: item.title.clone(),
                    url: item.web_url.clone(),
                });
            }
        };
        add(
            prefs.pull_request_opened,
            old.is_none() && item.state == ChangeRequestState::Open,
            WorkItemTransition::Opened,
        );
        add(
            prefs.pull_request_review_requested,
            item.state == ChangeRequestState::Open
                && review_requested(item, account_id) == Some(true)
                && (old.is_none()
                    || old.is_some_and(|old| review_requested(old, account_id) == Some(false))),
            WorkItemTransition::ReviewRequested,
        );
        if let Some(old) = old {
            add(
                prefs.pull_request_changes_requested,
                item.state == ChangeRequestState::Open
                    && item.review_status == ChangeRequestReviewStatus::ChangesRequested
                    && old.review_status != ChangeRequestReviewStatus::ChangesRequested,
                WorkItemTransition::ChangesRequested,
            );
            add(
                prefs.pull_request_merged,
                item.state == ChangeRequestState::Merged && old.state != item.state,
                WorkItemTransition::Merged,
            );
            add(
                prefs.pull_request_closed,
                item.state == ChangeRequestState::Closed && old.state != item.state,
                WorkItemTransition::Closed,
            );
        }
    }
    events
}

pub(super) fn issues(
    previous: Option<&[Issue]>,
    current: &[Issue],
    settings: &MonitoringSettings,
    account_id: &str,
    handle: Option<&str>,
) -> Vec<WorkItemEvent> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let prior: HashMap<_, _> = previous
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();
    let mut events = Vec::new();
    let prefs = settings.notifications;
    let assigned = |item: &Issue| {
        handle.is_some_and(|handle| {
            item.assignees
                .iter()
                .any(|assignee| assignee.eq_ignore_ascii_case(handle))
        })
    };
    for item in current {
        if !eligible(&item.relationships, settings, account_id) {
            continue;
        }
        let old = prior.get(item.id.as_str()).copied();
        let mut add = |enabled: bool, condition: bool, transition: WorkItemTransition| {
            if enabled && condition {
                events.push(WorkItemEvent {
                    kind: WorkItemKind::Issue,
                    transition,
                    number: item.number,
                    title: item.title.clone(),
                    url: item.web_url.clone(),
                });
            }
        };
        add(
            prefs.issue_opened,
            old.is_none() && item.state == IssueState::Open,
            WorkItemTransition::Opened,
        );
        add(
            prefs.issue_assigned,
            item.state == IssueState::Open && assigned(item) && !old.is_some_and(assigned),
            WorkItemTransition::Assigned,
        );
        if let Some(old) = old {
            add(
                prefs.issue_reopened,
                old.state == IssueState::Closed && item.state == IssueState::Open,
                WorkItemTransition::Reopened,
            );
            add(
                prefs.issue_closed,
                old.state == IssueState::Open && item.state == IssueState::Closed,
                WorkItemTransition::Closed,
            );
        }
    }
    events
}
