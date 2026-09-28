//! GitHub source module for connection, repository, and workflow discovery.

mod issues;
mod workflows;
#[cfg(test)]
use workflows::GitHubWorkflow;
use workflows::{GitHubWorkflowPage, GitHubWorkflowRun, GitHubWorkflowRunPage};
mod change_requests;
#[cfg(test)]
use change_requests::GitHubChangeRequest;

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
const SERVER_URL_KEY: &str = "serverUrl";
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
            .map(|client| Self { client })
            .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)
    }

    fn api_url(
        configuration: &ConnectionConfiguration,
        path: &str,
    ) -> Result<reqwest::Url, ConnectionValidationFailure> {
        let server_url = configuration
            .get(SERVER_URL_KEY)
            .ok_or(ConnectionValidationFailure::InvalidConfiguration)?;
        let base = if server_url == DEFAULT_SERVER_URL {
            reqwest::Url::parse(DEFAULT_API_URL)
        } else {
            reqwest::Url::parse(&format!("{server_url}/api/v3/"))
        }
        .map_err(|_| ConnectionValidationFailure::InvalidConfiguration)?;
        base.join(path)
            .map_err(|_| ConnectionValidationFailure::InvalidConfiguration)
    }

    fn graphql_url(
        configuration: &ConnectionConfiguration,
    ) -> Result<reqwest::Url, ConnectionValidationFailure> {
        let server_url = configuration
            .get(SERVER_URL_KEY)
            .ok_or(ConnectionValidationFailure::InvalidConfiguration)?;
        let value = if server_url == DEFAULT_SERVER_URL {
            "https://api.github.com/graphql".to_owned()
        } else {
            format!("{server_url}/api/graphql")
        };
        reqwest::Url::parse(&value).map_err(|_| ConnectionValidationFailure::InvalidConfiguration)
    }

    fn validation_request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        Ok(self
            .client
            .get(Self::api_url(configuration, "user")?)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION))
    }

    fn repositories_request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        page: usize,
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        let mut url = Self::api_url(configuration, "user/repos")?;
        url.query_pairs_mut()
            .append_pair("per_page", &REPOSITORIES_PER_PAGE.to_string())
            .append_pair("page", &page.to_string());
        Ok(self
            .client
            .get(url)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION))
    }

    async fn download_log_archive(
        &self,
        configuration: &ConnectionConfiguration,
        location: &str,
    ) -> Result<Vec<u8>, WorkflowRunLogsFailure> {
        let url = reqwest::Url::parse(location)
            .map_err(|_| WorkflowRunLogsFailure::UnexpectedResponse)?;
        let host = url
            .host_str()
            .ok_or(WorkflowRunLogsFailure::UnexpectedResponse)?;
        let configured_host = configuration
            .get(SERVER_URL_KEY)
            .and_then(|value| reqwest::Url::parse(value).ok())
            .and_then(|url| url.host_str().map(str::to_owned));
        let is_configured_host =
            configured_host.is_some_and(|configured| configured.eq_ignore_ascii_case(host));
        if url.scheme() != "https"
            || (!is_configured_host
                && (host.eq_ignore_ascii_case("localhost")
                    || host.ends_with(".localhost")
                    || host.ends_with(".local")
                    || host.parse::<IpAddr>().is_ok()))
        {
            return Err(WorkflowRunLogsFailure::UnexpectedResponse);
        }

        // The temporary archive URL is deliberately requested without the provider token.
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| WorkflowRunLogsFailure::ProviderUnavailable)?;
        if response.status() != StatusCode::OK {
            return Err(WorkflowRunLogsFailure::LogsUnavailable);
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_LOG_ARCHIVE_BYTES as u64)
        {
            return Err(WorkflowRunLogsFailure::LogsTooLarge);
        }

        let mut archive = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| WorkflowRunLogsFailure::ProviderUnavailable)?
        {
            if archive.len().saturating_add(chunk.len()) > MAX_LOG_ARCHIVE_BYTES {
                return Err(WorkflowRunLogsFailure::LogsTooLarge);
            }
            archive.extend_from_slice(&chunk);
        }
        Ok(archive)
    }
}

