//! Gitea Actions adapter (requires the workflow API available in Gitea 1.26+).

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

const DEFAULT_SERVER_URL: &str = "https://gitea.com";

pub struct GiteaClient {
    client: Client,
}

impl GiteaClient {
    async fn pages<T: DeserializeOwned>(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        path: &[&str],
        query: &[(&str, &str)],
    ) -> Result<Vec<T>, Failure> {
        let mut items = Vec::new();
        for page in 1..=10 {
            let response = http::send(
                self.request(config, token, path)?
                    .query(query)
                    .query(&[("limit", http::PAGE_SIZE), ("page", page)]),
            )
            .await?;
            if !response.status().is_success() {
                return Err(http::status_failure(response.status()));
            }
            let total = response
                .headers()
                .get("x-total-count")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<usize>().ok());
            let values: Vec<T> = http::decode(response).await?;
            let done = total.map_or(values.len() < http::PAGE_SIZE, |count| {
                items.len() + values.len() >= count
            });
            if values.is_empty() && !done {
                return Err(Failure::UnexpectedResponse);
            }
            items.extend(values);
            if done {
                return Ok(items);
            }
        }
        Err(Failure::UnexpectedResponse)
    }

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
        let base = format!("{}/api/v1/", configured.unique_key);
        Ok(self
            .client
            .get(http::endpoint(&base, path)?)
            .header("Authorization", format!("token {}", token.expose()))
            .header("Accept", "application/json"))
    }
}

#[derive(Deserialize)]
struct User {
    id: u64,
    login: String,
    #[serde(default)]
    full_name: String,
    html_url: Option<String>,
}
#[derive(Deserialize)]
struct Repo {
    id: u64,
    owner: Owner,
    name: String,
    description: Option<String>,
    private: bool,
    archived: bool,
    html_url: String,
}
#[derive(Deserialize)]
struct Owner {
    login: String,
}
impl From<Repo> for Repository {
    fn from(value: Repo) -> Self {
        Self {
            id: value.id.to_string(),
            owner: value.owner.login,
            name: value.name,
            description: value.description,
            visibility: if value.private {
                RepositoryVisibility::Private
            } else {
                RepositoryVisibility::Public
            },
            web_url: value.html_url,
        }
    }
}

#[async_trait]
impl SourceModule for GiteaClient {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "gitea".into(),
            name: "Gitea".into(),
            abbreviation: "GT".into(),
            description: "Repositories, Actions workflows, logs, pull requests, and issues".into(),
            capabilities: vec![
                SourceCapability::Workflows,
                SourceCapability::ChangeRequests,
                SourceCapability::Issues,
            ],
            credential: CredentialField {
                label: "Access token".into(),
                placeholder: "Gitea access token".into(),
                help: "Use a token with read:user, read:repository, and read:issue access. \
                    Requires Gitea 1.26+ with Actions enabled."
                    .into(),
            },
            connection_fields: vec![ConnectionField {
                input_type: crate::application::ConnectionFieldType::Url,
                key: SERVER_URL_KEY.into(),
                label: "Server URL".into(),
                placeholder: "https://gitea.example.com".into(),
                default_value: DEFAULT_SERVER_URL.into(),
                help: "The HTTPS origin of your Gitea instance.".into(),
            }],
        }
    }

    fn configure(&self, config: &ConnectionConfiguration) -> Result<ConfiguredSource, Failure> {
        http::configure_server(config, DEFAULT_SERVER_URL)
    }

    async fn validate(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<ValidatedAccount, Failure> {
        let user: User = http::json(self.request(config, token, &["user"])?).await?;
        Ok(ValidatedAccount {
            external_id: user.id.to_string(),
            name: if user.full_name.is_empty() {
                user.login.clone()
            } else {
                user.full_name
            },
            handle: Some(user.login),
            profile_url: user.html_url,
        })
    }

    async fn list_repositories(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<Vec<Repository>, Failure> {
        let mut repositories = Vec::new();
        let mut received = 0;
        for page in 1..=http::MAX_PAGES {
            let response = http::send(
                self.request(config, token, &["user", "repos"])?
                    .query(&[("limit", http::PAGE_SIZE), ("page", page)]),
            )
            .await?;
            if !response.status().is_success() {
                return Err(http::status_failure(response.status()));
            }
            let total = response
                .headers()
                .get("x-total-count")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<usize>().ok());
            let batch: Vec<Repo> = http::decode(response).await?;
            received += batch.len();
            let done = batch.is_empty()
                || total.map_or(batch.len() < http::PAGE_SIZE, |count| received >= count);
            repositories.extend(
                batch
                    .into_iter()
                    .filter(|repo| !repo.archived)
                    .map(Repository::from),
            );
            if done {
                return Ok(repositories);
            }
        }
        Err(Failure::UnexpectedResponse)
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
