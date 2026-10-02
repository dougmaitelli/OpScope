use super::*;
use crate::domain::{Relationships, RunLifecycle, RunOutcome, WorkflowRunLog, WorkflowState};

#[derive(Deserialize)]
struct Workflows {
    workflows: Vec<ActionWorkflow>,
}
#[derive(Deserialize)]
struct ActionWorkflow {
    id: String,
    name: String,
    path: String,
    state: String,
    html_url: String,
}
#[derive(Deserialize)]
struct Runs {
    total_count: usize,
    workflow_runs: Vec<Run>,
}
#[derive(Deserialize)]
pub(super) struct Run {
    id: u64,
    path: String,
    run_number: u64,
    run_attempt: u64,
    display_title: String,
    status: String,
    conclusion: Option<String>,
    head_branch: Option<String>,
    head_sha: String,
    actor: Option<Owner>,
    event: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    html_url: String,
}

impl From<Run> for WorkflowRun {
    fn from(value: Run) -> Self {
        let lifecycle = match value.status.as_str() {
            "queued" | "waiting" | "pending" | "blocked" => RunLifecycle::Queued,
            "in_progress" | "running" => RunLifecycle::Running,
            "completed" | "success" | "failure" | "cancelled" | "skipped" => {
                RunLifecycle::Completed
            }
            _ => RunLifecycle::Unknown,
        };
        let conclusion = value
            .conclusion
            .filter(|value| !value.is_empty())
            .or_else(|| (lifecycle == RunLifecycle::Completed).then(|| value.status.clone()));
        let outcome = match conclusion.as_deref() {
            Some("success") => RunOutcome::Success,
            Some("failure" | "timed_out" | "action_required") => RunOutcome::Failure,
            Some("cancelled") => RunOutcome::Cancelled,
            Some("skipped") => RunOutcome::Skipped,
            Some("neutral") => RunOutcome::Warning,
            _ => RunOutcome::Unknown,
        };
        let started = value
            .started_at
            .filter(|time| !time.starts_with("0001-") && !time.is_empty());
        let completed = value
            .completed_at
            .filter(|time| !time.starts_with("0001-") && !time.is_empty());
        Self {
            relationships: Relationships::default(),
            id: value.id.to_string(),
            workflow_id: workflow_id(&value.path),
            run_number: value.run_number,
            attempt: value.run_attempt.max(1),
            title: value.display_title,
            lifecycle,
            outcome,
            branch: value.head_branch,
            commit_sha: value.head_sha,
            actor: value.actor.map(|actor| actor.login),
            trigger: value.event,
            // Gitea's run API does not expose created_at or updated_at. Keep missing
            // times empty rather than inventing a timestamp from the local clock.
            created_at: started.clone().unwrap_or_default(),
            updated_at: completed.or_else(|| started.clone()).unwrap_or_default(),
            started_at: started,
            web_url: value.html_url,
            provider_status: value.status,
            provider_conclusion: conclusion,
        }
    }
}

fn workflow_id(path: &str) -> String {
    path.strip_prefix(".gitea/workflows/")
        .or_else(|| path.strip_prefix(".github/workflows/"))
        .unwrap_or(path)
        .to_owned()
}

#[derive(Deserialize)]
struct Jobs {
    total_count: usize,
    jobs: Vec<Job>,
}
#[derive(Deserialize)]
struct Job {
    id: u64,
    name: String,
}

impl GiteaClient {
    pub(super) async fn workflows(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<Workflow>, Failure> {
        let data: Workflows = http::json(self.request(
            config,
            token,
            &[
                "repos",
                &repository.owner,
                &repository.name,
                "actions",
                "workflows",
            ],
        )?)
        .await?;
        Ok(data
            .workflows
            .into_iter()
            .map(|value| Workflow {
                id: workflow_id(&value.id),
                name: value.name,
                path: value.path,
                state: if value.state == "active" {
                    WorkflowState::Active
                } else {
                    WorkflowState::Disabled
                },
                web_url: value.html_url,
            })
            .collect())
    }
    pub(super) async fn runs(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, Failure> {
        let mut runs = Vec::new();
        for page in 1..=http::RUN_PAGES {
            let data: Runs = http::json(
                self.request(
                    config,
                    token,
                    &[
                        "repos",
                        &repository.owner,
                        &repository.name,
                        "actions",
                        "runs",
                    ],
                )?
                .query(&[("limit", http::PAGE_SIZE), ("page", page)]),
            )
            .await?;
            let empty = data.workflow_runs.is_empty();
            runs.extend(data.workflow_runs.into_iter().map(WorkflowRun::from));
            if empty || runs.len() >= data.total_count {
                break;
            }
        }
        Ok(runs)
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
                "repos",
                &repository.owner,
                &repository.name,
                "actions",
                "runs",
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
        Ok(Some(http::decode::<Run>(response).await?.into()))
    }
    pub(super) async fn logs(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repository: &Repository,
        run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        let mut files = Vec::new();
        let mut remaining = http::LOG_TOTAL_LIMIT;
        let mut seen = 0;
        let mut truncated = false;
        for page in 1..=http::LOG_FILES_LIMIT / http::PAGE_SIZE {
            let data: Jobs = http::json(
                self.request(
                    config,
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
                        "jobs",
                    ],
                )
                .map_err(http::log_failure)?
                .query(&[("limit", http::PAGE_SIZE), ("page", page)]),
            )
            .await
            .map_err(http::log_failure)?;
            let empty = data.jobs.is_empty();
            seen += data.jobs.len();
            for job in data.jobs {
                if remaining == 0 || files.len() == http::LOG_FILES_LIMIT {
                    truncated = true;
                    break;
                }
                let request = self
                    .request(
                        config,
                        token,
                        &[
                            "repos",
                            &repository.owner,
                            &repository.name,
                            "actions",
                            "jobs",
                            &job.id.to_string(),
                            "logs",
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
                    name: format!("{} ({})", job.name, job.id),
                    content,
                });
            }
            if empty || seen >= data.total_count {
                break;
            }
            if remaining == 0 || page == http::LOG_FILES_LIMIT / http::PAGE_SIZE {
                truncated = true;
                break;
            }
        }
        Ok(WorkflowRunLogs {
            files,
            truncated,
        })
    }
}
