use super::*;
use crate::integrations::test_support::{Exchange, MockApi, relevance_repository, relevance_run};
use reqwest::header::AUTHORIZATION;
use serde_json::json;
use std::io::Write;
use zip::write::SimpleFileOptions;

#[tokio::test]
async fn feature_operations_execute_authenticated_enterprise_routes() {
    let auth = "authorization: Bearer test-token";
    let repository = relevance_repository();
    let mut run = relevance_run("abc");
    run.id = "42".into();
    run.attempt = 2;
    let mut missing = Exchange::json("/api/v3/repos/team/app/actions/runs/42", auth, json!({}));
    missing.status = 404;
    let mut logs = Exchange::json(
        "/api/v3/repos/team/app/actions/runs/42/attempts/2/logs",
        auth,
        json!({}),
    );
    logs.status = 302;
    logs.headers
        .push_str("Location: http://127.0.0.1/unsafe-archive\r\n");
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v3/user",
            auth,
            json!({"id":1,"login":"alice","name":null,"html_url":"https://github.example/alice"}),
        ),
        Exchange::json("/api/v3/user/repos?per_page=100&page=1", auth, json!([])),
        Exchange::json(
            "/api/v3/repos/team/app/actions/workflows?per_page=100&page=1",
            auth,
            json!({"workflows":[]}),
        ),
        Exchange::json(
            "/api/v3/repos/team/app/actions/runs?per_page=100&page=1",
            auth,
            json!({"workflow_runs":[]}),
        ),
        missing,
        logs,
    ]);
    let client = GitHubClient::new().unwrap();
    let config = [(SERVER_URL_KEY.into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("test-token".into());
    assert_eq!(
        client.validate(&config, &token).await.unwrap().external_id,
        "1"
    );
    assert!(
        client
            .list_repositories(&config, &token)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        client
            .list_workflows(&config, &token, &repository)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        client
            .list_workflow_runs(&config, &token, &repository)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        client
            .workflow_run(&config, &token, &repository, "42")
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        client
            .workflow_run_logs(&config, &token, &repository, &run)
            .await,
        Err(WorkflowRunLogsFailure::UnexpectedResponse)
    );
    assert_eq!(
        client
            .workflow_run(&config, &token, &repository, "../42")
            .await,
        Err(ConnectionValidationFailure::UnexpectedResponse)
    );
    api.finish();
}

#[tokio::test]
async fn workflow_operations_keep_pagination_in_the_feature_module() {
    let auth = "authorization: Bearer test-token";
    let workflow = json!({"id":1,"name":"Build","path":"ci.yml","state":"active","html_url":"https://github.example/team/app/actions/1"});
    let api = MockApi::start(vec![
        Exchange::json(
            "/api/v3/repos/team/app/actions/workflows?per_page=100&page=1",
            auth,
            json!({"workflows":vec![workflow;100]}),
        ),
        Exchange::json(
            "/api/v3/repos/team/app/actions/workflows?per_page=100&page=2",
            auth,
            json!({"workflows":[]}),
        ),
    ]);
    let client = GitHubClient::new().unwrap();
    let config = [(SERVER_URL_KEY.into(), api.url.clone())]
        .into_iter()
        .collect();
    assert_eq!(
        client
            .list_workflows(
                &config,
                &ProviderToken::new("test-token".into()),
                &relevance_repository()
            )
            .await
            .unwrap()
            .len(),
        100
    );
    api.finish();
}

#[tokio::test]
async fn pull_requests_execute_graphql_with_cursor_and_details_variables() {
    let auth = "authorization: Bearer test-token";
    let repo = relevance_repository();
    let api = MockApi::start(vec![
        Exchange::graphql(
            "/api/graphql",
            auth,
            json!({"owner":"team","name":"app","first":100,"after":null}),
            &[
                "query OpenChangeRequests",
                "pullRequests(first:",
                "statusCheckRollup { state }",
            ],
            json!({"data":{"repository":{"pullRequests":{"nodes":[],"pageInfo":{"hasNextPage":true,"endCursor":"next"}}}}}),
        ),
        Exchange::graphql(
            "/api/graphql",
            auth,
            json!({"owner":"team","name":"app","first":100,"after":"next"}),
            &["query OpenChangeRequests"],
            json!({"data":{"repository":{"pullRequests":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}),
        ),
        Exchange::graphql(
            "/api/graphql",
            auth,
            json!({"owner":"team","name":"app","number":7}),
            &[
                "query ChangeRequestDetails",
                "reviews(first: 100)",
                "checkSuite { workflowRun { databaseId } }",
            ],
            json!({"data":{"repository":{"pullRequest":null}}}),
        ),
    ]);
    let client = GitHubClient::new().unwrap();
    let config = [(SERVER_URL_KEY.into(), api.url.clone())]
        .into_iter()
        .collect();
    let token = ProviderToken::new("test-token".into());
    assert_eq!(
        client
            .list_change_requests(&config, &token, &repo)
            .await
            .unwrap(),
        Some(vec![])
    );
    assert_eq!(
        client
            .change_request_details(&config, &token, &repo, 7)
            .await
            .unwrap(),
        None
    );
    api.finish();
}

#[test]
fn shared_rest_builder_encodes_provider_identifiers_as_segments() {
    let client = GitHubClient::new().unwrap();
    let token = ProviderToken::new("test-token".into());
    let request = client
        .request(
            &github_com(),
            &token,
            &["repos", "team/other", "app?query", "actions", "runs"],
        )
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        request.url().path(),
        "/repos/team%2Fother/app%3Fquery/actions/runs"
    );
    assert!(request.url().query().is_none());
}

fn github_com() -> ConnectionConfiguration {
    [(SERVER_URL_KEY.to_owned(), DEFAULT_SERVER_URL.to_owned())]
        .into_iter()
        .collect()
}

#[test]
fn enterprise_server_is_normalized_and_uses_the_api_v3_prefix()
-> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::new()?;
    let configured = client.configure(
        &[(
            SERVER_URL_KEY.to_owned(),
            "https://GitHub.EXAMPLE.com/".to_owned(),
        )]
        .into_iter()
        .collect(),
    )?;
    let token = ProviderToken::new("enterprise_token".to_owned());
    let request = client
        .request(&configured.configuration, &token, &["user"])?
        .build()?;

    assert_eq!(configured.unique_key, "https://github.example.com");
    assert_eq!(configured.label, "github.example.com");
    assert_eq!(
        request.url().as_str(),
        "https://github.example.com/api/v3/user"
    );
    Ok(())
}

#[test]
fn github_configuration_rejects_paths_and_insecure_remote_servers()
-> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::new()?;
    for value in [
        "https://github.example.com/team",
        "http://github.example.com",
        "https://user:password@github.example.com",
        "https://github.example.com?tenant=one",
    ] {
        let result = client.configure(
            &[(SERVER_URL_KEY.to_owned(), value.to_owned())]
                .into_iter()
                .collect(),
        );
        assert_eq!(
            result,
            Err(ConnectionValidationFailure::InvalidConfiguration)
        );
    }
    Ok(())
}

