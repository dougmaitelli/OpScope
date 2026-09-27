use super::*;
use reqwest::header::AUTHORIZATION;
use std::io::Write;
use zip::write::SimpleFileOptions;

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
        .validation_request(&configured.configuration, &token)?
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
    let request = client.validation_request(&github_com(), &token)?.build()?;

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
        .repositories_request(&github_com(), &token, 2)?
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
        .workflows_request(&github_com(), &token, &repository, 2)?
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
fn change_request_request_uses_graphql_without_provider_data_in_the_contract()
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
        .change_requests_request(&github_com(), &token, &repository, Some("cursor"))?
        .build()?;
    let body: serde_json::Value = serde_json::from_slice(
        request
            .body()
            .and_then(reqwest::Body::as_bytes)
            .expect("JSON body"),
    )?;

    assert_eq!(request.url().as_str(), "https://api.github.com/graphql");
    assert_eq!(request.method(), reqwest::Method::POST);
    assert_eq!(body["variables"]["owner"], "octocat");
    assert_eq!(body["variables"]["name"], "Hello-World");
    assert_eq!(body["variables"]["after"], "cursor");
    assert_eq!(request.headers()[AUTHORIZATION], "Bearer github_pat_test");
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
fn change_request_details_request_targets_the_selected_pull_request()
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
        .change_request_details_request(&github_com(), &token, &repository, 42)?
        .build()?;
    let body: serde_json::Value = serde_json::from_slice(
        request
            .body()
            .and_then(reqwest::Body::as_bytes)
            .expect("JSON body"),
    )?;

    assert_eq!(request.url().as_str(), "https://api.github.com/graphql");
    assert_eq!(body["variables"]["number"], 42);
    assert!(
        body["query"]
            .as_str()
            .is_some_and(|query| query.contains("reviews(first: 100)"))
    );
    Ok(())
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
          "commits":{"nodes":[{"commit":{"oid":"abcdef123456","messageHeadline":"Harden tokens","committedDate":"2026-09-27T16:00:00Z","author":{"name":"Octo Cat","user":{"login":"octocat"}},"statusCheckRollup":{"state":"SUCCESS","contexts":{"nodes":[{"__typename":"CheckRun","name":"test","status":"COMPLETED","conclusion":"SUCCESS","detailsUrl":"https://github.com/octocat/Hello-World/actions/runs/123456/job/789"}]}}}}]}
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
        .workflow_runs_request(&github_com(), &token, &repository, 2)?
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
        .workflow_run_logs_request(&github_com(), &token, &repository, &run)?
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
