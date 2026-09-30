use super::*;
use crate::domain::{RunLifecycle, RunOutcome};
use crate::integrations::test_support::{Exchange, MockApi};
use serde_json::json;

#[tokio::test]
async fn cloud_connection_discovers_repositories_and_reads_build_logs() {
    let auth = "authorization: Basic dXNlckBleGFtcGxlLmNvbTpzZWNyZXQ=";
    let repo = json!({"uuid":"repo-uuid","slug":"app","workspace":{"slug":"team"},"description":null,"is_private":true,"links":{"html":{"href":"https://bitbucket.org/team/app"}}});
    let pipeline = json!({"uuid":"run-uuid","build_number":7,"state":{"name":"COMPLETED","result":{"name":"FAILED"}},"target":{"ref_name":"main","commit":{"hash":"abc","message":"Build app"}},"created_on":"2026-01-01T00:00:00Z","completed_on":"2026-01-01T00:01:00Z"});
    let mut log = Exchange::json(
        "/2.0/repositories/team/repo-uuid/pipelines/run-uuid/steps/step-uuid/logs/log-uuid",
        auth,
        json!(null),
    );
    log.body = "build error".into();
    let api = MockApi::start(vec![
        Exchange::json(
            "/2.0/user",
            auth,
            json!({"uuid":"user-uuid","display_name":"Alice","nickname":"alice","links":{"html":{"href":"https://bitbucket.org/alice"}}}),
        ),
        Exchange::json(
            "/2.0/repositories/team?pagelen=1",
            auth,
            json!({"values":[]}),
        ),
        Exchange::json(
            "/2.0/repositories/team?pagelen=50",
            auth,
            json!({"values":[repo]}),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo-uuid/pipelines_config",
            auth,
            json!({"enabled":true}),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo-uuid/pipelines_config",
            auth,
            json!({"enabled":true}),
        ),
        Exchange::json(
            "/2.0/repositories/team/repo-uuid/pipelines?sort=-created_on&pagelen=50",
            auth,
            json!({"values":[pipeline.clone()]}),
        ),
        Exchange::denied("/2.0/repositories/team/repo-uuid/commit/abc", auth),
        Exchange::denied(
            "/2.0/repositories/team/repo-uuid/commit/abc/pullrequests?pagelen=50",
            auth,
        ),
        Exchange::json(
            "/2.0/repositories/team/repo-uuid/pipelines/run-uuid",
            auth,
            pipeline,
        ),
        Exchange::denied("/2.0/repositories/team/repo-uuid/commit/abc", auth),
        Exchange::denied(
            "/2.0/repositories/team/repo-uuid/commit/abc/pullrequests?pagelen=50",
            auth,
        ),
        Exchange::json(
            "/2.0/repositories/team/repo-uuid/pipelines/run-uuid/steps?pagelen=50",
            auth,
            json!({"values":[{"uuid":"step-uuid","name":"Build","log_files":[{"uuid":"log-uuid","name":"Build container"}]}]}),
        ),
        log,
    ]);
    let mut client = BitbucketClient::new().unwrap();
    client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
    let token = ProviderToken::new("secret".into());
    assert_eq!(
        client
            .validate(&config(), &token)
            .await
            .unwrap()
            .external_id,
        "user-uuid"
    );
    let repos = client.list_repositories(&config(), &token).await.unwrap();
    assert_eq!(repos.len(), 1);
    let workflows = client
        .list_workflows(&config(), &token, &repos[0])
        .await
        .unwrap();
    let runs = client
        .list_workflow_runs(&config(), &token, &repos[0])
        .await
        .unwrap();
    assert_eq!(workflows[0].id, runs[0].workflow_id);
    assert_eq!(
        client
            .workflow_run(&config(), &token, &repos[0], "run-uuid")
            .await
            .unwrap()
            .unwrap()
            .id,
        "run-uuid"
    );
    let logs = client
        .workflow_run_logs(&config(), &token, &repos[0], &runs[0])
        .await
        .unwrap();
    assert_eq!(logs.files[0].content, "build error");
    api.finish();
}

fn config() -> ConnectionConfiguration {
    [
        ("workspace".into(), "Team".into()),
        ("email".into(), "user@example.com".into()),
    ]
    .into_iter()
    .collect()
}

