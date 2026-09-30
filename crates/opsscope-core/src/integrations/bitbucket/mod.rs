//! Bitbucket Cloud API-token adapter, with one connection per workspace.

mod change_requests;
mod relationships;
mod workflows;

use super::http;
use crate::application::{
    ConfiguredSource, ConnectionConfiguration, ConnectionField,
    ConnectionValidationFailure as Failure, CredentialField, ProviderToken, SourceCapability,
    SourceDescriptor, SourceModule, ValidatedAccount, WorkflowRunLogsFailure,
};
use crate::domain::{
    ChangeRequest, ChangeRequestDetails, Repository, RepositoryVisibility, Workflow, WorkflowRun,
    WorkflowRunLogs,
};
use async_trait::async_trait;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, de::DeserializeOwned};

const API_BASE: &str = "https://api.bitbucket.org/2.0/";
const WORKSPACE_KEY: &str = "workspace";
const EMAIL_KEY: &str = "email";

pub struct BitbucketClient {
    client: Client,
    api_base: Url,
}

impl BitbucketClient {
    pub fn new() -> Result<Self, Failure> {
        Ok(Self {
            client: http::client()?,
            api_base: Url::parse(API_BASE).map_err(|_| Failure::InvalidConfiguration)?,
        })
    }

    fn request_url(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        url: Url,
    ) -> Result<RequestBuilder, Failure> {
        let configured = self.configure(config)?;
        if url.origin() != self.api_base.origin()
            || !url.path().starts_with(self.api_base.path())
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(Failure::UnexpectedResponse);
        }
        Ok(self
            .client
            .get(url)
            .basic_auth(&configured.configuration[EMAIL_KEY], Some(token.expose()))
            .header("Accept", "application/json"))
    }

    fn request(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        path: &[&str],
    ) -> Result<RequestBuilder, Failure> {
        self.request_url(config, token, http::endpoint(self.api_base.as_str(), path)?)
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
        let mut url = http::endpoint(self.api_base.as_str(), path)?;
        url.query_pairs_mut()
            .extend_pairs(query.iter().copied())
            .append_pair("pagelen", &http::PAGE_SIZE.to_string());
        let original = url.clone();
        let mut items = Vec::new();
        let mut visited = std::collections::HashSet::new();
        for _ in 0..max_pages {
            if !visited.insert(url.as_str().to_owned()) {
                return Err(Failure::UnexpectedResponse);
            }
            let page: Page<T> = http::json(self.request_url(config, token, url)?).await?;
            items.extend(page.values);
            let Some(next) = page.next.filter(|value| !value.is_empty()) else {
                return Ok(items);
            };
            url = safe_next(&original, &next)?;
        }
        if recent_only {
            Ok(items)
        } else {
            Err(Failure::UnexpectedResponse)
        }
    }
}

fn safe_next(original: &Url, next: &str) -> Result<Url, Failure> {
    let url = Url::parse(next).map_err(|_| Failure::UnexpectedResponse)?;
    if url.origin() != original.origin()
        || url.path() != original.path()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Failure::UnexpectedResponse);
    }
    Ok(url)
}

#[derive(Deserialize)]
struct Page<T> {
    values: Vec<T>,
    next: Option<String>,
}
#[derive(Deserialize)]
struct User {
    uuid: String,
    display_name: String,
    nickname: Option<String>,
    links: Links,
}
#[derive(Deserialize)]
struct Link {
    href: String,
}
#[derive(Deserialize)]
struct Links {
    html: Link,
}
#[derive(Deserialize)]
struct Workspace {
    slug: String,
}
#[derive(Deserialize)]
struct Repo {
    uuid: String,
    slug: String,
    workspace: Workspace,
    description: Option<String>,
    is_private: bool,
    #[serde(default)]
    is_archived: bool,
    links: Links,
}
impl From<Repo> for Repository {
    fn from(value: Repo) -> Self {
        Self {
            id: value.uuid,
            owner: value.workspace.slug,
            name: value.slug,
            description: value.description,
            visibility: if value.is_private {
                RepositoryVisibility::Private
            } else {
                RepositoryVisibility::Public
            },
            web_url: value.links.html.href,
        }
    }
}

