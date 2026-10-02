use super::*;

#[test]
fn pipeline_uses_provider_finish_time_and_preserves_unknown_states() {
    for (status, lifecycle, outcome) in [
        ("failed", RunLifecycle::Completed, RunOutcome::Failure),
        ("running", RunLifecycle::Running, RunOutcome::Unknown),
        ("manual", RunLifecycle::Queued, RunOutcome::Unknown),
        ("future_status", RunLifecycle::Unknown, RunOutcome::Unknown),
    ] {
        let pipeline: Pipeline = serde_json::from_value(serde_json::json!({"id": 12, "iid": 3, "name": null, "status": status, "source": "push", "ref": "main", "sha": "abc", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:04:00Z", "started_at": "2026-01-01T00:01:00Z", "finished_at": "2026-01-01T00:03:00Z", "web_url": "https://gitlab.com/team/repo/-/pipelines/12"})).unwrap();
        let run = WorkflowRun::from(pipeline);
        assert_eq!(run.workflow_id, WORKFLOW_ID);
        assert_eq!(run.lifecycle, lifecycle);
        assert_eq!(run.outcome, outcome);
        assert_eq!(run.updated_at, "2026-01-01T00:03:00Z");
        assert_eq!(run.run_number, 3);
    }
}
