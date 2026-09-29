use super::*;
use crate::application::{
    ConnectedSource, MonitoringSettings, NotificationDeliveryFailure, SecretReference,
    StoredConnection, ValidatedAccount, WorkflowRunLogsFailure,
};
use crate::domain::{
    ChangeRequestCheckStatus, ChangeRequestDetails, ChangeRequestMergeStatus, IssueDetails,
    Relevance, RepositoryVisibility, Workflow, WorkflowRun, WorkflowRunLogs,
};
use crate::persistence::SqliteDatabase;
use crate::source_data::{SourceDataFailure, WorkflowRunCollection};
use async_trait::async_trait;
use std::sync::Mutex;

fn pull(number: u64) -> ChangeRequest {
    ChangeRequest {
        id: format!("pr-{number}"),
        number,
        title: format!("Change {number}"),
        relevance: Relevance {
            account_id: Some("42".into()),
            complete: true,
            reasons: vec![RelevanceReason::Authored],
        },
        author: Some("me".into()),
        source_branch: "feature".into(),
        target_branch: "main".into(),
        state: ChangeRequestState::Open,
        draft: false,
        review_status: ChangeRequestReviewStatus::ReviewRequired,
        check_status: ChangeRequestCheckStatus::Unknown,
        merge_status: ChangeRequestMergeStatus::Unknown,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        web_url: format!("https://example.test/pull/{number}"),
    }
}

fn issue(number: u64) -> Issue {
    Issue {
        id: format!("issue-{number}"),
        number,
        title: format!("Problem {number}"),
        relevance: pull(number).relevance,
        author: Some("me".into()),
        state: IssueState::Open,
        labels: vec![],
        assignees: vec![],
        comment_count: 0,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
        web_url: format!("https://example.test/issues/{number}"),
    }
}

fn repository() -> Repository {
    Repository {
        id: "repo".into(),
        owner: "owner".into(),
        name: "project".into(),
        description: None,
        visibility: RepositoryVisibility::Private,
        web_url: "https://example.test/owner/project".into(),
    }
}

fn connection() -> StoredConnection {
    StoredConnection {
        id: "source".into(),
        source_id: "module".into(),
        unique_key: "server".into(),
        label: "Server".into(),
        configuration: Default::default(),
        account: ValidatedAccount {
            external_id: "42".into(),
            name: "Me".into(),
            handle: Some("me".into()),
            profile_url: None,
        },
        secret_reference: SecretReference::for_connection("source", "test"),
    }
}