#[async_trait]
impl SourceModule for BitbucketClient {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "bitbucket".into(),
            name: "Bitbucket Cloud".into(),
            abbreviation: "BB".into(),
            description: "Workspace repositories, Pipelines logs, and pull requests".into(),
            capabilities: vec![
                SourceCapability::Workflows,
                SourceCapability::ChangeRequests,
            ],
            credential: CredentialField {
                label: "API token".into(),
                placeholder: "Atlassian API token".into(),
                help: "Use a scoped API token with read:user:bitbucket, \
                    read:repository:bitbucket, read:pipeline:bitbucket, \
                    and read:pullrequest:bitbucket. \
                    App passwords are not supported."
                    .into(),
            },
            connection_fields: vec![
                ConnectionField {
                    input_type: crate::application::ConnectionFieldType::Text,
                    key: WORKSPACE_KEY.into(),
                    label: "Workspace".into(),
                    placeholder: "your-workspace".into(),
                    default_value: String::new(),
                    help: "The workspace slug from your Bitbucket URL, not its display name."
                        .into(),
                },
                ConnectionField {
                    input_type: crate::application::ConnectionFieldType::Email,
                    key: EMAIL_KEY.into(),
                    label: "Atlassian account email".into(),
                    placeholder: "you@example.com".into(),
                    default_value: String::new(),
                    help: "The email address that owns the API token.".into(),
                },
            ],
        }
    }

    fn configure(&self, config: &ConnectionConfiguration) -> Result<ConfiguredSource, Failure> {
        if config
            .keys()
            .any(|key| key != WORKSPACE_KEY && key != EMAIL_KEY)
        {
            return Err(Failure::InvalidConfiguration);
        }
        let workspace = config
            .get(WORKSPACE_KEY)
            .ok_or(Failure::InvalidConfiguration)?
            .trim()
            .to_ascii_lowercase();
        let email = config
            .get(EMAIL_KEY)
            .ok_or(Failure::InvalidConfiguration)?
            .trim()
            .to_owned();
        if workspace.is_empty()
            || workspace.len() > 100
            || !workspace
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            || !email.contains('@')
            || email.contains(':')
            || email.chars().any(char::is_whitespace)
            || email.chars().any(char::is_control)
        {
            return Err(Failure::InvalidConfiguration);
        }
        Ok(ConfiguredSource {
            unique_key: format!("https://bitbucket.org/{workspace}"),
            label: workspace.clone(),
            configuration: [(WORKSPACE_KEY.into(), workspace), (EMAIL_KEY.into(), email)]
                .into_iter()
                .collect(),
        })
    }

    async fn validate(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<ValidatedAccount, Failure> {
        let configured = self.configure(config)?;
        let user: User =
            http::json(self.request(&configured.configuration, token, &["user"])?).await?;
        // Validate workspace access too, without loading its repository inventory.
        let _: Page<Repo> = http::json(
            self.request(
                &configured.configuration,
                token,
                &["repositories", &configured.configuration[WORKSPACE_KEY]],
            )?
            .query(&[("pagelen", 1)]),
        )
        .await?;
        Ok(ValidatedAccount {
            external_id: user.uuid,
            name: user.display_name,
            handle: user.nickname,
            profile_url: Some(user.links.html.href),
        })
    }

    async fn list_repositories(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Result<Vec<Repository>, Failure> {
        let configured = self.configure(config)?;
        let repos: Vec<Repo> = self
            .pages(
                &configured.configuration,
                token,
                &["repositories", &configured.configuration[WORKSPACE_KEY]],
                &[],
                http::MAX_PAGES,
                false,
            )
            .await?;
        Ok(repos
            .into_iter()
            .filter(|repo| !repo.is_archived)
            .map(Repository::from)
            .collect())
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
