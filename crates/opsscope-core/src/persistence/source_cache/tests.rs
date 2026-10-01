use super::*;
use crate::application::{
    ConnectionRepository, SecretReference, StoredConnection, ValidatedAccount,
};
use tempfile::tempdir;

fn test_connection() -> StoredConnection {
    StoredConnection {
        id: "example".to_owned(),
        source_id: "example".to_owned(),
        unique_key: "example".to_owned(),
        label: "Example".to_owned(),
        configuration: Default::default(),
        account: ValidatedAccount {
            external_id: "42".to_owned(),
            name: "The Octocat".to_owned(),
            handle: Some("octocat".to_owned()),
            profile_url: Some("https://example.com/octocat".to_owned()),
        },
        secret_reference: SecretReference::for_connection("example", "test"),
    }
}

#[test]
fn queued_runs_without_timestamps_remain_latest_after_cache_reload()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let database = SqliteDatabase::open(directory.path().join("opsscope.sqlite3"))?;
    database.save(&test_connection())?;
    let queued = WorkflowRun {
        relationships: Default::default(),
        id: "new".into(),
        workflow_id: "build.yml".into(),
        run_number: 8,
        attempt: 1,
        title: "Queued build".into(),
        lifecycle: RunLifecycle::Queued,
        outcome: RunOutcome::Unknown,
        branch: Some("main".into()),
        commit_sha: "abc".into(),
        actor: None,
        trigger: "push".into(),
        created_at: String::new(),
        started_at: None,
        updated_at: String::new(),
        web_url: "https://gitea.example.com/team/app/actions/runs/8".into(),
        provider_status: "queued".into(),
        provider_conclusion: None,
    };
    let older = WorkflowRun {
        id: "old".into(),
        run_number: 7,
        attempt: 3,
        lifecycle: RunLifecycle::Completed,
        outcome: RunOutcome::Failure,
        created_at: "2026-09-28T00:00:00Z".into(),
        ..queued.clone()
    };
    database.replace_workflow_runs(
        "example",
        "42",
        "repository-1",
        &WorkflowRunSnapshot {
            last_attempted_at: 123,
            last_successful_at: Some(123),
            last_error: None,
            runs: vec![older, queued],
        },
    )?;
    let cached = database
        .workflow_runs("example", "42", "repository-1")?
        .unwrap();
    assert_eq!(cached.runs[0].id, "new");
    assert_eq!(cached.runs[0].lifecycle, RunLifecycle::Queued);
    Ok(())
}