#[derive(Default)]
struct Sink {
    sent: Mutex<Vec<Notification>>,
    fail: bool,
}
#[async_trait]
impl NotificationSink for Sink {
    async fn send(&self, notification: &Notification) -> Result<(), NotificationDeliveryFailure> {
        self.sent.lock().unwrap().push(notification.clone());
        if self.fail {
            Err(NotificationDeliveryFailure)
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct Details {
    pulls: Mutex<HashMap<u64, ChangeRequest>>,
    issues: Mutex<HashMap<u64, Issue>>,
}
#[async_trait]
impl SourceData for Details {
    async fn workflow_run(
        &self,
        _: &str,
        _: &Repository,
        _: &str,
    ) -> Result<Option<WorkflowRun>, SourceDataFailure> {
        unreachable!()
    }
    fn sources(&self) -> Result<Vec<ConnectedSource>, SourceDataFailure> {
        unreachable!()
    }
    async fn repositories(
        &self,
        _: &str,
        _: RefreshMode,
    ) -> Result<Option<Vec<Repository>>, SourceDataFailure> {
        unreachable!()
    }
    async fn workflows(
        &self,
        _: &str,
        _: &Repository,
        _: RefreshMode,
    ) -> Result<Vec<Workflow>, SourceDataFailure> {
        unreachable!()
    }
    async fn workflow_runs(
        &self,
        _: &str,
        _: &Repository,
        _: RefreshMode,
    ) -> Result<WorkflowRunCollection, SourceDataFailure> {
        unreachable!()
    }
    async fn change_requests(
        &self,
        _: &str,
        _: &Repository,
        _: RefreshMode,
    ) -> Result<Option<Vec<ChangeRequest>>, SourceDataFailure> {
        unreachable!()
    }
    async fn change_request_details(
        &self,
        _: &str,
        _: &Repository,
        number: u64,
        refresh: RefreshMode,
    ) -> Result<Option<ChangeRequestDetails>, SourceDataFailure> {
        assert_eq!(refresh, RefreshMode::Force);
        Ok(self
            .pulls
            .lock()
            .unwrap()
            .get(&number)
            .cloned()
            .map(|change_request| ChangeRequestDetails {
                change_request,
                body: None,
                labels: vec![],
                reviews: vec![],
                checks: vec![],
                latest_commit: None,
            }))
    }
    async fn issue_details(
        &self,
        _: &str,
        _: &Repository,
        number: u64,
        refresh: RefreshMode,
    ) -> Result<Option<IssueDetails>, SourceDataFailure> {
        assert_eq!(refresh, RefreshMode::Force);
        Ok(self
            .issues
            .lock()
            .unwrap()
            .get(&number)
            .cloned()
            .map(|issue| IssueDetails {
                issue,
                body: None,
                milestone: None,
                comments: vec![],
            }))
    }
    async fn workflow_run_logs(
        &self,
        _: &str,
        _: &Repository,
        _: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        unreachable!()
    }
}

fn notifier(db: Arc<SqliteDatabase>, sink: Arc<Sink>) -> NotifyWorkItems {
    NotifyWorkItems::new(db.clone(), db.clone(), db, sink)
}

#[test]
fn detects_each_pull_request_transition_and_not_unchanged_observations() {
    let settings = MonitoringSettings::default();
    let base = pull(1);
    assert!(
        transitions::pull_requests(None, std::slice::from_ref(&base), &settings, "42").is_empty()
    );
    assert!(
        transitions::pull_requests(
            Some(std::slice::from_ref(&base)),
            std::slice::from_ref(&base),
            &settings,
            "42"
        )
        .is_empty()
    );
    assert_eq!(
        transitions::pull_requests(Some(&[]), std::slice::from_ref(&base), &settings, "42").len(),
        1
    );
    for (expected, current) in [
        ("requests your review", {
            let mut p = base.clone();
            p.relevance.reasons.push(RelevanceReason::ReviewRequested);
            p
        }),
        ("has changes requested", {
            let mut p = base.clone();
            p.review_status = ChangeRequestReviewStatus::ChangesRequested;
            p
        }),
        ("merged", {
            let mut p = base.clone();
            p.state = ChangeRequestState::Merged;
            p
        }),
        ("closed", {
            let mut p = base.clone();
            p.state = ChangeRequestState::Closed;
            p
        }),
    ] {
        let events = transitions::pull_requests(
            Some(std::slice::from_ref(&base)),
            std::slice::from_ref(&current),
            &settings,
            "42",
        );
        assert_eq!(events.len(), 1);
        assert!(events[0].contains(expected));
        assert!(events[0].contains(&current.title));
        assert!(events[0].contains(&current.web_url));
        assert!(
            transitions::pull_requests(
                Some(std::slice::from_ref(&current)),
                std::slice::from_ref(&current),
                &settings,
                "42"
            )
            .is_empty()
        );
    }
}

#[test]
fn changes_requested_is_detected_after_an_unset_review_decision() {
    let mut previous = pull(1);
    previous.review_status = ChangeRequestReviewStatus::Unknown;
    let mut current = previous.clone();
    current.review_status = ChangeRequestReviewStatus::ChangesRequested;
    let events = transitions::pull_requests(
        Some(&[previous]),
        &[current],
        &MonitoringSettings::default(),
        "42",
    );
    assert_eq!(events.len(), 1);
    assert!(events[0].contains("has changes requested"));
}

#[test]
fn detects_issue_open_assignment_close_and_reopen() {
    let settings = MonitoringSettings::default();
    let base = issue(1);
    assert!(
        transitions::issues(
            None,
            std::slice::from_ref(&base),
            &settings,
            "42",
            Some("me")
        )
        .is_empty()
    );
    assert_eq!(
        transitions::issues(
            Some(&[]),
            std::slice::from_ref(&base),
            &settings,
            "42",
            Some("me")
        )
        .len(),
        1
    );
    let mut assigned = base.clone();
    assigned.assignees.push("ME".into());
    assert!(
        transitions::issues(
            Some(std::slice::from_ref(&base)),
            std::slice::from_ref(&assigned),
            &settings,
            "42",
            Some("me")
        )[0]
        .contains("assigned to you")
    );
    assert!(
        transitions::issues(
            Some(std::slice::from_ref(&base)),
            std::slice::from_ref(&assigned),
            &settings,
            "42",
            Some("someone-else")
        )
        .is_empty()
    );
    assert!(
        transitions::issues(
            Some(std::slice::from_ref(&assigned)),
            std::slice::from_ref(&assigned),
            &settings,
            "42",
            Some("me")
        )
        .is_empty()
    );
    let mut closed = base.clone();
    closed.state = IssueState::Closed;
    assert!(
        transitions::issues(
            Some(std::slice::from_ref(&base)),
            std::slice::from_ref(&closed),
            &settings,
            "42",
            Some("me")
        )[0]
        .contains("closed")
    );
    assert!(
        transitions::issues(
            Some(std::slice::from_ref(&closed)),
            std::slice::from_ref(&base),
            &settings,
            "42",
            Some("me")
        )[0]
        .contains("reopened")
    );
}

#[test]
fn every_work_item_switch_and_personal_scope_controls_delivery() {
    let before_pr = pull(1);
    let mut changed_pr = before_pr.clone();
    changed_pr.review_status = ChangeRequestReviewStatus::ChangesRequested;
    changed_pr
        .relevance
        .reasons
        .push(RelevanceReason::ReviewRequested);
    let before_issue = issue(1);
    let mut assigned_issue = before_issue.clone();
    assigned_issue.assignees.push("me".into());
    let mut settings = MonitoringSettings::default();
    settings.notifications.pull_request_opened = false;
    settings.notifications.pull_request_review_requested = false;
    settings.notifications.pull_request_changes_requested = false;
    settings.notifications.pull_request_closed = false;
    settings.notifications.pull_request_merged = false;
    settings.notifications.issue_opened = false;
    settings.notifications.issue_assigned = false;
    settings.notifications.issue_reopened = false;
    settings.notifications.issue_closed = false;
    assert!(transitions::pull_requests(Some(&[]), &[pull(2)], &settings, "42").is_empty());
    assert!(
        transitions::pull_requests(
            Some(std::slice::from_ref(&before_pr)),
            std::slice::from_ref(&changed_pr),
            &settings,
            "42"
        )
        .is_empty()
    );
    for state in [ChangeRequestState::Closed, ChangeRequestState::Merged] {
        changed_pr.state = state;
        assert!(
            transitions::pull_requests(
                Some(std::slice::from_ref(&before_pr)),
                std::slice::from_ref(&changed_pr),
                &settings,
                "42"
            )
            .is_empty()
        );
    }
    assert!(transitions::issues(Some(&[]), &[issue(2)], &settings, "42", Some("me")).is_empty());
    assert!(
        transitions::issues(
            Some(std::slice::from_ref(&before_issue)),
            std::slice::from_ref(&assigned_issue),
            &settings,
            "42",
            Some("me")
        )
        .is_empty()
    );
    let mut closed = before_issue.clone();
    closed.state = IssueState::Closed;
    assert!(
        transitions::issues(
            Some(std::slice::from_ref(&before_issue)),
            std::slice::from_ref(&closed),
            &settings,
            "42",
            Some("me")
        )
        .is_empty()
    );
    assert!(
        transitions::issues(
            Some(std::slice::from_ref(&closed)),
            std::slice::from_ref(&before_issue),
            &settings,
            "42",
            Some("me")
        )
        .is_empty()
    );

    let personal = MonitoringSettings {
        only_my_work: true,
        ..Default::default()
    };
    assert!(
        transitions::pull_requests(Some(&[]), &[pull(2)], &personal, "another-account").is_empty()
    );
    assert!(
        transitions::issues(
            Some(&[]),
            &[issue(2)],
            &personal,
            "another-account",
            Some("me")
        )
        .is_empty()
    );
    let mut unrelated = pull(2);
    unrelated.relevance = Relevance::default();
    assert!(transitions::pull_requests(Some(&[]), &[unrelated], &personal, "42").is_empty());
    assert_eq!(
        transitions::pull_requests(Some(&[]), &[pull(2)], &personal, "42").len(),
        1
    );
}

#[tokio::test]
async fn groups_events_and_deduplicates_across_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notifications.sqlite3");
    let db = Arc::new(SqliteDatabase::open(&path).unwrap());
    db.save(&connection()).unwrap();
    let sink = Arc::new(Sink::default());
    let details = Details::default();
    let service = notifier(db.clone(), sink.clone());
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(&[pull(1)]),
            Some(&[issue(1)]),
        )
        .await
        .unwrap();
    assert!(sink.sent.lock().unwrap().is_empty());
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(&[pull(1), pull(2)]),
            Some(&[issue(1), issue(2)]),
        )
        .await
        .unwrap();
    {
        let sent = sink.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].body.lines().count(), 2);
        assert_eq!(sent[0].severity, NotificationSeverity::Info);
    }
    drop(service);
    drop(db);
    let reopened = Arc::new(SqliteDatabase::open(&path).unwrap());
    notifier(reopened, sink.clone())
        .observe(
            "source",
            &repository(),
            &details,
            Some(&[pull(1), pull(2)]),
            Some(&[issue(1), issue(2)]),
        )
        .await
        .unwrap();
    assert_eq!(sink.sent.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn missing_items_are_retained_until_details_confirm_closure_and_reopen_is_detected() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    db.save(&connection()).unwrap();
    let sink = Arc::new(Sink::default());
    let service = notifier(db, sink.clone());
    let details = Details::default();
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(&[pull(1)]),
            Some(&[issue(1)]),
        )
        .await
        .unwrap();
    service
        .observe("source", &repository(), &details, Some(&[]), Some(&[]))
        .await
        .unwrap();
    assert!(sink.sent.lock().unwrap().is_empty());
    let mut closed_pr = pull(1);
    closed_pr.state = ChangeRequestState::Merged;
    let mut closed_issue = issue(1);
    closed_issue.state = IssueState::Closed;
    details.pulls.lock().unwrap().insert(1, closed_pr);
    details.issues.lock().unwrap().insert(1, closed_issue);
    service
        .observe("source", &repository(), &details, Some(&[]), Some(&[]))
        .await
        .unwrap();
    assert_eq!(sink.sent.lock().unwrap().len(), 1);
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(&[]),
            Some(&[issue(1)]),
        )
        .await
        .unwrap();
    assert!(sink.sent.lock().unwrap()[1].body.contains("reopened"));
}

