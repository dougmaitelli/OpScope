//! GitHub source module for connection, repository, and workflow discovery.

use crate::application::{
    ConnectionValidationFailure, CredentialField, ProviderToken, SourceDescriptor, SourceModule,
    ValidatedAccount,
};
use crate::domain::{Repository, RepositoryVisibility, Workflow, WorkflowState};
use async_trait::async_trait;
use reqwest::header::{ACCEPT, HeaderValue, USER_AGENT as USER_AGENT_HEADER};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration;

const USER_API_URL: &str = "https://api.github.com/user";
const REPOSITORIES_API_URL: &str = "https://api.github.com/user/repos";
const API_VERSION: &str = "2026-03-10";
const ACCEPT_VALUE: &str = "application/vnd.github+json";
const USER_AGENT: &str = "CI-Watcher/0.1";
const REPOSITORIES_PER_PAGE: usize = 100;
const MAX_REPOSITORY_PAGES: usize = 100;
const WORKFLOWS_PER_PAGE: usize = 100;
const MAX_WORKFLOW_PAGES: usize = 100;

#[derive(Clone, Debug)]
pub struct GitHubClient {
    client: Client,
}

impl GitHubClient {
    pub fn new() -> Result<Self, ConnectionValidationFailure> {
        Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(USER_AGENT)
            .build()
            .map(|client| Self { client })
            .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)
    }

    fn validation_request(&self, token: &ProviderToken) -> reqwest::RequestBuilder {
        self.client
            .get(USER_API_URL)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION)
    }

    fn repositories_request(&self, token: &ProviderToken, page: usize) -> reqwest::RequestBuilder {
        let url = format!("{REPOSITORIES_API_URL}?per_page={REPOSITORIES_PER_PAGE}&page={page}");
        self.client
            .get(url)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION)
    }

    fn workflows_request(
        &self,
        token: &ProviderToken,
        repository: &Repository,
        page: usize,
    ) -> reqwest::RequestBuilder {
        let url = format!(
            "https://api.github.com/repos/{}/{}/actions/workflows?per_page={WORKFLOWS_PER_PAGE}&page={page}",
            repository.owner, repository.name
        );
        self.client
            .get(url)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION)
    }
}

#[derive(Deserialize)]
struct GitHubUser {
    id: u64,
    login: String,
    name: Option<String>,
    html_url: String,
}

#[derive(Deserialize)]
struct GitHubRepositoryOwner {
    login: String,
}

#[derive(Deserialize)]
struct GitHubRepository {
    id: u64,
    owner: GitHubRepositoryOwner,
    name: String,
    description: Option<String>,
    private: bool,
    html_url: String,
}

impl From<GitHubRepository> for Repository {
    fn from(repository: GitHubRepository) -> Self {
        Self {
            id: repository.id.to_string(),
            owner: repository.owner.login,
            name: repository.name,
            description: repository.description,
            visibility: if repository.private {
                RepositoryVisibility::Private
            } else {
                RepositoryVisibility::Public
            },
            web_url: repository.html_url,
        }
    }
}

#[derive(Deserialize)]
struct GitHubWorkflowPage {
    workflows: Vec<GitHubWorkflow>,
}

#[derive(Deserialize)]
struct GitHubWorkflow {
    id: u64,
    name: String,
    path: String,
    state: String,
    html_url: String,
}

impl From<GitHubWorkflow> for Workflow {
    fn from(workflow: GitHubWorkflow) -> Self {
        Self {
            id: workflow.id.to_string(),
            name: workflow.name,
            path: workflow.path,
            state: if workflow.state == "active" {
                WorkflowState::Active
            } else {
                WorkflowState::Disabled
            },
            web_url: workflow.html_url,
        }
    }
}

fn response_failure(response: &reqwest::Response) -> ConnectionValidationFailure {
    match response.status() {
        StatusCode::UNAUTHORIZED => ConnectionValidationFailure::InvalidCredentials,
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
            if response
                .headers()
                .get("x-ratelimit-remaining")
                .is_some_and(|value| value == HeaderValue::from_static("0"))
                || response.headers().contains_key("retry-after")
                || response.status() == StatusCode::TOO_MANY_REQUESTS =>
        {
            ConnectionValidationFailure::RateLimited
        }
        StatusCode::FORBIDDEN => ConnectionValidationFailure::InvalidCredentials,
        status if status.is_server_error() => ConnectionValidationFailure::ProviderUnavailable,
        _ => ConnectionValidationFailure::UnexpectedResponse,
    }
}

