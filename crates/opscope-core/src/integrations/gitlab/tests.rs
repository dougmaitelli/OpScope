use super::*;
use crate::integrations::test_support::{Exchange, MockApi};
use serde_json::json;

#[tokio::test]
async fn discovers_projects_and_reads_pipeline_logs_through_the_real_http_adapter() {
    let auth = "private-token: test-token";
    let project = json!({"id":3,"path":"repo","namespace":{"full_path":"team/sub"},"description":null,"visibility":"private","archived":false,"web_url":"https://gitlab.com/team/sub/repo"});
    let mut archived = project.clone();
    archived["id"] = json!(4);
    archived["archived"] = json!(true);
    let mut page1 = Exchange::json(
        "/api/v4/projects?membership=true&archived=false&order_by=id&sort=asc&per_page=50&page=1",
        auth,
        json!([archived]),
    );
    page1.headers.push_str("X-Next-Page: 2\r\n");
    let pipeline = json!({"id":12,"iid":3,"name":"CI","status":"failed","source":"push","ref":"main","sha":"abc","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:01:00Z","web_url":"https://gitlab.com/team/sub/repo/-/pipelines/12"});
    let mut log = Exchange::json("/api/v4/projects/3/jobs/20/trace", auth, json!(null));
    log.body = "build failed\n".into();
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v4/user",
            auth,
            json!({"id":1,"username":"alice","name":"Alice","web_url":"https://gitlab.com/alice"}),
        ),
        page1,
        Exchange::json(
            "/api/v4/projects?membership=true&archived=false&order_by=id&sort=asc&per_page=50&page=2",
            auth,
            json!([project]),
        ),
        Exchange::json(
            "/api/v4/projects/3",
            auth,
            json!({"builds_access_level":"enabled","ci_config_path":null}),
        ),
        Exchange::json(
            "/api/v4/projects/3",
            auth,
            json!({"builds_access_level":"enabled","ci_config_path":null}),
        ),
        Exchange::json(
            "/api/v4/projects/3/pipelines?order_by=id&sort=desc&per_page=50&page=1",
            auth,
            json!([pipeline.clone()]),
        ),
        Exchange::denied("/api/v4/user", auth),
        Exchange::denied("/api/v4/projects/3/repository/commits/abc", auth),
        Exchange::denied(
            "/api/v4/projects/3/repository/commits/abc/merge_requests?per_page=50&page=1",
            auth,
        ),
        Exchange::json("/api/v4/projects/3/pipelines/12", auth, pipeline),
        Exchange::denied("/api/v4/user", auth),
        Exchange::denied("/api/v4/projects/3/repository/commits/abc", auth),
        Exchange::denied(
            "/api/v4/projects/3/repository/commits/abc/merge_requests?per_page=50&page=1",
            auth,
        ),
        Exchange::json(
            "/api/v4/projects/3/pipelines/12/jobs?include_retried=false&per_page=50&page=1",
            auth,
            json!([{"id":20,"name":"build","stage":"test"}]),
        ),
        log,
    ]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("test-token".into());
    assert_eq!(
        client
            .validate(&config, &token)
            .await
            .unwrap()
            .handle
            .as_deref(),
        Some("alice")
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
    assert_eq!(runs[0].workflow_id, workflows[0].id);
    assert!(runs[0].started_at.is_none());
    assert_eq!(
        client
            .workflow_run(&config, &token, &repos[0], "12")
            .await
            .unwrap()
            .unwrap()
            .id,
        "12"
    );
    let logs = client
        .workflow_run_logs(&config, &token, &repos[0], &runs[0])
        .await
        .unwrap();
    assert_eq!(logs.files[0].content, "build failed\n");
    api.finish();
}

#[test]
fn requests_use_private_token_and_nested_project_ids_are_encoded() {
    let client = GitLabClient::new().unwrap();
    let config = client
        .configure(
            &[("serverUrl".into(), "https://gitlab.example.com".into())]
                .into_iter()
                .collect(),
        )
        .unwrap();
    let token = ProviderToken::new("test-token".into());
    let request = client
        .request(
            &config.configuration,
            &token,
            &["projects", "team/sub/repo", "pipelines"],
        )
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        request.url().as_str(),
        "https://gitlab.example.com/api/v4/projects/team%2Fsub%2Frepo/pipelines"
    );
    assert_eq!(request.headers()["PRIVATE-TOKEN"], "test-token");
    assert!(!request.url().as_str().contains("test-token"));
}

#[test]
fn projects_keep_subgroup_ownership_and_internal_visibility_private() {
    let project: Project = serde_json::from_value(serde_json::json!({"id": 3, "path": "repo", "namespace": {"full_path":"team/sub"}, "description": null, "visibility": "internal", "archived": false, "web_url": "https://gitlab.com/team/sub/repo"})).unwrap();
    let repository = Repository::from(project);
    assert_eq!(repository.owner, "team/sub");
    assert_eq!(repository.visibility, RepositoryVisibility::Private);
}

#[tokio::test]
async fn disabled_ci_does_not_request_pipeline_runs() {
    let api = MockApi::start(vec![Exchange::json(
        "/api/v4/projects/3",
        "private-token: test-token",
        json!({"builds_access_level":"disabled"}),
    )]);
    let client = GitLabClient::new().unwrap();
    let config = [("serverUrl".into(), api.url.clone())]
        .into_iter()
        .collect();
    let repository = Repository {
        id: "3".into(),
        owner: "team".into(),
        name: "repo".into(),
        description: None,
        visibility: RepositoryVisibility::Private,
        web_url: "https://gitlab.com/team/repo".into(),
    };
    assert!(
        client
            .list_workflow_runs(
                &config,
                &ProviderToken::new("test-token".into()),
                &repository
            )
            .await
            .unwrap()
            .is_empty()
    );
    api.finish();
}