#[test]
fn validation_request_targets_only_github_with_required_headers()
-> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::new()?;
    let token = ProviderToken::new("github_pat_test".to_owned());
    let request = client.request(&github_com(), &token, &["user"])?.build()?;

    assert_eq!(request.method(), reqwest::Method::GET);
    assert_eq!(request.url().as_str(), "https://api.github.com/user");
    assert_eq!(request.headers()[ACCEPT], ACCEPT_VALUE);
    assert_eq!(request.headers()["X-GitHub-Api-Version"], API_VERSION);
    assert_eq!(request.headers()[USER_AGENT_HEADER], USER_AGENT);
    assert_eq!(request.headers()[AUTHORIZATION], "Bearer github_pat_test");
    Ok(())
}

#[test]
fn github_response_maps_to_a_non_secret_identity() -> Result<(), Box<dyn std::error::Error>> {
    let user: GitHubUser = serde_json::from_str(
        r#"{"id":42,"login":"octocat","name":"The Octocat","html_url":"https://github.com/octocat"}"#,
    )?;

    let validated = ValidatedAccount {
        external_id: user.id.to_string(),
        name: user.name.expect("fixture name"),
        handle: Some(user.login),
        profile_url: Some(user.html_url),
    };
    assert_eq!(validated.external_id, "42");
    assert_eq!(validated.handle.as_deref(), Some("octocat"));
    Ok(())
}

