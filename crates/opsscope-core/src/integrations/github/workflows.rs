use super::*;

impl GitHubClient {
    pub(super) fn workflows_request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        page: usize,
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        let mut url = Self::api_url(
            configuration,
            &format!(
                "repos/{}/{}/actions/workflows",
                repository.owner, repository.name
            ),
        )?;
        url.query_pairs_mut()
            .append_pair("per_page", &WORKFLOWS_PER_PAGE.to_string())
            .append_pair("page", &page.to_string());
        Ok(self
            .client
            .get(url)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION))
    }

    pub(super) fn workflow_runs_request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        page: usize,
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        let mut url = Self::api_url(
            configuration,
            &format!(
                "repos/{}/{}/actions/runs",
                repository.owner, repository.name
            ),
        )?;
        url.query_pairs_mut()
            .append_pair("per_page", &WORKFLOW_RUNS_PER_PAGE.to_string())
            .append_pair("page", &page.to_string());
        Ok(self
            .client
            .get(url)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION))
    }

    pub(super) fn workflow_run_logs_request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        let url = Self::api_url(
            configuration,
            &format!(
                "repos/{}/{}/actions/runs/{}/attempts/{}/logs",
                repository.owner, repository.name, run.id, run.attempt
            ),
        )?;
        Ok(self
            .client
            .get(url)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION))
    }

    pub(super) fn workflow_run_request(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run_id: &str,
    ) -> Result<reqwest::RequestBuilder, ConnectionValidationFailure> {
        if run_id.is_empty() || !run_id.chars().all(|character| character.is_ascii_digit()) {
            return Err(ConnectionValidationFailure::UnexpectedResponse);
        }
        let url = Self::api_url(
            configuration,
            &format!(
                "repos/{}/{}/actions/runs/{run_id}",
                repository.owner, repository.name
            ),
        )?;
        Ok(self
            .client
            .get(url)
            .bearer_auth(token.expose())
            .header(ACCEPT, ACCEPT_VALUE)
            .header(USER_AGENT_HEADER, USER_AGENT)
            .header("X-GitHub-Api-Version", API_VERSION))
    }
}

#[derive(Deserialize)]
pub(super) struct GitHubWorkflowPage {
    pub(super) workflows: Vec<GitHubWorkflow>,
}

#[derive(Deserialize)]
pub(super) struct GitHubWorkflow {
    pub(super) id: u64,
    pub(super) name: String,
    pub(super) path: String,
    pub(super) state: String,
    pub(super) html_url: String,
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
pub(super) struct GitHubWorkflowRunPage {
    pub(super) workflow_runs: Vec<GitHubWorkflowRun>,
}

#[derive(Deserialize)]
pub(super) struct GitHubWorkflowRunActor {
    pub(super) login: String,
}

#[derive(Deserialize)]
pub(super) struct GitHubWorkflowRun {
    #[serde(skip)]
    pub(super) relevance: crate::domain::Relevance,
    #[serde(default)]
    pub(super) pull_requests: Vec<GitHubRunPullRequest>,
    pub(super) id: u64,
    pub(super) workflow_id: u64,
    pub(super) run_number: u64,
    pub(super) run_attempt: u64,
    pub(super) display_title: String,
    pub(super) status: String,
    pub(super) conclusion: Option<String>,
    pub(super) head_branch: Option<String>,
    pub(super) head_sha: String,
    pub(super) actor: Option<GitHubWorkflowRunActor>,
    pub(super) event: String,
    pub(super) created_at: String,
    pub(super) run_started_at: Option<String>,
    pub(super) updated_at: String,
    pub(super) html_url: String,
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
            relevance: run.relevance,
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

#[derive(Deserialize)]
pub(super) struct GitHubRunPullRequest {
    pub(super) number: u64,
}