fn read_log_archive(bytes: Vec<u8>) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| WorkflowRunLogsFailure::UnexpectedResponse)?;
    let mut files = Vec::new();
    let mut text_bytes = 0usize;
    let mut truncated = false;

    for index in 0..archive.len() {
        if files.len() == MAX_LOG_FILES || text_bytes == MAX_LOG_TEXT_BYTES {
            truncated = true;
            break;
        }

        let mut file = archive
            .by_index(index)
            .map_err(|_| WorkflowRunLogsFailure::UnexpectedResponse)?;
        if file.is_dir() {
            continue;
        }
        let Some(name) = file
            .enclosed_name()
            .map(|path| path.to_string_lossy().into_owned())
        else {
            truncated = true;
            continue;
        };
        let remaining = MAX_LOG_TEXT_BYTES - text_bytes;
        let limit = remaining.min(MAX_LOG_FILE_BYTES);
        let mut content = Vec::new();
        file.by_ref()
            .take((limit + 1) as u64)
            .read_to_end(&mut content)
            .map_err(|_| WorkflowRunLogsFailure::UnexpectedResponse)?;
        if content.len() > limit {
            content.truncate(limit);
            truncated = true;
        }
        text_bytes += content.len();
        files.push(WorkflowRunLog {
            name,
            content: String::from_utf8_lossy(&content).into_owned(),
        });
    }

    Ok(WorkflowRunLogs { files, truncated })
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
    archived: bool,
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

fn map_available_repositories(repositories: Vec<GitHubRepository>) -> Vec<Repository> {
    repositories
        .into_iter()
        .filter(|repository| !repository.archived)
        .map(Repository::from)
        .collect()
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
                help: "Use a fine-grained token with Metadata, Actions, Pull requests, Issues, Checks, and Commit statuses read access, or a classic token with repo access."
                    .to_owned(),
            },
            connection_fields: vec![ConnectionField {
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
            .validation_request(configuration, token)?
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
        let mut repositories = Vec::new();
        for page in 1..=MAX_REPOSITORY_PAGES {
            let response = self
                .repositories_request(configuration, token, page)?
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
            repositories.extend(map_available_repositories(page_repositories));
            if page_size < REPOSITORIES_PER_PAGE {
                return Ok(repositories);
            }
        }
        Err(ConnectionValidationFailure::UnexpectedResponse)
    }

    async fn list_workflows(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, ConnectionValidationFailure> {
        let mut workflows = Vec::new();
        for page in 1..=MAX_WORKFLOW_PAGES {
            let response = self
                .workflows_request(configuration, token, repository, page)?
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
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, ConnectionValidationFailure> {
        let mut runs = Vec::new();
        for page in 1..=MAX_WORKFLOW_RUN_PAGES {
            let response = self
                .workflow_runs_request(configuration, token, repository, page)?
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

    async fn list_change_requests(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Option<Vec<ChangeRequest>>, ConnectionValidationFailure> {
        self.fetch_change_requests(configuration, token, repository)
            .await
    }
    async fn change_request_details(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        number: u64,
    ) -> Result<Option<ChangeRequestDetails>, ConnectionValidationFailure> {
        self.fetch_change_request_details(configuration, token, repository, number)
            .await
    }
    async fn list_issues(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Option<Vec<Issue>>, ConnectionValidationFailure> {
        issues::list(self, configuration, token, repository).await
    }

    async fn issue_details(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        number: u64,
    ) -> Result<Option<IssueDetails>, ConnectionValidationFailure> {
        issues::details(self, configuration, token, repository, number).await
    }

    async fn workflow_run(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run_id: &str,
    ) -> Result<Option<WorkflowRun>, ConnectionValidationFailure> {
        let response = self
            .workflow_run_request(configuration, token, repository, run_id)?
            .send()
            .await
            .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if response.status() != StatusCode::OK {
            return Err(response_failure(&response));
        }
        response
            .json::<GitHubWorkflowRun>()
            .await
            .map(WorkflowRun::from)
            .map(Some)
            .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)
    }

    async fn workflow_run_logs(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        let response = self
            .workflow_run_logs_request(configuration, token, repository, run)
            .map_err(|_| WorkflowRunLogsFailure::UnexpectedResponse)?
            .send()
            .await
            .map_err(|_| WorkflowRunLogsFailure::ProviderUnavailable)?;
        if response.status() != StatusCode::FOUND {
            return Err(log_response_failure(&response));
        }
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(WorkflowRunLogsFailure::UnexpectedResponse)?;
        let bytes = self.download_log_archive(configuration, location).await?;
        read_log_archive(bytes)
    }
}

#[cfg(test)]
mod tests;