#[test]
fn workspace_identity_is_normalized_and_email_is_not_part_of_deduplication() {
    let client = BitbucketClient::new().unwrap();
    let configured = client.configure(&config()).unwrap();
    assert_eq!(configured.unique_key, "https://bitbucket.org/team");
    let request = client
        .request(&config(), &ProviderToken::new("secret".into()), &["user"])
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(request.url().as_str(), "https://api.bitbucket.org/2.0/user");
    assert!(
        request.headers()["Authorization"]
            .to_str()
            .unwrap()
            .starts_with("Basic ")
    );
    for workspace in ["", "../other", "https://evil.example", "team?x=1"] {
        let mut configuration = config();
        configuration.insert("workspace".into(), workspace.into());
        assert!(client.configure(&configuration).is_err());
    }
}

#[test]
fn pagination_cannot_leak_credentials_or_switch_resources() {
    let original = Url::parse("https://api.bitbucket.org/2.0/repositories/team").unwrap();
    assert!(
        safe_next(
            &original,
            "https://api.bitbucket.org/2.0/repositories/team?page=2"
        )
        .is_ok()
    );
    for next in [
        "https://evil.example/2.0/repositories/team",
        "http://api.bitbucket.org/2.0/repositories/team",
        "https://api.bitbucket.org/2.0/user",
        "https://user@api.bitbucket.org/2.0/repositories/team",
    ] {
        assert!(safe_next(&original, next).is_err());
    }
}

#[test]
fn logs_only_follow_secure_storage_redirects() {
    assert!(
        workflows::log_redirect("https://logs.s3.amazonaws.com/signed?token=temporary").is_ok()
    );
    for url in [
        "http://logs.s3.amazonaws.com/file",
        "https://127.0.0.1/file",
        "https://evil.example/file",
        "https://s3.amazonaws.com.evil.example/file",
        "https://user:pass@logs.s3.amazonaws.com/file",
    ] {
        assert!(workflows::log_redirect(url).is_err());
    }
}

#[test]
fn pipeline_state_and_build_number_map_without_fabricated_start_time() {
    let pipeline: workflows::Pipeline = serde_json::from_value(serde_json::json!({"uuid":"{run}", "build_number": 7, "state": {"name":"COMPLETED", "result":{"name":"FAILED"}}, "target":{"ref_name":"main", "commit":{"hash":"abc", "message":"Build app\nDetails"}}, "created_on":"2026-01-01T00:00:00Z", "completed_on":"2026-01-01T00:02:00Z"})).unwrap();
    let repository = Repository {
        id: "{repo}".into(),
        owner: "team".into(),
        name: "app".into(),
        description: None,
        visibility: RepositoryVisibility::Private,
        web_url: "https://bitbucket.org/team/app".into(),
    };
    let run = pipeline.into_run(&repository);
    assert_eq!(run.title, "Build app");
    assert_eq!(run.lifecycle, RunLifecycle::Completed);
    assert_eq!(run.outcome, RunOutcome::Failure);
    assert_eq!(
        run.web_url,
        "https://bitbucket.org/team/app/pipelines/results/7"
    );
    assert_eq!(run.started_at, None);
}

#[tokio::test]
async fn disabled_pipelines_do_not_request_runs() {
    let api = MockApi::start(vec![Exchange::json(
        "/2.0/repositories/team/repo-uuid/pipelines_config",
        "authorization: Basic dXNlckBleGFtcGxlLmNvbTpzZWNyZXQ=",
        json!({"enabled":false}),
    )]);
    let mut client = BitbucketClient::new().unwrap();
    client.api_base = Url::parse(&format!("{}/2.0/", api.url)).unwrap();
    let repository = Repository {
        id: "repo-uuid".into(),
        owner: "team".into(),
        name: "app".into(),
        description: None,
        visibility: RepositoryVisibility::Private,
        web_url: "https://bitbucket.org/team/app".into(),
    };
    assert!(
        client
            .list_workflow_runs(&config(), &ProviderToken::new("secret".into()), &repository)
            .await
            .unwrap()
            .is_empty()
    );
    api.finish();
}
