use super::*;
use crate::domain::{Relationships, RunLifecycle, RunOutcome, WorkflowRunLog, WorkflowState};

const WORKFLOW_ID: &str = "pipelines";

#[derive(Deserialize)]
struct PipelineConfig {
    enabled: bool,
}
#[derive(Deserialize)]
pub(super) struct Pipeline {
    uuid: String,
    build_number: u64,
    state: State,
    target: Target,
    creator: Option<Creator>,
    trigger: Option<Trigger>,
    created_on: String,
    completed_on: Option<String>,
}
#[derive(Deserialize)]
struct State {
    name: String,
    result: Option<ResultState>,
}
#[derive(Deserialize)]
struct ResultState {
    name: String,
}
#[derive(Deserialize)]
struct Target {
    ref_name: Option<String>,
    commit: Commit,
    selector: Option<Selector>,
}
#[derive(Deserialize)]
struct Commit {
    hash: String,
    message: Option<String>,
}
#[derive(Deserialize)]
struct Selector {
    pattern: Option<String>,
}
#[derive(Deserialize)]
struct Creator {
    display_name: String,
}
#[derive(Deserialize)]
struct Trigger {
    name: Option<String>,
}

impl Pipeline {
    pub(super) fn into_run(self, repository: &Repository) -> WorkflowRun {
        let lifecycle = match self.state.name.as_str() {
            "PENDING" => RunLifecycle::Queued,
            "IN_PROGRESS" => RunLifecycle::Running,
            "COMPLETED" => RunLifecycle::Completed,
            _ => RunLifecycle::Unknown,
        };
        let conclusion = self.state.result.map(|result| result.name);
        let outcome = match conclusion.as_deref() {
            Some("SUCCESSFUL") => RunOutcome::Success,
            Some("FAILED" | "ERROR" | "EXPIRED") => RunOutcome::Failure,
            Some("STOPPED") => RunOutcome::Cancelled,
            _ => RunOutcome::Unknown,
        };
        let title = self
            .target
            .commit
            .message
            .as_deref()
            .and_then(|message| message.lines().next())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .or_else(|| self.target.selector.and_then(|selector| selector.pattern))
            .unwrap_or_else(|| format!("Pipeline #{}", self.build_number));
        WorkflowRun {
            relationships: Relationships::default(),
            id: self.uuid,
            workflow_id: WORKFLOW_ID.into(),
            run_number: self.build_number,
            attempt: 1,
            title,
            lifecycle,
            outcome,
            branch: self.target.ref_name,
            commit_sha: self.target.commit.hash,
            actor: self.creator.map(|creator| creator.display_name),
            trigger: self
                .trigger
                .and_then(|trigger| trigger.name)
                .unwrap_or_else(|| "unknown".into()),
            // Pipeline creation is not execution start; do not invent a duration.
            created_at: self.created_on.clone(),
            started_at: None,
            updated_at: self.completed_on.unwrap_or(self.created_on),
            web_url: format!(
                "{}/pipelines/results/{}",
                repository.web_url.trim_end_matches('/'),
                self.build_number
            ),
            provider_status: self.state.name,
            provider_conclusion: conclusion,
        }
    }
}

#[derive(Deserialize)]
struct Step {
    uuid: String,
    name: String,
    #[serde(default)]
    log_files: Vec<LogFile>,
}
#[derive(Deserialize)]
struct LogFile {
    uuid: String,
    name: Option<String>,
}

