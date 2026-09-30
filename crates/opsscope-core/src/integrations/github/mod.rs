//! GitHub source module for connection, repository, and workflow discovery.

mod issues;
mod relationships;
mod workflows;
use workflows::GitHubWorkflowRun;
#[cfg(test)]
use workflows::{GitHubWorkflow, read_log_archive};
mod repositories;
#[cfg(test)]
use repositories::{GitHubRepository, map_available_repositories};
mod actions;
mod change_requests;
#[cfg(test)]
use change_requests::GitHubChangeRequest;

use super::SERVER_URL_KEY;
use crate::application::{
    ConfiguredSource, ConnectionConfiguration, ConnectionField, ConnectionValidationFailure,
    CredentialField, ProviderToken, SourceCapability, SourceDescriptor, SourceModule,
    ValidatedAccount, WorkflowRunLogsFailure,
};
use crate::domain::{
    ChangeRequest, ChangeRequestCheck, ChangeRequestCheckStatus, ChangeRequestCommit,
    ChangeRequestDetails, ChangeRequestMergeStatus, ChangeRequestReview, ChangeRequestReviewStatus,
    ChangeRequestState, Issue, IssueDetails, Repository, RepositoryVisibility, RunLifecycle,
    RunOutcome, Workflow, WorkflowRun, WorkflowRunLog, WorkflowRunLogs, WorkflowState,
};
use async_trait::async_trait;
use reqwest::header::{ACCEPT, HeaderValue, LOCATION, USER_AGENT as USER_AGENT_HEADER};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::io::{Cursor, Read};
use std::net::IpAddr;
use std::time::Duration;

const DEFAULT_SERVER_URL: &str = "https://github.com";
const DEFAULT_API_URL: &str = "https://api.github.com/";
const API_VERSION: &str = "2026-03-10";
const ACCEPT_VALUE: &str = "application/vnd.github+json";
const USER_AGENT: &str = "OpsScope/0.1";
const REPOSITORIES_PER_PAGE: usize = 100;
const MAX_REPOSITORY_PAGES: usize = 100;
const WORKFLOWS_PER_PAGE: usize = 100;
const MAX_WORKFLOW_PAGES: usize = 100;
const WORKFLOW_RUNS_PER_PAGE: usize = 100;
const MAX_WORKFLOW_RUN_PAGES: usize = 2;
const CHANGE_REQUESTS_PER_PAGE: usize = 100;
const MAX_CHANGE_REQUEST_PAGES: usize = 10;
const MAX_LOG_ARCHIVE_BYTES: usize = 25 * 1024 * 1024;
const MAX_LOG_FILES: usize = 100;
const MAX_LOG_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_LOG_TEXT_BYTES: usize = 10 * 1024 * 1024;

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
            .map(|client| Self {
                client,
            })
            .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)
    }

    fn request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        path: &[&str],
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        let server_url = configuration
            .get(SERVER_URL_KEY)
            .ok_or(ConnectionValidationFailure::InvalidConfiguration)?;
        let base = if server_url == DEFAULT_SERVER_URL {
            reqwest::Url::parse(DEFAULT_API_URL)
        } else {
            reqwest::Url::parse(&format!("{server_url}/api/v3/"))
        }
        .map_err(|_| ConnectionValidationFailure::InvalidConfiguration)?;
        let url = super::http::endpoint(base.as_str(), path)?;
        Ok(self.authenticate(self.client.get(url), token))
    }

    fn graphql_request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        body: serde_json::Value,
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        let server_url = configuration
            .get(SERVER_URL_KEY)
            .ok_or(ConnectionValidationFailure::InvalidConfiguration)?;
        let value = if server_url == DEFAULT_SERVER_URL {
            "https://api.github.com/graphql".to_owned()
        } else {
            format!("{server_url}/api/graphql")
        };
        let url = reqwest::Url::parse(&value)
            .map_err(|_| ConnectionValidationFailure::InvalidConfiguration)?;
        Ok(self.authenticate(self.client.post(url), token).json(&body))
    }

    fn authenticate(
        &self,
        request: reqwest::RequestBuilder,
        token: &ProviderToken,
    ) -> reqwest::RequestBuilder {
        request
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
struct GitHubGraphQlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GitHubGraphQlError>,
}

