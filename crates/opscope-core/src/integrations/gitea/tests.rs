use super::*;
use crate::domain::{RunLifecycle, RunOutcome};
use crate::integrations::test_support::{Exchange, MockApi};
use serde_json::json;

#[tokio::test]
async fn actions_workflows_runs_and_logs_use_gitea_routes_and_attempts() {
    let auth = "authorization: token test-token";
    let repo = json!({"id":3,"owner":{"login":"team"},"name":"app","description":null,"private":true,"archived":false,"html_url":"https://gitea.com/team/app"});
    let mut archived = repo.clone();
    archived["archived"] = json!(true);
    let mut page1 = Exchange::json(
        "/api/v1/user/repos?limit=50&page=1",
        auth,
        json!([archived]),
    );
    page1.headers.push_str("X-Total-Count: 2\r\n");
    let run = json!({"id":100,"path":"build.yml","run_number":3,"run_attempt":2,"display_title":"Build app","status":"completed","conclusion":"failure","head_branch":"main","head_sha":"abc","actor":{"login":"alice"},"event":"push","started_at":"2026-01-01T00:00:00Z","completed_at":"2026-01-01T00:01:00Z","html_url":"https://gitea.com/team/app/actions/runs/3"});
    let mut log = Exchange::json(
        "/api/v1/repos/team/app/actions/jobs/20/logs",
        auth,
        json!(null),
    );
    log.body = "test failure".into();
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v1/user",
            auth,
            json!({"id":1,"login":"alice","full_name":"Alice","html_url":"https://gitea.com/alice"}),
        ),
        page1,
        Exchange::json("/api/v1/user/repos?limit=50&page=2", auth, json!([repo])),
        Exchange::json(
            "/api/v1/repos/team/app/actions/workflows",
            auth,
            json!({"total_count":1,"workflows":[{"id":"build.yml","name":"Build","path":".gitea/workflows/build.yml","state":"active","html_url":"https://gitea.com/team/app/actions?workflow=build.yml"}]}),
        ),
        Exchange::json(
            "/api/v1/repos/team/app/actions/runs?limit=50&page=1",
            auth,
            json!({"total_count":1,"workflow_runs":[run.clone()]}),
        ),
        Exchange::denied("/api/v1/repos/team/app/git/commits/abc", auth),
        Exchange::denied("/api/v1/repos/team/app/commits/abc/pull", auth),
        Exchange::json("/api/v1/repos/team/app/actions/runs/100", auth, run),
        Exchange::denied("/api/v1/repos/team/app/git/commits/abc", auth),
        Exchange::denied("/api/v1/repos/team/app/commits/abc/pull", auth),
        Exchange::json(
            "/api/v1/repos/team/app/actions/runs/100/attempts/2/jobs?limit=50&page=1",
            auth,
            json!({"total_count":1,"jobs":[{"id":20,"name":"test"}]}),
        ),
        log,
    ]);
    let client = GiteaClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("test-token".into());
    assert_eq!(
        client.validate(&config, &token).await.unwrap().external_id,
        "1"
    );
    let repos = client.list_repositories(&config, &token).await.unwrap();
    assert_eq!(repos.len(), 1);
    let workflows = client
        .list_workflows(&config, &token, &repos[0])
        .await
        .unwrap();
    let runs = client
        .list_workflow_runs(&config, &token, &repos[0])
        .await
        .unwrap();
    assert_eq!(workflows[0].id, runs[0].workflow_id);
    assert_eq!(
        client
            .workflow_run(&config, &token, &repos[0], "100")
            .await
            .unwrap()
            .unwrap()
            .id,
        "100"
    );
    let logs = client
        .workflow_run_logs(&config, &token, &repos[0], &runs[0])
        .await
        .unwrap();
    assert_eq!(logs.files[0].content, "test failure");
    api.finish();
}

#[test]
fn gitea_requests_use_the_configured_origin_and_token_auth() {
    let client = GiteaClient::new().unwrap();
    let config = client
        .configure(
            &[("serverUrl".into(), "https://git.example.com/".into())]
                .into_iter()
                .collect(),
        )
        .unwrap();
    let request = client
        .request(
            &config.configuration,
            &ProviderToken::new("test-token".into()),
            &["user", "repos"],
        )
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        request.url().as_str(),
        "https://git.example.com/api/v1/user/repos"
    );
    assert_eq!(request.headers()["Authorization"], "token test-token");
}

#[test]
fn runs_use_workflow_filename_not_numeric_run_id() {
    let value: workflows::Run = serde_json::from_value(serde_json::json!({"id": 100, "path": ".gitea/workflows/build.yml", "run_number": 3, "run_attempt": 2, "display_title": "Build app", "status": "completed", "conclusion": "failure", "head_branch": "main", "head_sha": "abc", "actor": {"login": "alice"}, "event": "push", "started_at": "2026-01-01T00:00:00Z", "completed_at": "2026-01-01T00:01:00Z", "html_url": "https://git.example.com/team/repo/actions/runs/3"})).unwrap();
    let run = WorkflowRun::from(value);
    assert_eq!(run.id, "100");
    assert_eq!(run.workflow_id, "build.yml");
    assert_eq!(run.attempt, 2);
    assert_eq!(run.lifecycle, RunLifecycle::Completed);
    assert_eq!(run.outcome, RunOutcome::Failure);
    assert_eq!(run.updated_at, "2026-01-01T00:01:00Z");
}
