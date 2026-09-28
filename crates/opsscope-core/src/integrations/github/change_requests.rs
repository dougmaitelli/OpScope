use super::*;

impl GitHubClient {
    pub(super) fn change_requests_request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        after: Option<&str>,
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        const QUERY: &str = r#"
          query OpenChangeRequests($owner: String!, $name: String!, $first: Int!, $after: String) {
            repository(owner: $owner, name: $name) {
              pullRequests(first: $first, after: $after, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
                nodes {
                  id number title state isDraft reviewDecision mergeStateStatus createdAt updatedAt url
                  author { login }
                  headRefName
                  baseRefName
                  commits(last: 1) {
                    nodes { commit { statusCheckRollup { state } } }
                  }
                }
                pageInfo { hasNextPage endCursor }
              }
            }
          }
        "#;
        Ok(self
            .client
            .post(Self::graphql_url(configuration)?)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION)
            .json(&serde_json::json!({
                "query": QUERY,
                "variables": {
                    "owner": repository.owner,
                    "name": repository.name,
                    "first": CHANGE_REQUESTS_PER_PAGE,
                    "after": after,
                }
            })))
    }

    pub(super) fn change_request_details_request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        number: u64,
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        const QUERY: &str = r#"
          query ChangeRequestDetails($owner: String!, $name: String!, $number: Int!) {
            repository(owner: $owner, name: $name) {
              pullRequest(number: $number) {
                id number title state isDraft reviewDecision mergeStateStatus createdAt updatedAt url body
                author { login }
                headRefName
                baseRefName
                labels(first: 50) { nodes { name } }
                reviews(first: 100) { nodes { author { login } state submittedAt } }
                commits(last: 1) {
                  nodes {
                    commit {
                      oid messageHeadline committedDate
                      author { name user { login } }
                      statusCheckRollup {
                        state
                        contexts(first: 100) {
                          nodes {
                            __typename
                            ... on CheckRun {
                              name status conclusion detailsUrl
                              checkSuite { workflowRun { databaseId } }
                            }
                            ... on StatusContext { context state targetUrl }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        "#;
        Ok(self
            .client
            .post(Self::graphql_url(configuration)?)
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

    pub(super) async fn fetch_change_requests(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Option<Vec<ChangeRequest>>, ConnectionValidationFailure> {
        let mut change_requests = Vec::new();
        let mut after = None;
        for _ in 0..MAX_CHANGE_REQUEST_PAGES {
            let response = self
                .change_requests_request(configuration, token, repository, after.as_deref())?
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
                .json::<GitHubGraphQlResponse<GitHubChangeRequestData>>()
                .await
                .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
            if !response.errors.is_empty() {
                report_graphql_errors(&response.errors);
            }
            let repository = response
                .data
                .and_then(|data| data.repository)
                .ok_or_else(|| graphql_failure(&response.errors))?;
            change_requests.extend(
                repository
                    .pull_requests
                    .nodes
                    .into_iter()
                    .flatten()
                    .map(ChangeRequest::from),
            );
            if !repository.pull_requests.page_info.has_next_page {
                return Ok(Some(change_requests));
            }
            after = repository.pull_requests.page_info.end_cursor;
            if after.is_none() {
                return Err(ConnectionValidationFailure::UnexpectedResponse);
            }
        }
        Err(ConnectionValidationFailure::UnexpectedResponse)
    }

    pub(super) async fn fetch_change_request_details(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        number: u64,
    ) -> Result<Option<ChangeRequestDetails>, ConnectionValidationFailure> {
        let response = self
            .change_request_details_request(configuration, token, repository, number)?
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
            .json::<GitHubGraphQlResponse<GitHubChangeRequestDetailsData>>()
            .await
            .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
        if !response.errors.is_empty() {
            report_graphql_errors(&response.errors);
        }
        let repository = response
            .data
            .and_then(|data| data.repository)
            .ok_or_else(|| graphql_failure(&response.errors))?;
        Ok(repository
            .pull_request
            .map(GitHubChangeRequest::into_details))
    }
}

#[derive(Deserialize)]
pub(super) struct GitHubChangeRequestData {
    repository: Option<GitHubChangeRequestRepository>,
}

#[derive(Deserialize)]
pub(super) struct GitHubChangeRequestDetailsData {
    repository: Option<GitHubChangeRequestDetailsRepository>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GitHubChangeRequestDetailsRepository {
    pull_request: Option<GitHubChangeRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GitHubChangeRequestRepository {
    pull_requests: GitHubChangeRequestConnection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GitHubChangeRequestConnection {
    nodes: Vec<Option<GitHubChangeRequest>>,
    page_info: GitHubPageInfo,
}

#[derive(Deserialize)]
pub(super) struct GitHubStatusCheckRollup {
    state: String,
    #[serde(default)]
    contexts: Option<GitHubCheckContextConnection>,
}

#[derive(Deserialize)]
pub(super) struct GitHubCheckContextConnection {
    nodes: Vec<Option<GitHubCheckContext>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GitHubCheckSuite {
    workflow_run: Option<GitHubGraphQlWorkflowRun>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GitHubGraphQlWorkflowRun {
    database_id: Option<u64>,
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum GitHubCheckContext {
    CheckRun {
        name: String,
        status: String,
        conclusion: Option<String>,
        #[serde(rename = "detailsUrl")]
        details_url: Option<String>,
        #[serde(rename = "checkSuite")]
        check_suite: Option<GitHubCheckSuite>,
    },
    StatusContext {
        context: String,
        state: String,
        #[serde(rename = "targetUrl")]
        target_url: Option<String>,
    },
}

#[derive(Deserialize)]
pub(super) struct GitHubCommitAuthorUser {
    login: String,
}

#[derive(Deserialize)]
pub(super) struct GitHubCommitAuthor {
    name: Option<String>,
    user: Option<GitHubCommitAuthorUser>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GitHubCommit {
    status_check_rollup: Option<GitHubStatusCheckRollup>,
    #[serde(default)]
    oid: Option<String>,
    #[serde(default)]
    message_headline: Option<String>,
    #[serde(default)]
    committed_date: Option<String>,
    #[serde(default)]
    author: Option<GitHubCommitAuthor>,
}

#[derive(Deserialize)]
pub(super) struct GitHubCommitNode {
    commit: GitHubCommit,
}

#[derive(Deserialize)]
pub(super) struct GitHubCommitConnection {
    nodes: Vec<Option<GitHubCommitNode>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GitHubReview {
    author: Option<GitHubChangeRequestAuthor>,
    state: String,
    submitted_at: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct GitHubReviewConnection {
    nodes: Vec<Option<GitHubReview>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GitHubChangeRequest {
    id: String,
    number: u64,
    title: String,
    author: Option<GitHubChangeRequestAuthor>,
    head_ref_name: String,
    base_ref_name: String,
    state: String,
    is_draft: bool,
    review_decision: Option<String>,
    merge_state_status: String,
    commits: GitHubCommitConnection,
    created_at: String,
    updated_at: String,
    url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    labels: Option<GitHubLabelConnection>,
    #[serde(default)]
    reviews: Option<GitHubReviewConnection>,
}

impl From<GitHubChangeRequest> for ChangeRequest {
    fn from(change_request: GitHubChangeRequest) -> Self {
        let state = match change_request.state.as_str() {
            "OPEN" => ChangeRequestState::Open,
            "MERGED" => ChangeRequestState::Merged,
            _ => ChangeRequestState::Closed,
        };
        let review_status = match change_request.review_decision.as_deref() {
            Some("APPROVED") => ChangeRequestReviewStatus::Approved,
            Some("CHANGES_REQUESTED") => ChangeRequestReviewStatus::ChangesRequested,
            Some("REVIEW_REQUIRED") => ChangeRequestReviewStatus::ReviewRequired,
            _ => ChangeRequestReviewStatus::Unknown,
        };
        let check_status = match change_request
            .commits
            .nodes
            .iter()
            .rev()
            .flatten()
            .next()
            .and_then(|node| node.commit.status_check_rollup.as_ref())
            .map(|rollup| rollup.state.as_str())
        {
            Some("SUCCESS") => ChangeRequestCheckStatus::Passed,
            Some("FAILURE" | "ERROR") => ChangeRequestCheckStatus::Failing,
            Some("PENDING" | "EXPECTED") => ChangeRequestCheckStatus::Running,
            _ => ChangeRequestCheckStatus::Unknown,
        };
        let merge_status = match change_request.merge_state_status.as_str() {
            "CLEAN" | "HAS_HOOKS" | "UNSTABLE" => ChangeRequestMergeStatus::Ready,
            "BLOCKED" | "BEHIND" => ChangeRequestMergeStatus::Blocked,
            "DIRTY" => ChangeRequestMergeStatus::Conflicting,
            _ => ChangeRequestMergeStatus::Unknown,
        };
        Self {
            id: change_request.id,
            number: change_request.number,
            title: change_request.title,
            author: change_request.author.map(|author| author.login),
            source_branch: change_request.head_ref_name,
            target_branch: change_request.base_ref_name,
            state,
            draft: change_request.is_draft,
            review_status,
            check_status,
            merge_status,
            created_at: change_request.created_at,
            updated_at: change_request.updated_at,
            web_url: change_request.url,
        }
    }
}

impl GitHubChangeRequest {
    pub(super) fn into_details(self) -> ChangeRequestDetails {
        let labels = self.labels.as_ref().map_or_else(Vec::new, |labels| {
            labels
                .nodes
                .iter()
                .flatten()
                .map(|label| label.name.clone())
                .collect()
        });
        let reviews = self.reviews.as_ref().map_or_else(Vec::new, |reviews| {
            reviews
                .nodes
                .iter()
                .flatten()
                .map(|review| ChangeRequestReview {
                    reviewer: review.author.as_ref().map(|author| author.login.clone()),
                    status: match review.state.as_str() {
                        "APPROVED" => ChangeRequestReviewStatus::Approved,
                        "CHANGES_REQUESTED" => ChangeRequestReviewStatus::ChangesRequested,
                        "PENDING" => ChangeRequestReviewStatus::ReviewRequired,
                        _ => ChangeRequestReviewStatus::Unknown,
                    },
                    submitted_at: review.submitted_at.clone(),
                })
                .collect()
        });
        let latest = self.commits.nodes.iter().rev().flatten().next();
        let latest_commit = latest.and_then(|node| {
            Some(ChangeRequestCommit {
                sha: node.commit.oid.clone()?,
                title: node.commit.message_headline.clone()?,
                author: node.commit.author.as_ref().and_then(|author| {
                    author
                        .user
                        .as_ref()
                        .map(|user| user.login.clone())
                        .or_else(|| author.name.clone())
                }),
                committed_at: node.commit.committed_date.clone()?,
            })
        });
        let checks = latest
            .and_then(|node| node.commit.status_check_rollup.as_ref())
            .and_then(|rollup| rollup.contexts.as_ref())
            .map_or_else(Vec::new, |contexts| {
                contexts
                    .nodes
                    .iter()
                    .flatten()
                    .map(|context| match context {
                        GitHubCheckContext::CheckRun {
                            name,
                            status,
                            conclusion,
                            details_url,
                            check_suite,
                        } => ChangeRequestCheck {
                            name: name.clone(),
                            status: if status != "COMPLETED" {
                                ChangeRequestCheckStatus::Running
                            } else {
                                match conclusion.as_deref() {
                                    Some("SUCCESS" | "NEUTRAL" | "SKIPPED") => {
                                        ChangeRequestCheckStatus::Passed
                                    }
                                    Some(
                                        "FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED"
                                        | "STARTUP_FAILURE",
                                    ) => ChangeRequestCheckStatus::Failing,
                                    _ => ChangeRequestCheckStatus::Unknown,
                                }
                            },
                            web_url: details_url.clone(),
                            workflow_run_id: check_suite
                                .as_ref()
                                .and_then(|suite| suite.workflow_run.as_ref())
                                .and_then(|run| run.database_id)
                                .map(|run_id| run_id.to_string())
                                .or_else(|| details_url.as_deref().and_then(github_actions_run_id)),
                        },
                        GitHubCheckContext::StatusContext {
                            context,
                            state,
                            target_url,
                        } => ChangeRequestCheck {
                            name: context.clone(),
                            status: match state.as_str() {
                                "SUCCESS" => ChangeRequestCheckStatus::Passed,
                                "FAILURE" | "ERROR" => ChangeRequestCheckStatus::Failing,
                                "PENDING" | "EXPECTED" => ChangeRequestCheckStatus::Running,
                                _ => ChangeRequestCheckStatus::Unknown,
                            },
                            web_url: target_url.clone(),
                            workflow_run_id: target_url.as_deref().and_then(github_actions_run_id),
                        },
                    })
                    .collect()
            });
        let body = self.body.clone().filter(|body| !body.trim().is_empty());
        ChangeRequestDetails {
            change_request: ChangeRequest::from(self),
            body,
            labels,
            reviews,
            checks,
            latest_commit,
        }
    }
}

pub(super) fn github_actions_run_id(url: &str) -> Option<String> {
    let (_, suffix) = url.split_once("/actions/runs/")?;
    let run_id = suffix.split(['/', '?', '#']).next()?;
    (!run_id.is_empty() && run_id.chars().all(|character| character.is_ascii_digit()))
        .then(|| run_id.to_owned())
}
