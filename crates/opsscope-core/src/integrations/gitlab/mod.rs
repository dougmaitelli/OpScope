//! GitLab.com and self-managed GitLab source adapter.

mod change_requests;
mod issues;
mod relationships;
mod workflows;

use super::{SERVER_URL_KEY, http};
use crate::application::{
    ConfiguredSource, ConnectionConfiguration, ConnectionField,
    ConnectionValidationFailure as Failure, CredentialField, ProviderToken, SourceCapability,
    SourceDescriptor, SourceModule, ValidatedAccount, WorkflowRunLogsFailure,
};
use crate::domain::{
    ChangeRequest, ChangeRequestDetails, Issue, IssueDetails, Repository, RepositoryVisibility,
    Workflow, WorkflowRun, WorkflowRunLogs,
};
use async_trait::async_trait;
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, de::DeserializeOwned};

const DEFAULT_SERVER_URL: &str = "https://gitlab.com";

pub struct GitLabClient {
    client: Client,
}

impl GitLabClient {
    pub fn new() -> Result<Self, Failure> {
        Ok(Self {
            client: http::client()?,
        })
    }

    fn request(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        path: &[&str],
    ) -> Result<RequestBuilder, Failure> {
        let configured = http::configure_server(config, DEFAULT_SERVER_URL)?;
        let base = format!("{}/api/v4/", configured.unique_key);
        Ok(self
            .client
            .get(http::endpoint(&base, path)?)
            .header("PRIVATE-TOKEN", token.expose())
            .header("Accept", "application/json"))
    }

    async fn pages<T: DeserializeOwned>(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        path: &[&str],
        query: &[(&str, &str)],
        max_pages: usize,
        recent_only: bool,
    ) -> Result<Vec<T>, Failure> {
        let mut items = Vec::new();
        for page in 1..=max_pages {
            let response = http::send(
                self.request(config, token, path)?
                    .query(query)
                    .query(&[("per_page", http::PAGE_SIZE), ("page", page)]),
            )
            .await?;
            if !response.status().is_success() {
                return Err(http::status_failure(response.status()));
            }
            let next = response
                .headers()
                .get("x-next-page")
                .and_then(|v| v.to_str().ok())
                .map(|v| !v.is_empty());
            let batch: Vec<T> = http::decode(response).await?;
            let done = !next.unwrap_or(batch.len() == http::PAGE_SIZE);
            items.extend(batch);
            if done {
                return Ok(items);
            }
        }
        if recent_only {
            Ok(items)
        } else {
            Err(Failure::UnexpectedResponse)
        }
    }
}

#[derive(Deserialize)]
struct User {
    id: u64,
    username: String,
    name: String,
    web_url: String,
}

#[derive(Deserialize)]
struct Project {
    id: u64,
    path: String,
    namespace: Namespace,
    description: Option<String>,
    visibility: String,
    archived: bool,
    web_url: String,
}
#[derive(Deserialize)]
struct Namespace {
    full_path: String,
}

impl From<Project> for Repository {
    fn from(value: Project) -> Self {
        Self {
            id: value.id.to_string(),
            owner: value.namespace.full_path,
            name: value.path,
            description: value.description,
            visibility: if value.visibility == "public" {
                RepositoryVisibility::Public
            } else {
                RepositoryVisibility::Private
            },
            web_url: value.web_url,
        }
    }
}

#[async_trait]
impl SourceModule for GitLabClient {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "gitlab".into(),
            name: "GitLab".into(),
            abbreviation: "GL".into(),
            description: "Repositories, CI/CD pipelines, logs, merge requests, and issues".into(),
            capabilities: vec![
                SourceCapability::Workflows,
                SourceCapability::ChangeRequests,
                SourceCapability::Issues,
            ],
            credential: CredentialField {
                label: "Personal access token".into(),
                placeholder: "glpat-…".into(),
                help: "Use a personal access token with read_api access to your projects.".into(),
            },
            connection_fields: vec![ConnectionField {
                input_type: crate::application::ConnectionFieldType::Url,
                key: SERVER_URL_KEY.into(),
                label: "Server URL".into(),
                placeholder: DEFAULT_SERVER_URL.into(),
                default_value: DEFAULT_SERVER_URL.into(),
                help: "GitLab.com or your self-managed GitLab origin (HTTPS).".into(),
            }],
        }
    }

    fn configure(
        &self,
        configuration: &ConnectionConfiguration,
    ) -> Result<ConfiguredSource, Failure> {
        http::configure_server(configuration, DEFAULT_SERVER_URL)
    }

    async fn validate(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<ValidatedAccount, Failure> {
        let user: User = http::json(self.request(config, token, &["user"])?).await?;
        Ok(ValidatedAccount {
            external_id: user.id.to_string(),
            name: user.name,
            handle: Some(user.username),
            profile_url: Some(user.web_url),
        })
    }

    async fn list_repositories(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<Vec<Repository>, Failure> {
        let projects: Vec<Project> = self
            .pages(
                config,
                token,
                &["projects"],
                &[
                    ("membership", "true"),
                    ("archived", "false"),
                    ("order_by", "id"),
                    ("sort", "asc"),
                ],
                http::MAX_PAGES,
                false,
            )
            .await?;
        Ok(projects
            .into_iter()
            .filter(|p| !p.archived)
            .map(Repository::from)
            .collect())
    }

    async fn list_issues(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Option<Vec<Issue>>, Failure> {
        self.issues(config, token, repository).await
    }

    async fn issue_details(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        number: u64,
    ) -> Result<Option<IssueDetails>, Failure> {
        self.issue(config, token, repository, number).await
    }

    async fn list_change_requests(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Option<Vec<ChangeRequest>>, Failure> {
        self.pull_requests(config, token, repository).await
    }
    async fn change_request_details(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        number: u64,
    ) -> Result<Option<ChangeRequestDetails>, Failure> {
        self.pull_details(config, token, repository, number).await
    }

    async fn list_workflows(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, Failure> {
        self.workflows(config, token, repository).await
    }
    async fn list_workflow_runs(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, Failure> {
        let mut runs = self.runs(config, token, repository).await?;
        self.run_relationships(config, token, repository, &mut runs)
            .await;
        Ok(runs)
    }
    async fn workflow_run(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run_id: &str,
    ) -> Result<Option<WorkflowRun>, Failure> {
        let mut run = self.run(config, token, repository, run_id).await?;
        if let Some(run) = &mut run {
            self.run_relationships(config, token, repository, std::slice::from_mut(run))
                .await;
        }
        Ok(run)
    }
    async fn workflow_run_logs(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        self.logs(config, token, repository, run).await
    }
}

#[cfg(test)]
mod tests;
