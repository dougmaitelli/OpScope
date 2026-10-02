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

struct PersonalSettings(Mutex<bool>);

#[tokio::test]
async fn disabling_workflow_alerts_advances_state_without_replay() {
    use crate::application::{MonitoringSettings, SettingsRepository};
    let settings = Arc::new(crate::persistence::SqliteDatabase::in_memory().unwrap());
    let mut disabled = MonitoringSettings::default();
    disabled.notifications.workflow_failures = false;
    settings.save_settings(disabled).unwrap();
    let sink = Arc::new(RecordingSink::default());
    let notifier = NotifyRepositoryFailures::new(Arc::new(MemoryStates::default()), sink.clone())
        .with_settings(settings.clone());
    notifier
        .observe("1", "source", &repository(), &workflows(), &[])
        .await
        .unwrap();
    let failed = run("workflow-1", "run-1", 1, RunOutcome::Failure);
    notifier
        .observe(
            "1",
            "source",
            &repository(),
            &workflows(),
            std::slice::from_ref(&failed),
        )
        .await
        .unwrap();
    settings
        .save_settings(MonitoringSettings::default())
        .unwrap();
    notifier
        .observe("1", "source", &repository(), &workflows(), &[failed])
        .await
        .unwrap();
    assert!(sink.0.lock().unwrap().is_empty());
    notifier
        .observe(
            "1",
            "source",
            &repository(),
            &workflows(),
            &[run("workflow-1", "run-2", 2, RunOutcome::Failure)],
        )
        .await
        .unwrap();
    assert_eq!(sink.0.lock().unwrap().len(), 1);
}

impl crate::application::SettingsRepository for PersonalSettings {
    fn load_settings(&self) -> Result<crate::application::MonitoringSettings, PersistenceFailure> {
        Ok(crate::application::MonitoringSettings {
            only_my_work: *self.0.lock().unwrap(),
            ..Default::default()
        })
    }
    fn save_settings(
        &self,
        settings: crate::application::MonitoringSettings,
    ) -> Result<(), PersistenceFailure> {
        *self.0.lock().unwrap() = settings.only_my_work;
        Ok(())
    }
}

#[tokio::test]
async fn personal_scope_filters_before_aggregation_without_replaying_on_toggle() {
    let states = Arc::new(MemoryStates::default());
    let sink = Arc::new(RecordingSink::default());
    let settings = Arc::new(PersonalSettings(Mutex::new(true)));
    let notifier =
        NotifyRepositoryFailures::new(states, sink.clone()).with_settings(settings.clone());
    notifier
        .observe("1", "source", &repository(), &workflows(), &[])
        .await
        .unwrap();
    let mut mine = run("workflow-1", "mine", 1, RunOutcome::Failure);
    mine.relationships.commit_authors.ids.push("1".into());
    let other = run("workflow-2", "other", 2, RunOutcome::Failure);
    notifier
        .observe(
            "1",
            "source",
            &repository(),
            &workflows(),
            &[mine.clone(), other.clone()],
        )
        .await
        .unwrap();
    {
        let sent = sink.0.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].body, "Build");
    }
    *settings.0.lock().unwrap() = false;
    notifier
        .observe("1", "source", &repository(), &workflows(), &[mine, other])
        .await
        .unwrap();
    assert_eq!(sink.0.lock().unwrap().len(), 1);
    notifier
        .observe(
            "1",
            "source",
            &repository(),
            &workflows(),
            &[run("workflow-2", "new", 3, RunOutcome::Failure)],
        )
        .await
        .unwrap();
    assert_eq!(sink.0.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn personal_history_never_replaces_the_actual_latest_run_for_notifications() {
    let sink = Arc::new(RecordingSink::default());
    let notifier = NotifyRepositoryFailures::new(Arc::new(MemoryStates::default()), sink.clone())
        .with_settings(Arc::new(PersonalSettings(Mutex::new(true))));
    notifier
        .observe("1", "source", &repository(), &workflows(), &[])
        .await
        .unwrap();
    let mut history = run("workflow-1", "mine", 1, RunOutcome::Failure);
    history
        .relationships
        .change_request_authors
        .ids
        .push("1".into());
    notifier
        .observe(
            "1",
            "source",
            &repository(),
            &workflows(),
            &[history, run("workflow-1", "other", 2, RunOutcome::Failure)],
        )
        .await
        .unwrap();
    assert!(sink.0.lock().unwrap().is_empty());
}

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
        relationships: Default::default(),
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
            "1",
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
        .observe("1", "source", &repository, &workflows, &[])
        .await
        .expect("baseline succeeds");
    let failed_runs = vec![
        run("workflow-1", "run-1", 1, RunOutcome::Failure),
        run("workflow-2", "run-2", 2, RunOutcome::Failure),
    ];

    notifier
        .observe("1", "source", &repository, &workflows, &failed_runs)
        .await
        .expect("notification succeeds");
    notifier
        .observe("1", "source", &repository, &workflows, &failed_runs)
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
        .observe("1", "source", &repository, &workflows, &[])
        .await
        .expect("baseline succeeds");

    for (id, number) in [("run-1", 1), ("run-2", 2)] {
        notifier
            .observe(
                "1",
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
            "1",
            "source",
            &repository,
            &workflows,
            &[history.clone(), latest.clone()],
        )
        .await
        .expect("baseline succeeds");
    notifier
        .observe(
            "1",
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
            "1",
            "source",
            &repository,
            &workflows,
            &[history, run("workflow-1", "run-2", 2, RunOutcome::Failure)],
        )
        .await
        .expect("failure transition succeeds");

    assert_eq!(sink.0.lock().expect("notification lock").len(), 1);
}