#[test]
fn snapshots_persist_and_support_empty_results() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let database_path = directory.path().join("opsscope.sqlite3");
    let repositories = RepositorySnapshot {
        refreshed_at: 123,
        repositories: vec![Repository {
            id: "repository-1".to_owned(),
            owner: "octocat".to_owned(),
            name: "hello-world".to_owned(),
            description: Some("A cached repository".to_owned()),
            visibility: RepositoryVisibility::Private,
            web_url: "https://example.com/octocat/hello-world".to_owned(),
        }],
    };
    let workflows = WorkflowSnapshot {
        refreshed_at: 124,
        workflows: Vec::new(),
    };
    let runs = WorkflowRunSnapshot {
        last_attempted_at: 125,
        last_successful_at: Some(125),
        last_error: None,
        runs: vec![WorkflowRun {
            relationships: crate::domain::Relationships {
                commit_authors: crate::domain::AccountSet::new(["42".into()], true),
                ..Default::default()
            },
            id: "run-1".to_owned(),
            workflow_id: "workflow-1".to_owned(),
            run_number: 7,
            attempt: 2,
            title: "Build main".to_owned(),
            lifecycle: RunLifecycle::Completed,
            outcome: RunOutcome::Success,
            branch: Some("main".to_owned()),
            commit_sha: "abcdef123456".to_owned(),
            actor: Some("octocat".to_owned()),
            trigger: "push".to_owned(),
            created_at: "2026-09-26T18:00:00Z".to_owned(),
            started_at: Some("2026-09-26T18:00:02Z".to_owned()),
            updated_at: "2026-09-26T18:03:00Z".to_owned(),
            web_url: "https://example.com/runs/1".to_owned(),
            provider_status: "completed".to_owned(),
            provider_conclusion: Some("success".to_owned()),
        }],
    };
    let mut change_requests = ChangeRequestSnapshot {
        refreshed_at: 126,
        change_requests: vec![ChangeRequest {
            relationships: crate::domain::Relationships {
                authors: crate::domain::AccountSet::new(["42".into()], true),
                reviewers: crate::domain::AccountSet::new(["99".into()], true),
                viewers: vec![crate::domain::ViewerRelationships {
                    account_id: "42".into(),
                    review_requested: Some(false),
                    ..Default::default()
                }],
                ..Default::default()
            },
            id: "change-1".to_owned(),
            number: 42,
            title: "Harden authentication".to_owned(),
            author: Some("octocat".to_owned()),
            source_branch: "auth-fix".to_owned(),
            target_branch: "main".to_owned(),
            state: ChangeRequestState::Open,
            draft: false,
            review_status: ChangeRequestReviewStatus::Approved,
            check_status: ChangeRequestCheckStatus::Passed,
            merge_status: ChangeRequestMergeStatus::Ready,
            created_at: "2026-09-26T18:00:00Z".to_owned(),
            updated_at: "2026-09-27T18:00:00Z".to_owned(),
            web_url: "https://example.com/pulls/42".to_owned(),
        }],
    };
    let mut no_signals = change_requests.change_requests[0].clone();
    no_signals.id = "change-no-signals".to_owned();
    no_signals.number = 41;
    no_signals.web_url = "https://example.com/pulls/41".to_owned();
    no_signals.check_status = ChangeRequestCheckStatus::None;
    no_signals.review_status = ChangeRequestReviewStatus::None;
    change_requests.change_requests.push(no_signals);
    let change_request_details = ChangeRequestDetailsSnapshot {
        refreshed_at: 127,
        details: crate::domain::ChangeRequestDetails {
            change_request: change_requests.change_requests[0].clone(),
            body: Some("Improves token handling.".to_owned()),
            labels: vec!["security".to_owned()],
            reviews: vec![crate::domain::ChangeRequestReview {
                reviewer: Some("reviewer".to_owned()),
                status: ChangeRequestReviewStatus::Approved,
                submitted_at: Some("2026-09-27T17:00:00Z".to_owned()),
            }],
            checks: vec![crate::domain::ChangeRequestCheck {
                name: "test".to_owned(),
                status: ChangeRequestCheckStatus::Passed,
                web_url: Some("https://example.com/checks/1".to_owned()),
                workflow_run_id: Some("123".to_owned()),
            }],
            latest_commit: Some(crate::domain::ChangeRequestCommit {
                sha: "abcdef123456".to_owned(),
                title: "Harden tokens".to_owned(),
                author: Some("octocat".to_owned()),
                committed_at: "2026-09-27T16:00:00Z".to_owned(),
            }),
        },
    };

    {
        let database = SqliteDatabase::open(&database_path)?;
        database.save(&test_connection())?;
        database.replace_repositories("example", "42", &repositories)?;
        database.replace_workflows("example", "42", "repository-1", &workflows)?;
        database.replace_workflow_runs("example", "42", "repository-1", &runs)?;
        database.replace_change_requests("example", "42", "repository-1", &change_requests)?;
        database.replace_change_request_details(
            "example",
            "42",
            "repository-1",
            42,
            &change_request_details,
        )?;
    }

    let database = SqliteDatabase::open(&database_path)?;
    assert_eq!(database.repositories("example", "42")?, Some(repositories));
    assert!(
        database
            .repositories("example", "different-account")?
            .is_none()
    );
    assert_eq!(
        database.workflows("example", "42", "repository-1")?,
        Some(workflows)
    );
    assert_eq!(
        database.workflow_runs("example", "42", "repository-1")?,
        Some(runs)
    );
    let persisted = database
        .change_requests("example", "42", "repository-1")?
        .unwrap();
    let facts = &persisted.change_requests[0].relationships;
    assert_eq!(
        facts.evaluate("42").reasons,
        [crate::domain::RelevanceReason::Authored]
    );
    assert_eq!(
        facts.evaluate("99").reasons,
        [crate::domain::RelevanceReason::Reviewed]
    );
    assert_eq!(facts.review_requested("99"), None);
    assert!(
        database
            .change_requests("example", "99", "repository-1")?
            .is_none()
    );
    assert_eq!(
        database.change_requests("example", "42", "repository-1")?,
        Some(change_requests.clone())
    );
    assert_eq!(
        database.change_request_details("example", "42", "repository-1", 42)?,
        Some(change_request_details.clone())
    );
    let other = ChangeRequest {
        id: "other".into(),
        number: 43,
        ..change_requests.change_requests[0].clone()
    };
    let snapshot = ChangeRequestSnapshot {
        refreshed_at: 126,
        change_requests: vec![change_requests.change_requests[0].clone(), other.clone()],
    };
    database.replace_change_requests("example", "42", "repository-1", &snapshot)?;
    database.replace_change_requests("example", "99", "repository-1", &snapshot)?;
    let mut updated = change_request_details;
    updated.refreshed_at = 200;
    updated.details.change_request.title = "Updated individually".into();
    database.replace_change_request_details("example", "42", "repository-1", 42, &updated)?;
    let cached = database
        .change_requests("example", "42", "repository-1")?
        .unwrap();
    assert_eq!(cached.refreshed_at, 126);
    assert!(
        cached
            .change_requests
            .contains(&updated.details.change_request)
    );
    assert!(cached.change_requests.contains(&other));
    updated.details.change_request.state = ChangeRequestState::Merged;
    database.replace_change_request_details("example", "42", "repository-1", 42, &updated)?;
    let cached = database
        .change_requests("example", "42", "repository-1")?
        .unwrap();
    assert_eq!(cached.refreshed_at, 126);
    assert_eq!(cached.change_requests, vec![other]);
    assert_eq!(
        database.change_request_details("example", "42", "repository-1", 42)?,
        Some(updated)
    );
    assert_eq!(
        database
            .change_requests("example", "99", "repository-1")?
            .unwrap()
            .change_requests
            .len(),
        2
    );
    database.delete("example")?;
    assert!(database.repositories("example", "42")?.is_none());
    assert!(
        database
            .workflows("example", "42", "repository-1")?
            .is_none()
    );
    assert!(
        database
            .workflow_runs("example", "42", "repository-1")?
            .is_none()
    );
    assert!(
        database
            .change_requests("example", "42", "repository-1")?
            .is_none()
    );
    assert!(
        database
            .change_request_details("example", "42", "repository-1", 42)?
            .is_none()
    );
    Ok(())
}