#[tokio::test]
async fn disabled_and_irrelevant_events_are_not_replayed_and_accounts_have_separate_baselines() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    db.save(&connection()).unwrap();
    let sink = Arc::new(Sink::default());
    let service = notifier(db.clone(), sink.clone());
    let details = Details::default();
    service
        .observe("source", &repository(), &details, Some(&[]), Some(&[]))
        .await
        .unwrap();
    let mut settings = MonitoringSettings {
        only_my_work: true,
        ..Default::default()
    };
    settings.notifications.pull_request_opened = false;
    db.save_settings(settings).unwrap();
    let mut unrelated = issue(1);
    unrelated.relevance = Relevance::default();
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(&[pull(1)]),
            Some(std::slice::from_ref(&unrelated)),
        )
        .await
        .unwrap();
    assert!(sink.sent.lock().unwrap().is_empty());
    db.save_settings(MonitoringSettings::default()).unwrap();
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(&[pull(1)]),
            Some(std::slice::from_ref(&unrelated)),
        )
        .await
        .unwrap();
    assert!(sink.sent.lock().unwrap().is_empty());
    let mut changed_account = connection();
    changed_account.account.external_id = "new-account".into();
    db.save(&changed_account).unwrap();
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(&[pull(1), pull(2)]),
            Some(&[issue(1), issue(2)]),
        )
        .await
        .unwrap();
    assert!(sink.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn failed_delivery_is_not_retried_and_failed_lists_do_not_establish_baseline() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    db.save(&connection()).unwrap();
    let sink = Arc::new(Sink {
        fail: true,
        ..Default::default()
    });
    let service = notifier(db, sink.clone());
    let details = Details::default();
    service
        .observe("source", &repository(), &details, None, Some(&[]))
        .await
        .unwrap();
    service
        .observe("source", &repository(), &details, Some(&[pull(1)]), None)
        .await
        .unwrap();
    assert!(sink.sent.lock().unwrap().is_empty());
    assert_eq!(
        service
            .observe(
                "source",
                &repository(),
                &details,
                Some(&[pull(1), pull(2)]),
                None
            )
            .await,
        Err(Failure::DeliveryUnavailable)
    );
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(&[pull(1), pull(2)]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(sink.sent.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn missing_optional_metadata_does_not_repeat_review_notifications() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    db.save(&connection()).unwrap();
    let sink = Arc::new(Sink::default());
    let service = notifier(db, sink.clone());
    let details = Details::default();
    let mut reviewed = pull(1);
    reviewed.review_status = ChangeRequestReviewStatus::ChangesRequested;
    reviewed
        .relevance
        .reasons
        .push(RelevanceReason::ReviewRequested);
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(std::slice::from_ref(&reviewed)),
            None,
        )
        .await
        .unwrap();
    let mut unknown = reviewed.clone();
    unknown.review_status = ChangeRequestReviewStatus::Unknown;
    unknown.relevance = Relevance::default();
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(std::slice::from_ref(&unknown)),
            None,
        )
        .await
        .unwrap();
    service
        .observe(
            "source",
            &repository(),
            &details,
            Some(std::slice::from_ref(&reviewed)),
            None,
        )
        .await
        .unwrap();
    assert!(sink.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_confirmed_review_request_is_recorded_even_with_other_metadata_missing() {
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    db.save(&connection()).unwrap();
    let sink = Arc::new(Sink::default());
    let service = notifier(db, sink.clone());
    let details = Details::default();
    service
        .observe("source", &repository(), &details, Some(&[pull(1)]), None)
        .await
        .unwrap();
    let mut requested = pull(1);
    requested.relevance.complete = false;
    requested
        .relevance
        .reasons
        .push(RelevanceReason::ReviewRequested);
    for _ in 0..2 {
        service
            .observe(
                "source",
                &repository(),
                &details,
                Some(std::slice::from_ref(&requested)),
                None,
            )
            .await
            .unwrap();
    }
    assert_eq!(sink.sent.lock().unwrap().len(), 1);
}
