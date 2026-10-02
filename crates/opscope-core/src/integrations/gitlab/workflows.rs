use super::*;
use crate::domain::{Relationships, RunLifecycle, RunOutcome, WorkflowRunLog, WorkflowState};

const WORKFLOW_ID: &str = "pipeline";

#[derive(Deserialize)]
struct Pipeline {
    id: u64,
    iid: u64,
    name: Option<String>,
    status: String,
    source: String,
    #[serde(rename = "ref")]
    branch: Option<String>,
    sha: String,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    user: Option<User>,
    web_url: String,
}

impl From<Pipeline> for WorkflowRun {
    fn from(value: Pipeline) -> Self {
        let (lifecycle, outcome) = match value.status.as_str() {
            "created"
            | "waiting_for_resource"
            | "preparing"
            | "waiting_for_callback"
            | "pending"
            | "manual"
            | "scheduled" => (RunLifecycle::Queued, RunOutcome::Unknown),
            "running" | "canceling" => (RunLifecycle::Running, RunOutcome::Unknown),
            "success" => (RunLifecycle::Completed, RunOutcome::Success),
            "failed" => (RunLifecycle::Completed, RunOutcome::Failure),
            "canceled" => (RunLifecycle::Completed, RunOutcome::Cancelled),
            "skipped" => (RunLifecycle::Completed, RunOutcome::Skipped),
            _ => (RunLifecycle::Unknown, RunOutcome::Unknown),
        };
        Self {
            relationships: Relationships::default(),
            id: value.id.to_string(),
            workflow_id: WORKFLOW_ID.into(),
            run_number: value.iid,
            attempt: 1,
            title: value
                .name
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| format!("Pipeline #{}", value.iid)),
            lifecycle,
            outcome,
            branch: value.branch,
            commit_sha: value.sha,
            actor: value.user.map(|user| user.username),
            trigger: value.source,
            created_at: value.created_at,
            started_at: value.started_at,
            updated_at: value.finished_at.unwrap_or(value.updated_at),
            web_url: value.web_url,
            provider_status: value.status.clone(),
            provider_conclusion: (lifecycle == RunLifecycle::Completed).then_some(value.status),
        }
    }
}

#[derive(Deserialize)]
struct ProjectSettings {
    builds_access_level: Option<String>,
    ci_config_path: Option<String>,
}
#[derive(Deserialize)]
struct Job {
    id: u64,
    name: String,
    stage: String,
}

impl GitLabClient {
    pub(super) async fn workflows(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, Failure> {
        let project: ProjectSettings =
            http::json(self.request(config, token, &["projects", &repository.id])?).await?;
        if project.builds_access_level.as_deref() == Some("disabled") {
            return Ok(Vec::new());
        }
        Ok(vec![Workflow {
            id: WORKFLOW_ID.into(),
            name: "CI/CD pipeline".into(),
            path: project
                .ci_config_path
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| ".gitlab-ci.yml".into()),
            state: WorkflowState::Active,
            web_url: format!("{}/-/pipelines", repository.web_url.trim_end_matches('/')),
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
                &["projects", &repository.id, "pipelines"],
                &[("order_by", "id"), ("sort", "desc")],
                http::RUN_PAGES,
                true,
            )
            .await?;
        // Older GitLab versions omit start times and actors from list responses.
        // Preserve missing metadata rather than making a request for every old run.
        Ok(pipelines.into_iter().map(WorkflowRun::from).collect())
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
            &["projects", &repository.id, "pipelines", run_id],
        )?)
        .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(http::status_failure(response.status()));
        }
        Ok(Some(WorkflowRun::from(
            http::decode::<Pipeline>(response).await?,
        )))
    }

    pub(super) async fn logs(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        let jobs: Vec<Job> = self
            .pages(
                config,
                token,
                &["projects", &repository.id, "pipelines", &run.id, "jobs"],
                &[("include_retried", "false")],
                http::MAX_PAGES,
                false,
            )
            .await
            .map_err(http::log_failure)?;
        let mut files = Vec::new();
        let mut remaining = http::LOG_TOTAL_LIMIT;
        let mut truncated = jobs.len() > http::LOG_FILES_LIMIT;
        for job in jobs.into_iter().take(http::LOG_FILES_LIMIT) {
            if remaining == 0 {
                truncated = true;
                break;
            }
            let request = self
                .request(
                    config,
                    token,
                    &[
                        "projects",
                        &repository.id,
                        "jobs",
                        &job.id.to_string(),
                        "trace",
                    ],
                )
                .map_err(http::log_failure)?;
            let (content, shortened) =
                match http::log_text(request, remaining.min(http::LOG_FILE_LIMIT)).await {
                    Ok(log) => log,
                    Err(WorkflowRunLogsFailure::LogsUnavailable) => continue,
                    Err(error) => return Err(error),
                };
            remaining = remaining.saturating_sub(content.len());
            truncated |= shortened;
            files.push(WorkflowRunLog {
                name: format!("{}/{} ({})", job.stage, job.name, job.id),
                content,
            });
        }
        Ok(WorkflowRunLogs {
            files,
            truncated,
        })
    }
}

#[cfg(test)]
mod tests;
