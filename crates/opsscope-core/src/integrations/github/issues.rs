use super::{
    ACCEPT_VALUE, API_VERSION, GitHubChangeRequestAuthor, GitHubClient, GitHubGraphQlResponse,
    GitHubLabelConnection, GitHubPageInfo, USER_AGENT, graphql_failure, report_graphql_errors,
    response_failure,
};
use crate::application::{ConnectionConfiguration, ConnectionValidationFailure, ProviderToken};
use crate::domain::{Issue, IssueComment, IssueDetails, IssueState, Repository};
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, USER_AGENT as USER_AGENT_HEADER};
use serde::Deserialize;

const ISSUES_PER_PAGE: usize = 100;
const MAX_ISSUE_PAGES: usize = 10;

fn request(
    client: &GitHubClient,
    configuration: &ConnectionConfiguration,
    token: &ProviderToken,
    repository: &Repository,
    after: Option<&str>,
) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
    const QUERY: &str = r#"
      query OpenIssues($owner: String!, $name: String!, $first: Int!, $after: String) {
        repository(owner: $owner, name: $name) {
          issues(first: $first, after: $after, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
            nodes {
              id number title state createdAt updatedAt url
              author { login }
              labels(first: 20) { nodes { name } }
              assignees(first: 20) { nodes { login } }
              comments { totalCount }
            }
            pageInfo { hasNextPage endCursor }
          }
        }
      }
    "#;
    Ok(client
        .client
        .post(GitHubClient::graphql_url(configuration)?)
        .bearer_auth(token.expose())
        .header(ACCEPT, ACCEPT_VALUE)
        .header(USER_AGENT_HEADER, USER_AGENT)
        .header("X-GitHub-Api-Version", API_VERSION)
        .json(&serde_json::json!({
            "query": QUERY,
            "variables": {
                "owner": repository.owner,
                "name": repository.name,
                "first": ISSUES_PER_PAGE,
                "after": after,
            }
        })))
}

fn details_request(
    client: &GitHubClient,
    configuration: &ConnectionConfiguration,
    token: &ProviderToken,
    repository: &Repository,
    number: u64,
) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
    const QUERY: &str = r#"
      query IssueDetails($owner: String!, $name: String!, $number: Int!) {
        repository(owner: $owner, name: $name) {
          issue(number: $number) {
            id number title state createdAt updatedAt url body
            author { login }
            labels(first: 20) { nodes { name } }
            assignees(first: 20) { nodes { login } }
            milestone { title }
            comments(last: 100) {
              totalCount
              nodes { id author { login } body createdAt updatedAt }
            }
          }
        }
      }
    "#;
    Ok(client
        .client
        .post(GitHubClient::graphql_url(configuration)?)
        .bearer_auth(token.expose())
        .header(ACCEPT, ACCEPT_VALUE)
        .header(USER_AGENT_HEADER, USER_AGENT)
        .header("X-GitHub-Api-Version", API_VERSION)
        .json(&serde_json::json!({
            "query": QUERY,
            "variables": {
                "owner": repository.owner,
                "name": repository.name,
                "number": number,
            }
        })))
}

pub(super) async fn list(
    client: &GitHubClient,
    configuration: &ConnectionConfiguration,
    token: &ProviderToken,
    repository: &Repository,
) -> Result<Option<Vec<Issue>>, ConnectionValidationFailure> {
    let mut issues = Vec::new();
    let mut after = None;
    for _ in 0..MAX_ISSUE_PAGES {
        let response = request(client, configuration, token, repository, after.as_deref())?
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
        let response = response
            .json::<GitHubGraphQlResponse<GitHubIssueData>>()
            .await
            .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
        if !response.errors.is_empty() {
            report_graphql_errors(&response.errors);
            return Err(graphql_failure(&response.errors));
        }
        let repository = response
            .data
            .and_then(|data| data.repository)
            .ok_or_else(|| graphql_failure(&response.errors))?;
        issues.extend(
            repository
                .issues
                .nodes
                .into_iter()
                .flatten()
                .map(|issue| issue.summary()),
        );
        if !repository.issues.page_info.has_next_page {
            return Ok(Some(issues));
        }
        after = repository.issues.page_info.end_cursor;
        if after.is_none() {
            return Err(ConnectionValidationFailure::UnexpectedResponse);
        }
    }
    Err(ConnectionValidationFailure::UnexpectedResponse)
}