#[test]
fn repository_request_is_authenticated_and_paginated() -> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::new()?;
    let token = ProviderToken::new("github_pat_test".to_owned());
    let request = client
        .request(&github_com(), &token, &["user", "repos"])?
        .query(&[("per_page", REPOSITORIES_PER_PAGE), ("page", 2)])
        .build()?;

    assert_eq!(request.method(), reqwest::Method::GET);
    assert_eq!(request.url().path(), "/user/repos");
    assert_eq!(request.url().query(), Some("per_page=100&page=2"));
    assert_eq!(request.headers()[AUTHORIZATION], "Bearer github_pat_test");
    Ok(())
}

#[test]
fn github_repository_maps_to_provider_independent_domain() -> Result<(), Box<dyn std::error::Error>>
{
    let repository: GitHubRepository = serde_json::from_str(
        r#"{"id":1296269,"owner":{"login":"octocat"},"name":"Hello-World","description":"A sample","private":false,"archived":false,"html_url":"https://github.com/octocat/Hello-World"}"#,
    )?;

    let mapped = Repository::from(repository);
    assert_eq!(mapped.id, "1296269");
    assert_eq!(mapped.owner, "octocat");
    assert_eq!(mapped.visibility, RepositoryVisibility::Public);
    Ok(())
}

#[test]
fn archived_github_repositories_are_not_available() -> Result<(), Box<dyn std::error::Error>> {
    let repositories: Vec<GitHubRepository> = serde_json::from_str(
        r#"[
            {"id":1,"owner":{"login":"octocat"},"name":"active","description":null,"private":false,"archived":false,"html_url":"https://github.com/octocat/active"},
            {"id":2,"owner":{"login":"octocat"},"name":"archived","description":null,"private":false,"archived":true,"html_url":"https://github.com/octocat/archived"}
        ]"#,
    )?;

    let mapped = map_available_repositories(repositories);

    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0].name, "active");
    Ok(())
}

#[test]
fn workflow_request_targets_selected_repository_with_required_headers()
-> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::new()?;
    let token = ProviderToken::new("github_pat_test".to_owned());
    let repository = Repository {
        id: "1296269".to_owned(),
        owner: "octocat".to_owned(),
        name: "Hello-World".to_owned(),
        description: None,
        visibility: RepositoryVisibility::Public,
        web_url: "https://github.com/octocat/Hello-World".to_owned(),
    };
    let request = client
        .request(
            &github_com(),
            &token,
            &[
                "repos",
                &repository.owner,
                &repository.name,
                "actions",
                "workflows",
            ],
        )?
        .query(&[("per_page", WORKFLOWS_PER_PAGE), ("page", 2)])
        .build()?;

    assert_eq!(
        request.url().path(),
        "/repos/octocat/Hello-World/actions/workflows"
    );
    assert_eq!(request.url().query(), Some("per_page=100&page=2"));
    assert_eq!(request.headers()[AUTHORIZATION], "Bearer github_pat_test");
    Ok(())
}

#[test]
fn github_workflow_maps_to_provider_independent_domain() -> Result<(), Box<dyn std::error::Error>> {
    let workflow: GitHubWorkflow = serde_json::from_str(
        r#"{"id":161335,"name":"CI","path":".github/workflows/ci.yml","state":"active","html_url":"https://github.com/octocat/Hello-World/actions/workflows/ci.yml"}"#,
    )?;

    let mapped = Workflow::from(workflow);
    assert_eq!(mapped.id, "161335");
    assert_eq!(mapped.name, "CI");
    assert_eq!(mapped.state, WorkflowState::Active);
    Ok(())
}

#[test]
fn github_change_request_maps_review_checks_and_merge_readiness()
-> Result<(), Box<dyn std::error::Error>> {
    let change_request: GitHubChangeRequest = serde_json::from_str(
        r#"{
          "id":"PR_kwDOExample","number":42,"title":"Harden authentication",
          "author":{"login":"octocat"},"headRefName":"auth-fix","baseRefName":"main",
          "state":"OPEN","isDraft":false,"reviewDecision":"APPROVED",
          "mergeStateStatus":"CLEAN","createdAt":"2026-09-26T18:00:00Z",
          "updatedAt":"2026-09-27T18:00:00Z","url":"https://github.com/octocat/Hello-World/pull/42",
          "commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"SUCCESS"}}}]}
        }"#,
    )?;

    let mapped = ChangeRequest::from(change_request);
    assert_eq!(mapped.number, 42);
    assert_eq!(mapped.review_status, ChangeRequestReviewStatus::Approved);
    assert_eq!(mapped.check_status, ChangeRequestCheckStatus::Passed);
    assert_eq!(mapped.merge_status, ChangeRequestMergeStatus::Ready);
    Ok(())
}

