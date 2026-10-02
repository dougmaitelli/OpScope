use super::*;

impl GitHubClient {
    pub(super) async fn workflows(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, ConnectionValidationFailure> {
        let mut workflows = Vec::new();
        for page in 1..=MAX_WORKFLOW_PAGES {
            let response = self
                .request(
                    configuration,
                    token,
                    &[
                        "repos",
                        &repository.owner,
                        &repository.name,
                        "actions",
                        "workflows",
                    ],
                )?
                .query(&[("per_page", WORKFLOWS_PER_PAGE), ("page", page)])
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

    pub(super) async fn runs(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, ConnectionValidationFailure> {
        let mut runs = Vec::new();
        for page in 1..=MAX_WORKFLOW_RUN_PAGES {
            let response = self
                .request(
                    configuration,
                    token,
                    &[
                        "repos",
                        &repository.owner,
                        &repository.name,
                        "actions",
                        "runs",
                    ],
                )?
                .query(&[("per_page", WORKFLOW_RUNS_PER_PAGE), ("page", page)])
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

            let mut page = response
                .json::<GitHubWorkflowRunPage>()
                .await
                .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
            let page_size = page.workflow_runs.len();
            relationships::runs(
                self,
                configuration,
                token,
                repository,
                &mut page.workflow_runs,
            )
            .await;
            runs.extend(page.workflow_runs.into_iter().map(WorkflowRun::from));
            if page_size < WORKFLOW_RUNS_PER_PAGE {
                return Ok(runs);
            }
        }
        Ok(runs)
    }

    pub(super) async fn run(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run_id: &str,
    ) -> Result<Option<WorkflowRun>, ConnectionValidationFailure> {
        if run_id.is_empty() || !run_id.chars().all(|character| character.is_ascii_digit()) {
            return Err(ConnectionValidationFailure::UnexpectedResponse);
        }
        let response = self
            .request(
                configuration,
                token,
                &[
                    "repos",
                    &repository.owner,
                    &repository.name,
                    "actions",
                    "runs",
                    run_id,
                ],
            )?
            .send()
            .await
            .map_err(|_| ConnectionValidationFailure::ProviderUnavailable)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if response.status() != StatusCode::OK {
            return Err(response_failure(&response));
        }
        let mut run = response
            .json::<GitHubWorkflowRun>()
            .await
            .map_err(|_| ConnectionValidationFailure::UnexpectedResponse)?;
        relationships::runs(
            self,
            configuration,
            token,
            repository,
            std::slice::from_mut(&mut run),
        )
        .await;
        Ok(Some(WorkflowRun::from(run)))
    }

    pub(super) async fn logs(
        &self,
        configuration: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        let response = self
            .request(
                configuration,
                token,
                &[
                    "repos",
                    &repository.owner,
                    &repository.name,
                    "actions",
                    "runs",
                    &run.id,
                    "attempts",
                    &run.attempt.to_string(),
                    "logs",
                ],
            )
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

pub(super) fn read_log_archive(bytes: Vec<u8>) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
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

    Ok(WorkflowRunLogs {
        files,
        truncated,
    })
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
    pub(super) relationships: crate::domain::Relationships,
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
            relationships: run.relationships,
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
