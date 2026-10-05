use opscope_core::application::{
    Notification, NotificationEvents, WorkItemKind, WorkItemTransition,
};

pub(super) fn render(notification: &Notification) -> (String, String) {
    let repository = &notification.repository;
    let repository_name = format!("{}/{}", repository.owner, repository.name);
    match &notification.events {
        NotificationEvents::WorkItems(events) => {
            let title = format!("{} updates in {repository_name}", events.len());
            let body = events
                .iter()
                .map(|event| {
                    let kind = match event.kind {
                        WorkItemKind::PullRequest => "PR",
                        WorkItemKind::Issue => "Issue",
                    };
                    let transition = match event.transition {
                        WorkItemTransition::Opened => "opened",
                        WorkItemTransition::ReviewRequested => "requests your review",
                        WorkItemTransition::ChangesRequested => "has changes requested",
                        WorkItemTransition::Merged => "merged",
                        WorkItemTransition::Closed => "closed",
                        WorkItemTransition::Assigned => "assigned to you",
                        WorkItemTransition::Reopened => "reopened",
                    };
                    format!("{kind} #{} {transition}: {}", event.number, event.title)
                })
                .collect::<Vec<_>>()
                .join("\n");
            (title, body)
        }
        NotificationEvents::WorkflowFailures(events) => {
            let title = if events.len() == 1 {
                format!("Workflow failed in {repository_name}")
            } else {
                format!("{} workflows failed in {repository_name}", events.len())
            };
            let body = events
                .iter()
                .map(|event| match event.attempt {
                    Some(attempt) if attempt > 1 => format!("{} (attempt {attempt})", event.name),
                    _ => event.name.clone(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            (title, body)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opscope_core::application::{NotificationSeverity, WorkItemEvent, WorkflowFailureEvent};
    use opscope_core::domain::{Repository, RepositoryVisibility};

    fn notification(events: NotificationEvents) -> Notification {
        Notification {
            repository: Repository {
                id: "repo".into(),
                owner: "owner".into(),
                name: "project".into(),
                description: None,
                visibility: RepositoryVisibility::Private,
                web_url: "https://example.test/owner/project".into(),
            },
            events,
            severity: NotificationSeverity::Info,
        }
    }

    #[test]
    fn renders_grouped_work_items() {
        let notification = notification(NotificationEvents::WorkItems(vec![
            WorkItemEvent {
                kind: WorkItemKind::PullRequest,
                transition: WorkItemTransition::Merged,
                number: 42,
                title: "Fix — login".into(),
                url: "https://example.test/pull/42".into(),
            },
            WorkItemEvent {
                kind: WorkItemKind::Issue,
                transition: WorkItemTransition::Reopened,
                number: 7,
                title: "Crash".into(),
                url: "https://example.test/issues/7".into(),
            },
        ]));
        assert_eq!(
            render(&notification),
            (
                "2 updates in owner/project".into(),
                "PR #42 merged: Fix — login\nIssue #7 reopened: Crash".into()
            )
        );
    }

    #[test]
    fn renders_workflow_failures_and_retry_attempts() {
        let mut notification = notification(NotificationEvents::WorkflowFailures(vec![
            WorkflowFailureEvent {
                workflow_id: "build".into(),
                name: "Build".into(),
                run_id: Some("run".into()),
                attempt: Some(2),
            },
        ]));
        assert_eq!(
            render(&notification),
            (
                "Workflow failed in owner/project".into(),
                "Build (attempt 2)".into()
            )
        );
        let NotificationEvents::WorkflowFailures(events) = &mut notification.events else {
            unreachable!()
        };
        events.push(WorkflowFailureEvent {
            workflow_id: "test".into(),
            name: "Test".into(),
            run_id: None,
            attempt: Some(1),
        });
        assert_eq!(
            render(&notification),
            (
                "2 workflows failed in owner/project".into(),
                "Build (attempt 2), Test".into()
            )
        );
    }
}
