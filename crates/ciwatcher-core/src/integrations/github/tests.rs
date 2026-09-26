use super::*;
use reqwest::header::AUTHORIZATION;

#[test]
fn validation_request_targets_only_github_with_required_headers()
-> Result<(), Box<dyn std::error::Error>> {
    let client = GitHubClient::new()?;
    let token = ProviderToken::new("github_pat_test".to_owned());
    let request = client.validation_request(&token).build()?;

    assert_eq!(request.method(), reqwest::Method::GET);
    assert_eq!(request.url().as_str(), USER_API_URL);
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
    let request = client.repositories_request(&token, 2).build()?;

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
        r#"{"id":1296269,"owner":{"login":"octocat"},"name":"Hello-World","description":"A sample","private":false,"html_url":"https://github.com/octocat/Hello-World"}"#,
    )?;

    let mapped = Repository::from(repository);
    assert_eq!(mapped.id, "1296269");
    assert_eq!(mapped.owner, "octocat");
    assert_eq!(mapped.visibility, RepositoryVisibility::Public);
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
    let request = client.workflows_request(&token, &repository, 2).build()?;

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
        .workflow_runs_request(&token, &repository, 2)
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