impl BitbucketClient {
    pub(super) async fn workflows(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, Failure> {
        let settings: PipelineConfig = http::json(self.request(
            config,
            token,
            &[
                "repositories",
                &repository.owner,
                &repository.id,
                "pipelines_config",
            ],
        )?)
        .await?;
        if !settings.enabled {
            return Ok(Vec::new());
        }
        Ok(vec![Workflow {
            id: WORKFLOW_ID.into(),
            name: "Pipelines".into(),
            path: "bitbucket-pipelines.yml".into(),
            state: WorkflowState::Active,
            web_url: format!("{}/pipelines", repository.web_url.trim_end_matches('/')),
        }])
    }
    pub(super) async fn runs(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, Failure> {
        if self.workflows(config, token, repository).await?.is_empty() {
            return Ok(Vec::new());
        }
        let pipelines: Vec<Pipeline> = self
            .pages(
                config,
                token,
                &[
                    "repositories",
                    &repository.owner,
                    &repository.id,
                    "pipelines",
                ],
                &[("sort", "-created_on")],
                http::RUN_PAGES,
                true,
            )
            .await?;
        Ok(pipelines
            .into_iter()
            .map(|pipeline| pipeline.into_run(repository))
            .collect())
    }
    pub(super) async fn run(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run_id: &str,
    ) -> Result<Option<WorkflowRun>, Failure> {
        let response = http::send(self.request(
            config,
            token,
            &[
                "repositories",
                &repository.owner,
                &repository.id,
                "pipelines",
                run_id,
            ],
        )?)
        .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(http::status_failure(response.status()));
        }
        Ok(Some(
            http::decode::<Pipeline>(response)
                .await?
                .into_run(repository),
        ))
    }
    pub(super) async fn logs(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        let steps: Vec<Step> = self
            .pages(
                config,
                token,
                &[
                    "repositories",
                    &repository.owner,
                    &repository.id,
                    "pipelines",
                    &run.id,
                    "steps",
                ],
                &[],
                http::MAX_PAGES,
                false,
            )
            .await
            .map_err(http::log_failure)?;
        let mut files = Vec::new();
        let mut remaining = http::LOG_TOTAL_LIMIT;
        let mut truncated = false;
        'steps: for step in steps {
            let logs = if step.log_files.is_empty() {
                vec![None]
            } else {
                step.log_files.into_iter().map(Some).collect()
            };
            for file in logs {
                if files.len() == http::LOG_FILES_LIMIT || remaining == 0 {
                    truncated = true;
                    break 'steps;
                }
                let mut path = vec![
                    "repositories",
                    &repository.owner,
                    &repository.id,
                    "pipelines",
                    &run.id,
                    "steps",
                    &step.uuid,
                ];
                let name = if let Some(file) = &file {
                    path.extend(["logs", &file.uuid]);
                    format!(
                        "{} / {}",
                        step.name,
                        file.name.as_deref().unwrap_or(&file.uuid)
                    )
                } else {
                    path.push("log");
                    step.name.clone()
                };
                let request = self
                    .request(config, token, &path)
                    .map_err(http::log_failure)?;
                let response = http::send(request).await.map_err(http::log_failure)?;
                let (content, shortened) = if response.status().is_redirection() {
                    let location = response
                        .headers()
                        .get(reqwest::header::LOCATION)
                        .and_then(|v| v.to_str().ok())
                        .ok_or(WorkflowRunLogsFailure::UnexpectedResponse)?;
                    let url = log_redirect(location)?;
                    // Signed storage URLs are fetched WITHOUT the API credential.
                    http::log_text(self.client.get(url), remaining.min(http::LOG_FILE_LIMIT))
                        .await?
                } else {
                    match http::read_log(response, remaining.min(http::LOG_FILE_LIMIT)).await {
                        Ok(log) => log,
                        Err(WorkflowRunLogsFailure::LogsUnavailable) => continue,
                        Err(error) => return Err(error),
                    }
                };
                remaining = remaining.saturating_sub(content.len());
                truncated |= shortened;
                files.push(WorkflowRunLog {
                    name,
                    content,
                });
            }
        }
        Ok(WorkflowRunLogs {
            files,
            truncated,
        })
    }
}

pub(super) fn log_redirect(location: &str) -> Result<Url, WorkflowRunLogsFailure> {
    let url = Url::parse(location).map_err(|_| WorkflowRunLogsFailure::UnexpectedResponse)?;
    let host = url
        .host_str()
        .ok_or(WorkflowRunLogsFailure::UnexpectedResponse)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || !(host.ends_with(".amazonaws.com") || host.ends_with(".bitbucket.org"))
    {
        return Err(WorkflowRunLogsFailure::UnexpectedResponse);
    }
    Ok(url)
}
