
use super::*;
use crate::application::PersistenceFailure;
use crate::domain::{RepositoryVisibility, WorkflowState};
use std::sync::Mutex;

#[derive(Default)]
struct MemoryStates(Mutex<Option<Vec<LatestRunNotificationState>>>);

impl NotificationStateRepository for MemoryStates {
    fn load(
        &self,
        _source_id: &str,
        _repository_id: &str,
    ) -> Result<Option<Vec<LatestRunNotificationState>>, PersistenceFailure> {
        Ok(self.0.lock().expect("state lock").clone())
    }

    fn replace(
        &self,
        _source_id: &str,
        _repository_id: &str,
        states: &[LatestRunNotificationState],
    ) -> Result<(), PersistenceFailure> {
        *self.0.lock().expect("state lock") = Some(states.to_vec());
        Ok(())
    }
}

#[derive(Default)]
struct RecordingSink(Mutex<Vec<Notification>>);

#[async_trait]
impl NotificationSink for RecordingSink {
    async fn send(&self, notification: &Notification) -> Result<(), NotificationDeliveryFailure> {
        self.0
            .lock()
            .expect("notification lock")
            .push(notification.clone());
        Ok(())
    }
}

fn repository() -> Repository {
    Repository {
        id: "repository-1".to_owned(),
        owner: "owner".to_owned(),
        name: "project".to_owned(),
        description: None,
        visibility: RepositoryVisibility::Private,
        web_url: "https://example.com/owner/project".to_owned(),
    }
}

fn workflows() -> Vec<Workflow> {
    [("workflow-1", "Build"), ("workflow-2", "Test")]
        .into_iter()
        .map(|(id, name)| Workflow {
            id: id.to_owned(),
            name: name.to_owned(),
            path: format!("{id}.yml"),
            state: WorkflowState::Active,
            web_url: format!("https://example.com/{id}"),
        })
        .collect()
}

fn run(workflow_id: &str, id: &str, number: u64, outcome: RunOutcome) -> WorkflowRun {
    WorkflowRun {
        id: id.to_owned(),
        workflow_id: workflow_id.to_owned(),
        run_number: number,
        attempt: 1,
        title: id.to_owned(),
        lifecycle: RunLifecycle::Completed,
        outcome,
        branch: Some("main".to_owned()),
        commit_sha: "abcdef123456".to_owned(),
        actor: None,
        trigger: "push".to_owned(),
        created_at: format!("2026-09-26T18:{number:02}:00Z"),
        started_at: None,
        updated_at: format!("2026-09-26T18:{number:02}:30Z"),
        web_url: format!("https://example.com/runs/{id}"),
        provider_status: "completed".to_owned(),
        provider_conclusion: None,
    }
}

#[tokio::test]
async fn establishes_a_baseline_without_notifying_about_an_existing_failure() {
    let states = Arc::new(MemoryStates::default());
    let sink = Arc::new(RecordingSink::default());
    let notifier = NotifyRepositoryFailures::new(states, sink.clone());

    notifier
        .observe(
            "source",
            &repository(),
            &workflows(),
            &[run("workflow-1", "run-1", 1, RunOutcome::Failure)],
        )
        .await
        .expect("baseline succeeds");

    assert!(sink.0.lock().expect("notification lock").is_empty());
}

#[tokio::test]
async fn groups_new_latest_failures_once_per_repository() {
    let states = Arc::new(MemoryStates::default());
    let sink = Arc::new(RecordingSink::default());
    let notifier = NotifyRepositoryFailures::new(states, sink.clone());
    let repository = repository();
    let workflows = workflows();
    notifier
        .observe("source", &repository, &workflows, &[])
        .await
        .expect("baseline succeeds");
    let failed_runs = vec![
        run("workflow-1", "run-1", 1, RunOutcome::Failure),
        run("workflow-2", "run-2", 2, RunOutcome::Failure),
    ];

    notifier
        .observe("source", &repository, &workflows, &failed_runs)
        .await
        .expect("notification succeeds");
    notifier
        .observe("source", &repository, &workflows, &failed_runs)
        .await
        .expect("repeated observation succeeds");

    let sent = sink.0.lock().expect("notification lock");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].title, "2 workflows failed in owner/project");
    assert_eq!(sent[0].body, "Build, Test");
    assert_eq!(sent[0].severity, NotificationSeverity::Failure);
}

#[tokio::test]
async fn notifies_when_a_new_latest_run_fails_after_an_earlier_failure() {
    let states = Arc::new(MemoryStates::default());
    let sink = Arc::new(RecordingSink::default());
    let notifier = NotifyRepositoryFailures::new(states, sink.clone());
    let repository = repository();
    let workflows = workflows();
    notifier
        .observe("source", &repository, &workflows, &[])
        .await
        .expect("baseline succeeds");

    for (id, number) in [("run-1", 1), ("run-2", 2)] {
        notifier
            .observe(
                "source",
                &repository,
                &workflows,
                &[run("workflow-1", id, number, RunOutcome::Failure)],
            )
            .await
            .expect("notification succeeds");
    }

    assert_eq!(sink.0.lock().expect("notification lock").len(), 2);
}

#[tokio::test]
async fn ignores_failed_history_and_notifies_when_the_latest_run_enters_failure() {
    let states = Arc::new(MemoryStates::default());
    let sink = Arc::new(RecordingSink::default());
    let notifier = NotifyRepositoryFailures::new(states, sink.clone());
    let repository = repository();
    let workflows = workflows();
    let mut latest = run("workflow-1", "run-2", 2, RunOutcome::Unknown);
    latest.lifecycle = RunLifecycle::Running;
    let history = run("workflow-1", "run-1", 1, RunOutcome::Failure);

    notifier
        .observe(
            "source",
            &repository,
            &workflows,
            &[history.clone(), latest.clone()],
        )
        .await
        .expect("baseline succeeds");
    notifier
        .observe(
            "source",
            &repository,
            &workflows,
            &[history.clone(), latest],
        )
        .await
        .expect("unchanged latest run succeeds");
    assert!(sink.0.lock().expect("notification lock").is_empty());

    notifier
        .observe(
            "source",
            &repository,
            &workflows,
            &[history, run("workflow-1", "run-2", 2, RunOutcome::Failure)],
        )
        .await
        .expect("failure transition succeeds");

    assert_eq!(sink.0.lock().expect("notification lock").len(), 1);
}