#[derive(Deserialize)]
struct GitHubGraphQlError {
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
struct GitHubChangeRequestAuthor {
    login: String,
}

#[derive(Deserialize)]
struct GitHubLabel {
    name: String,
}

#[derive(Deserialize)]
struct GitHubLabelConnection {
    nodes: Vec<Option<GitHubLabel>>,
}

fn report_graphql_errors(errors: &[GitHubGraphQlError]) {
    for error in errors {
        let message = error.message.replace(['\r', '\n'], " ");
        let message = message.chars().take(500).collect::<String>();
        eprintln!("GitHub GraphQL query returned an error: {message}");
    }
}

fn graphql_failure(errors: &[GitHubGraphQlError]) -> ConnectionValidationFailure {
    if errors.iter().any(|error| {
        let message = error.message.to_ascii_lowercase();
        message.contains("permission")
            || message.contains("forbidden")
            || message.contains("not accessible")
    }) {
        ConnectionValidationFailure::PermissionDenied
    } else {
        ConnectionValidationFailure::UnexpectedResponse
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

fn log_response_failure(response: &reqwest::Response) -> WorkflowRunLogsFailure {
    match response.status() {
        StatusCode::UNAUTHORIZED => WorkflowRunLogsFailure::InvalidCredentials,
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
            if response
                .headers()
                .get("x-ratelimit-remaining")
                .is_some_and(|value| value == HeaderValue::from_static("0"))
                || response.headers().contains_key("retry-after")
                || response.status() == StatusCode::TOO_MANY_REQUESTS =>
        {
            WorkflowRunLogsFailure::RateLimited
        }
        StatusCode::FORBIDDEN => WorkflowRunLogsFailure::PermissionDenied,
        StatusCode::NOT_FOUND | StatusCode::CONFLICT => WorkflowRunLogsFailure::LogsUnavailable,
        status if status.is_server_error() => WorkflowRunLogsFailure::ProviderUnavailable,
        _ => WorkflowRunLogsFailure::UnexpectedResponse,
    }
}

#[async_trait]
impl SourceModule for GitHubClient {
    async fn action_options(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        target: &crate::application::ActionTarget,
    ) -> Result<crate::application::ActionOptions, crate::application::ActionFailure> {
        self.actions(config, token, repo, target).await
    }

    async fn execute_action(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        target: &crate::application::ActionTarget,
        action: crate::application::SourceAction,
        revision: Option<&str>,
    ) -> Result<(), crate::application::ActionFailure> {
        self.perform_action(config, token, repo, target, action, revision)
            .await
    }

    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "github".to_owned(),
            name: "GitHub".to_owned(),
            description: "Repositories, workflows, runs, pull requests, and issues".to_owned(),
            abbreviation: "GH".to_owned(),
            capabilities: vec![
                SourceCapability::Workflows,
                SourceCapability::ChangeRequests,
                SourceCapability::Issues,
            ],
            credential: CredentialField {
                label: "Personal access token".to_owned(),
                placeholder: "github_pat_… or ghp_…".to_owned(),
                help: "Use a fine-grained token with Metadata, Actions, Pull requests, Issues, \
                    Checks, and Commit statuses read access, or a classic token with repo access."
                    .to_owned(),
            },
            connection_fields: vec![ConnectionField {
                input_type: crate::application::ConnectionFieldType::Url,
                key: SERVER_URL_KEY.to_owned(),
                label: "Server URL".to_owned(),
                placeholder: DEFAULT_SERVER_URL.to_owned(),
                help: "Use GitHub.com or the origin of a GitHub Enterprise Server instance."
                    .to_owned(),
                default_value: DEFAULT_SERVER_URL.to_owned(),
            }],
        }
    }

    fn configure(
        &self,
        configuration: &ConnectionConfiguration,
    ) -> Result<ConfiguredSource, ConnectionValidationFailure> {
        if configuration.keys().any(|key| key != SERVER_URL_KEY) {
            return Err(ConnectionValidationFailure::InvalidConfiguration);
        }
        let value = configuration
            .get(SERVER_URL_KEY)
            .map_or(DEFAULT_SERVER_URL, String::as_str)
            .trim();
        let value = if value.is_empty() {
            DEFAULT_SERVER_URL
        } else {
            value
        };
        let mut url = reqwest::Url::parse(value)
            .map_err(|_| ConnectionValidationFailure::InvalidConfiguration)?;
        let host = url
            .host_str()
            .ok_or(ConnectionValidationFailure::InvalidConfiguration)?
            .to_owned();
        let loopback = host.eq_ignore_ascii_case("localhost")
            || host.ends_with(".localhost")
            || host
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback());
        if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(ConnectionValidationFailure::InvalidConfiguration);
        }
        url.set_path("");
        let server_url = url.as_str().trim_end_matches('/').to_owned();
        let label = if server_url == DEFAULT_SERVER_URL {
            "GitHub.com".to_owned()
        } else {
            match url.port() {
                Some(port) => format!("{host}:{port}"),
                None => host,
            }
        };
        Ok(ConfiguredSource {
            unique_key: server_url.clone(),
            label,
            configuration: [(SERVER_URL_KEY.to_owned(), server_url)]
                .into_iter()
                .collect(),
        })
    }

    async fn validate(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<ValidatedAccount, ConnectionValidationFailure> {
        let response = self
            .request(configuration, token, &["user"])?
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
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<Vec<Repository>, ConnectionValidationFailure> {
        self.repositories(configuration, token).await
    }

    async fn list_workflows(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, ConnectionValidationFailure> {
        self.workflows(configuration, token, repository).await
    }

    async fn list_workflow_runs(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, ConnectionValidationFailure> {
        self.runs(configuration, token, repository).await
    }

    async fn list_change_requests(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Option<Vec<ChangeRequest>>, ConnectionValidationFailure> {
        self.pull_requests(configuration, token, repository).await
    }
    async fn change_request_details(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        number: u64,
    ) -> Result<Option<ChangeRequestDetails>, ConnectionValidationFailure> {
        self.pull_details(configuration, token, repository, number)
            .await
    }
    async fn list_issues(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Option<Vec<Issue>>, ConnectionValidationFailure> {
        self.issues(configuration, token, repository).await
    }

    async fn issue_details(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        number: u64,
    ) -> Result<Option<IssueDetails>, ConnectionValidationFailure> {
        self.issue(configuration, token, repository, number).await
    }

    async fn workflow_run(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run_id: &str,
    ) -> Result<Option<WorkflowRun>, ConnectionValidationFailure> {
        self.run(configuration, token, repository, run_id).await
    }

    async fn workflow_run_logs(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        self.logs(configuration, token, repository, run).await
    }
}

#[cfg(test)]
mod tests;