#[async_trait]
impl SourceModule for GitHubClient {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "github".to_owned(),
            name: "GitHub".to_owned(),
            description: "Repositories, Actions workflows, and workflow runs".to_owned(),
            abbreviation: "GH".to_owned(),
            credential: CredentialField {
                label: "Fine-grained personal access token".to_owned(),
                placeholder: "github_pat_…".to_owned(),
                help: "Use a repository-scoped token with Metadata and Actions read access."
                    .to_owned(),
            },
        }
    }

    async fn validate(
        &self,
        token: &ProviderToken,
    ) -> Result<ValidatedAccount, ConnectionValidationFailure> {
        let response = self
            .validation_request(token)
            .send()
            .await
            .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)?;

        match response.status() {
            StatusCode::OK => {
                let user = response
                    .json::<GitHubUser>()
                    .await
                    .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
                let name = user.name.unwrap_or_else(|| user.login.clone());
                Ok(ValidatedAccount {
                    external_id: user.id.to_string(),
                    name,
                    handle: Some(user.login),
                    profile_url: Some(user.html_url),
                })
            }
            _ => Err(response_failure(&response)),
        }
    }

    async fn list_repositories(
        &self,
        token: &ProviderToken,
    ) -> Result<Vec<Repository>, ConnectionValidationFailure> {
        let mut repositories = Vec::new();
        for page in 1..=MAX_REPOSITORY_PAGES {
            let response = self
                .repositories_request(token, page)
                .send()
                .await
                .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)?;
            if response.status() != StatusCode::OK {
                return Err(response_failure(&response));
            }

            let page_repositories = response
                .json::<Vec<GitHubRepository>>()
                .await
                .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
            let page_size = page_repositories.len();
            repositories.extend(page_repositories.into_iter().map(Repository::from));
            if page_size < REPOSITORIES_PER_PAGE {
                return Ok(repositories);
            }
        }
        Err(ConnectionValidationFailure::UnexpectedResponse)
    }

    async fn list_workflows(
        &self,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, ConnectionValidationFailure> {
        let mut workflows = Vec::new();
        for page in 1..=MAX_WORKFLOW_PAGES {
            let response = self
                .workflows_request(token, repository, page)
                .send()
                .await
                .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)?;
            if matches!(
                response.status(),
                StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
            ) {
                return Err(ConnectionValidationFailure::PermissionDenied);
            }
            if response.status() != StatusCode::OK {
                return Err(response_failure(&response));
            }

            let page = response
                .json::<GitHubWorkflowPage>()
                .await
                .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
            let page_size = page.workflows.len();
            workflows.extend(page.workflows.into_iter().map(Workflow::from));
            if page_size < WORKFLOWS_PER_PAGE {
                return Ok(workflows);
            }
        }
        Err(ConnectionValidationFailure::UnexpectedResponse)
    }
}

#[cfg(test)]
mod tests {
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
    fn repository_request_is_authenticated_and_paginated() -> Result<(), Box<dyn std::error::Error>>
    {
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
    fn github_repository_maps_to_provider_independent_domain()
    -> Result<(), Box<dyn std::error::Error>> {
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
    fn github_workflow_maps_to_provider_independent_domain()
    -> Result<(), Box<dyn std::error::Error>> {
        let workflow: GitHubWorkflow = serde_json::from_str(
            r#"{"id":161335,"name":"CI","path":".github/workflows/ci.yml","state":"active","html_url":"https://github.com/octocat/Hello-World/actions/workflows/ci.yml"}"#,
        )?;

        let mapped = Workflow::from(workflow);
        assert_eq!(mapped.id, "161335");
        assert_eq!(mapped.name, "CI");
        assert_eq!(mapped.state, WorkflowState::Active);
        Ok(())
    }
}
