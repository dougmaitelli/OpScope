//! GitHub source module for connection, repository, and workflow discovery.

use crate::application::{
    ConnectionValidationFailure, CredentialField, ProviderToken, SourceDescriptor, SourceModule,
    ValidatedAccount,
};
use crate::domain::{
    Repository, RepositoryVisibility, RunLifecycle, RunOutcome, Workflow, WorkflowRun,
    WorkflowState,
};
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
const WORKFLOW_RUNS_PER_PAGE: usize = 100;
const MAX_WORKFLOW_RUN_PAGES: usize = 2;

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

    fn workflow_runs_request(
        &self,
        token: &ProviderToken,
        repository: &Repository,
        page: usize,
    ) -> reqwest::RequestBuilder {
        let url = format!(
            "https://api.github.com/repos/{}/{}/actions/runs?per_page={WORKFLOW_RUNS_PER_PAGE}&page={page}",
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

#[derive(Deserialize)]
struct GitHubWorkflowRunPage {
    workflow_runs: Vec<GitHubWorkflowRun>,
}

#[derive(Deserialize)]
struct GitHubWorkflowRunActor {
    login: String,
}

#[derive(Deserialize)]
struct GitHubWorkflowRun {
    id: u64,
    workflow_id: u64,
    run_number: u64,
    run_attempt: u64,
    display_title: String,
    status: String,
    conclusion: Option<String>,
    head_branch: Option<String>,
    head_sha: String,
    actor: Option<GitHubWorkflowRunActor>,
    event: String,
    created_at: String,
    run_started_at: Option<String>,
    updated_at: String,
    html_url: String,
}

impl From<GitHubWorkflowRun> for WorkflowRun {
    fn from(run: GitHubWorkflowRun) -> Self {
        let lifecycle = match run.status.as_str() {
            "queued" | "requested" | "waiting" | "pending" => RunLifecycle::Queued,
            "in_progress" => RunLifecycle::Running,
            "completed" => RunLifecycle::Completed,
            _ => RunLifecycle::Unknown,
        };
        let outcome = match run.conclusion.as_deref() {
            Some("success") => RunOutcome::Success,
            Some("neutral") => RunOutcome::Warning,
            Some("failure" | "timed_out" | "startup_failure" | "action_required") => {
                RunOutcome::Failure
            }
            Some("cancelled") => RunOutcome::Cancelled,
            Some("skipped") => RunOutcome::Skipped,
            _ => RunOutcome::Unknown,
        };

        Self {
            id: run.id.to_string(),
            workflow_id: run.workflow_id.to_string(),
            run_number: run.run_number,
            attempt: run.run_attempt,
            title: run.display_title,
            lifecycle,
            outcome,
            branch: run.head_branch,
            commit_sha: run.head_sha,
            actor: run.actor.map(|actor| actor.login),
            trigger: run.event,
            created_at: run.created_at,
            started_at: run.run_started_at,
            updated_at: run.updated_at,
            web_url: run.html_url,
            provider_status: run.status,
            provider_conclusion: run.conclusion,
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

    async fn list_workflow_runs(
        &self,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, ConnectionValidationFailure> {
        let mut runs = Vec::new();
        for page in 1..=MAX_WORKFLOW_RUN_PAGES {
            let response = self
                .workflow_runs_request(token, repository, page)
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
                .json::<GitHubWorkflowRunPage>()
                .await
                .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
            let page_size = page.workflow_runs.len();
            runs.extend(page.workflow_runs.into_iter().map(WorkflowRun::from));
            if page_size < WORKFLOW_RUNS_PER_PAGE {
                return Ok(runs);
            }
        }
        Ok(runs)
    }
}

#[cfg(test)]
mod tests;