#[test]
fn github_distinguishes_absent_and_unknown_checks_and_reviews() {
    let base = json!({
        "id":"PR_example", "number":42, "title":"Change", "author":null,
        "headRefName":"feature", "baseRefName":"main", "state":"OPEN",
        "isDraft":false, "reviewDecision":null, "mergeStateStatus":"CLEAN",
        "createdAt":"2026-09-26T18:00:00Z", "updatedAt":"2026-09-27T18:00:00Z",
        "url":"https://github.com/team/app/pull/42"
    });
    for (commits, reviews, check_status, review_status) in [
        (
            json!({"nodes":[{"commit":{"statusCheckRollup":null}}]}),
            json!({"totalCount":0}),
            ChangeRequestCheckStatus::None,
            ChangeRequestReviewStatus::None,
        ),
        (
            json!({"nodes":[]}),
            serde_json::Value::Null,
            ChangeRequestCheckStatus::Unknown,
            ChangeRequestReviewStatus::Unknown,
        ),
        (
            json!({"nodes":[{"commit":{"statusCheckRollup":{"state":"FUTURE"}}}]}),
            json!({"totalCount":1}),
            ChangeRequestCheckStatus::Unknown,
            ChangeRequestReviewStatus::Unknown,
        ),
        (
            json!({"nodes":[{"commit":{"statusCheckRollup":{"state":"PENDING"}}}]}),
            json!({"totalCount":0}),
            ChangeRequestCheckStatus::Running,
            ChangeRequestReviewStatus::None,
        ),
    ] {
        let mut value = base.clone();
        value["commits"] = commits;
        value["reviews"] = reviews;
        let request: GitHubChangeRequest = serde_json::from_value(value).unwrap();
        let summary = ChangeRequest::from(request);
        assert_eq!(summary.check_status, check_status);
        assert_eq!(summary.review_status, review_status);
    }
    let mut value = base;
    value["commits"] = json!({"nodes":[]});
    value["reviews"] = json!({"totalCount":0});
    value["reviewDecision"] = json!("REVIEW_REQUIRED");
    let request: GitHubChangeRequest = serde_json::from_value(value).unwrap();
    assert_eq!(
        ChangeRequest::from(request).review_status,
        ChangeRequestReviewStatus::ReviewRequired
    );
}