pub(super) async fn details(
    client: &GitHubClient,
    configuration: &ConnectionConfiguration,
    token: &ProviderToken,
    repository: &Repository,
    number: u64,
) -> Result<Option<IssueDetails>, ConnectionValidationFailure> {
    let response = details_request(client, configuration, token, repository, number)?
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
    let response = response
        .json::<GitHubGraphQlResponse<GitHubIssueDetailsData>>()
        .await
        .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
    if !response.errors.is_empty() {
        report_graphql_errors(&response.errors);
        return Err(graphql_failure(&response.errors));
    }
    let repository = response
        .data
        .and_then(|data| data.repository)
        .ok_or_else(|| graphql_failure(&response.errors))?;
    Ok(repository.issue.map(GitHubIssue::into_details))
}

#[derive(Deserialize)]
struct GitHubIssueData {
    repository: Option<GitHubIssueRepository>,
}

#[derive(Deserialize)]
struct GitHubIssueDetailsData {
    repository: Option<GitHubIssueDetailsRepository>,
}

#[derive(Deserialize)]
struct GitHubIssueDetailsRepository {
    issue: Option<GitHubIssue>,
}

#[derive(Deserialize)]
struct GitHubIssueRepository {
    issues: GitHubIssueConnection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubIssueConnection {
    nodes: Vec<Option<GitHubIssue>>,
    page_info: GitHubPageInfo,
}

#[derive(Deserialize)]
struct GitHubAssigneeConnection {
    nodes: Vec<Option<GitHubChangeRequestAuthor>>,
}

#[derive(Deserialize)]
struct GitHubMilestone {
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubIssueComment {
    id: String,
    author: Option<GitHubChangeRequestAuthor>,
    body: String,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubIssueCommentConnection {
    total_count: u64,
    #[serde(default)]
    nodes: Vec<Option<GitHubIssueComment>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubIssue {
    id: String,
    number: u64,
    title: String,
    author: Option<GitHubChangeRequestAuthor>,
    state: String,
    labels: GitHubLabelConnection,
    assignees: GitHubAssigneeConnection,
    comments: GitHubIssueCommentConnection,
    created_at: String,
    updated_at: String,
    url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    milestone: Option<GitHubMilestone>,
}

impl GitHubIssue {
    fn summary(&self) -> Issue {
        Issue {
            id: self.id.clone(),
            number: self.number,
            title: self.title.clone(),
            author: self.author.as_ref().map(|author| author.login.clone()),
            state: if self.state == "OPEN" {
                IssueState::Open
            } else {
                IssueState::Closed
            },
            labels: self
                .labels
                .nodes
                .iter()
                .flatten()
                .map(|label| label.name.clone())
                .collect(),
            assignees: self
                .assignees
                .nodes
                .iter()
                .flatten()
                .map(|assignee| assignee.login.clone())
                .collect(),
            comment_count: self.comments.total_count,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            web_url: self.url.clone(),
        }
    }

    fn into_details(self) -> IssueDetails {
        let issue = self.summary();
        IssueDetails {
            issue,
            body: self.body.filter(|body| !body.trim().is_empty()),
            milestone: self.milestone.map(|milestone| milestone.title),
            comments: self
                .comments
                .nodes
                .into_iter()
                .flatten()
                .map(|comment| IssueComment {
                    id: comment.id,
                    author: comment.author.map(|author| author.login),
                    body: comment.body,
                    created_at: comment.created_at,
                    updated_at: comment.updated_at,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests;