#[test]
fn github_change_request_details_map_provider_independent_sections()
-> Result<(), Box<dyn std::error::Error>> {
    let change_request: GitHubChangeRequest = serde_json::from_str(
        r#"{
          "id":"PR_kwDOExample","number":42,"title":"Harden authentication",
          "author":{"login":"octocat"},"headRefName":"auth-fix","baseRefName":"main",
          "state":"OPEN","isDraft":false,"reviewDecision":"APPROVED",
          "mergeStateStatus":"CLEAN","createdAt":"2026-09-26T18:00:00Z",
          "updatedAt":"2026-09-27T18:00:00Z","url":"https://github.com/octocat/Hello-World/pull/42",
          "body":"Improves token handling.","labels":{"nodes":[{"name":"security"}]},
          "reviews":{"nodes":[{"author":{"login":"reviewer"},"state":"APPROVED","submittedAt":"2026-09-27T17:00:00Z"}]},
          "commits":{"nodes":[{"commit":{"oid":"abcdef123456","messageHeadline":"Harden tokens","committedDate":"2026-09-27T16:00:00Z","author":{"name":"Octo Cat","user":{"login":"octocat"}},"statusCheckRollup":{"state":"SUCCESS","contexts":{"nodes":[{"__typename":"CheckRun","name":"test","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"https://github.com/octocat/Hello-World/runs/789","checkSuite":{"workflowRun":{"databaseId":123456}}}]}}}}]}
        }"#,
    )?;

    let details = change_request.into_details();
    assert_eq!(details.body.as_deref(), Some("Improves token handling."));
    assert_eq!(details.labels, vec!["security"]);
    assert_eq!(
        details.reviews[0].status,
        ChangeRequestReviewStatus::Approved
    );
    assert_eq!(details.checks[0].status, ChangeRequestCheckStatus::Passed);
    assert_eq!(details.checks[0].workflow_run_id.as_deref(), Some("123456"));
    assert_eq!(
        details.latest_commit.expect("latest commit").sha,
        "abcdef123456"
    );
    Ok(())
}

#[test]
fn workflow_run_request_targets_selected_repository_with_required_headers()
-> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::new()?;
    let token = ProviderToken::new("github_pat_test".to_owned());
    let repository = Repository {
        id: "1296269".to_owned(),
        owner: "octocat".to_owned(),
        name: "Hello-World".to_owned(),
        description: None,
        visibility: RepositoryVisibility::Public,
        web_url: "https://github.com/octocat/Hello-World".to_owned(),
    };
    let request = client
        .request(
            &github_com(),
            &token,
            &[
                "repos",
                &repository.owner,
                &repository.name,
                "actions",
                "runs",
            ],
        )?
        .query(&[("per_page", WORKFLOW_RUNS_PER_PAGE), ("page", 2)])
        .build()?;

    assert_eq!(
        request.url().path(),
        "/repos/octocat/Hello-World/actions/runs"
    );
    assert_eq!(request.url().query(), Some("per_page=100&page=2"));
    assert_eq!(request.headers()[ACCEPT], ACCEPT_VALUE);
    assert_eq!(request.headers()["X-GitHub-Api-Version"], API_VERSION);
    assert_eq!(request.headers()[AUTHORIZATION], "Bearer github_pat_test");
    Ok(())
}

#[test]
fn workflow_run_request_targets_a_run_outside_cached_history()
-> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::new()?;
    let token = ProviderToken::new("github_pat_test".to_owned());
    let repository = Repository {
        id: "1296269".to_owned(),
        owner: "octocat".to_owned(),
        name: "Hello-World".to_owned(),
        description: None,
        visibility: RepositoryVisibility::Public,
        web_url: "https://github.com/octocat/Hello-World".to_owned(),
    };
    let request = client
        .request(
            &github_com(),
            &token,
            &[
                "repos",
                &repository.owner,
                &repository.name,
                "actions",
                "runs",
                "30433642",
            ],
        )?
        .build()?;

    assert_eq!(
        request.url().path(),
        "/repos/octocat/Hello-World/actions/runs/30433642"
    );
    assert_eq!(request.headers()[AUTHORIZATION], "Bearer github_pat_test");
    Ok(())
}

#[test]
fn workflow_run_log_request_targets_the_selected_attempt() -> Result<(), Box<dyn std::error::Error>>
{
    let client = GitHubClient::new()?;
    let token = ProviderToken::new("github_pat_test".to_owned());
    let repository = Repository {
        id: "1296269".to_owned(),
        owner: "octocat".to_owned(),
        name: "Hello-World".to_owned(),
        description: None,
        visibility: RepositoryVisibility::Public,
        web_url: "https://github.com/octocat/Hello-World".to_owned(),
    };
    let run = WorkflowRun {
        relationships: Default::default(),
        id: "30433642".to_owned(),
        workflow_id: "161335".to_owned(),
        run_number: 562,
        attempt: 2,
        title: "Build".to_owned(),
        lifecycle: RunLifecycle::Completed,
        outcome: RunOutcome::Success,
        branch: Some("main".to_owned()),
        commit_sha: "abc123".to_owned(),
        actor: None,
        trigger: "push".to_owned(),
        created_at: "2026-09-26T18:00:00Z".to_owned(),
        started_at: None,
        updated_at: "2026-09-26T18:03:00Z".to_owned(),
        web_url: "https://github.com/octocat/Hello-World/actions/runs/30433642".to_owned(),
        provider_status: "completed".to_owned(),
        provider_conclusion: Some("success".to_owned()),
    };
    let request = client
        .request(
            &github_com(),
            &token,
            &[
                "repos",
                &repository.owner,
                &repository.name,
                "actions",
                "runs",
                &run.id,
                "attempts",
                &run.attempt.to_string(),
                "logs",
            ],
        )?
        .build()?;

    assert_eq!(
        request.url().path(),
        "/repos/octocat/Hello-World/actions/runs/30433642/attempts/2/logs"
    );
    assert_eq!(request.headers()[AUTHORIZATION], "Bearer github_pat_test");
    Ok(())
}

#[test]
fn log_archive_is_decoded_into_named_text_files() -> Result<(), Box<dyn std::error::Error>> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    writer.start_file("build/1_setup.txt", SimpleFileOptions::default())?;
    writer.write_all(b"Preparing build\n")?;
    writer.start_file("build/2_test.txt", SimpleFileOptions::default())?;
    writer.write_all(b"All tests passed\n")?;
    let bytes = writer.finish()?.into_inner();

    let logs = read_log_archive(bytes)?;

    assert_eq!(logs.files.len(), 2);
    assert_eq!(logs.files[0].name, "build/1_setup.txt");
    assert_eq!(logs.files[1].content, "All tests passed\n");
    assert!(!logs.truncated);
    Ok(())
}

#[test]
fn github_workflow_run_maps_to_provider_independent_domain()
-> Result<(), Box<dyn std::error::Error>> {
    let run: GitHubWorkflowRun = serde_json::from_str(
        r#"{
          "id": 30433642,
          "workflow_id": 161335,
          "run_number": 562,
          "run_attempt": 2,
          "display_title": "Update README",
          "status": "completed",
          "conclusion": "failure",
          "head_branch": "main",
          "head_sha": "acb5820ced9479c074f688cc328bf03f341a511d",
          "actor": {"login": "octocat"},
          "event": "push",
          "created_at": "2026-09-26T18:00:00Z",
          "run_started_at": "2026-09-26T18:00:02Z",
          "updated_at": "2026-09-26T18:03:00Z",
          "html_url": "https://github.com/octocat/Hello-World/actions/runs/30433642"
        }"#,
    )?;

    let mapped = WorkflowRun::from(run);
    assert_eq!(mapped.id, "30433642");
    assert_eq!(mapped.workflow_id, "161335");
    assert_eq!(mapped.run_number, 562);
    assert_eq!(mapped.attempt, 2);
    assert_eq!(mapped.lifecycle, RunLifecycle::Completed);
    assert_eq!(mapped.outcome, RunOutcome::Failure);
    assert_eq!(mapped.branch.as_deref(), Some("main"));
    assert_eq!(mapped.actor.as_deref(), Some("octocat"));
    assert_eq!(mapped.trigger, "push");
    assert_eq!(mapped.provider_status, "completed");
    assert_eq!(mapped.provider_conclusion.as_deref(), Some("failure"));
    Ok(())
}

#[test]
fn unfinished_github_run_has_no_outcome() -> Result<(), Box<dyn std::error::Error>> {
    let run: GitHubWorkflowRun = serde_json::from_str(
        r#"{
          "id": 30433643,
          "workflow_id": 161335,
          "run_number": 563,
          "run_attempt": 1,
          "display_title": "Build pull request",
          "status": "in_progress",
          "conclusion": null,
          "head_branch": "feature/cache",
          "head_sha": "bcb5820ced9479c074f688cc328bf03f341a511d",
          "actor": null,
          "event": "pull_request",
          "created_at": "2026-09-26T19:00:00Z",
          "run_started_at": "2026-09-26T19:00:04Z",
          "updated_at": "2026-09-26T19:00:04Z",
          "html_url": "https://github.com/octocat/Hello-World/actions/runs/30433643"
        }"#,
    )?;

    let mapped = WorkflowRun::from(run);
    assert_eq!(mapped.lifecycle, RunLifecycle::Running);
    assert_eq!(mapped.outcome, RunOutcome::Unknown);
    assert!(mapped.actor.is_none());
    assert!(mapped.provider_conclusion.is_none());
    Ok(())
}

#[test]
fn graphql_transport_preserves_headers_body_and_enterprise_endpoint() {
    let client = GitHubClient::new().unwrap();
    let token = ProviderToken::new("test-token".into());
    let body = json!({"query":"query Viewer { viewer { login } }","variables":{}});
    for (server, url) in [
        ("https://github.com", "https://api.github.com/graphql"),
        (
            "https://github.example",
            "https://github.example/api/graphql",
        ),
    ] {
        let config = [(SERVER_URL_KEY.into(), server.into())]
            .into_iter()
            .collect();
        let request = client
            .graphql_request(&config, &token, body.clone())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(request.url().as_str(), url);
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(request.headers()[ACCEPT], ACCEPT_VALUE);
        assert_eq!(request.headers()["X-GitHub-Api-Version"], API_VERSION);
        assert_eq!(request.headers()[AUTHORIZATION], "Bearer test-token");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                request.body().unwrap().as_bytes().unwrap()
            )
            .unwrap(),
            body
        );
    }
}
