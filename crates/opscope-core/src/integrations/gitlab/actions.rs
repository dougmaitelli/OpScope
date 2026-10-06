use super::*;
#[cfg(test)]
mod tests;
use crate::application::{
    ActionFailure, ActionOptions, ActionTarget, AvailableAction, SourceAction,
};
use crate::integrations::http::actions::{read, write, write_response};
use reqwest::Method;
use serde_json::json;

#[derive(Deserialize)]
struct Pipeline {
    status: String,
    updated_at: String,
}
#[derive(Deserialize)]
struct MergeRequest {
    state: String,
    sha: String,
    has_conflicts: bool,
    detailed_merge_status: String,
    rebase_in_progress: bool,
    draft: Option<bool>,
}

impl GitLabClient {
    pub(super) async fn actions(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        target: &ActionTarget,
    ) -> Result<ActionOptions, ActionFailure> {
        match target {
            ActionTarget::WorkflowRun {
                run_id,
            } => {
                let pipeline: Pipeline = read(self.request(
                    config,
                    token,
                    &["projects", &repo.id, "pipelines", run_id],
                )?)
                .await?;
                Ok(ActionOptions {
                    change_request: None,
                    revision: Some(pipeline.updated_at),
                    actions: vec![AvailableAction::new(
                        SourceAction::RerunWorkflow,
                        "Retry failed jobs",
                        "Retry failed or canceled jobs in this pipeline? Successful jobs are not repeated. This can execute deployments and other workflow side effects.",
                        (!matches!(pipeline.status.as_str(), "failed" | "canceled"))
                            .then_some("GitLab retries failed or canceled pipelines only."),
                    )],
                })
            }
            ActionTarget::ChangeRequest {
                number,
            } => {
                let mr: MergeRequest = read(
                    self.request(
                        config,
                        token,
                        &["projects", &repo.id, "merge_requests", &number.to_string()],
                    )?
                    .query(&[("include_rebase_in_progress", "true")]),
                )
                .await?;
                let reason = if mr.state != "opened" {
                    Some("The merge request is closed.")
                } else if mr.rebase_in_progress {
                    Some("A rebase is already in progress.")
                } else if mr.has_conflicts || mr.detailed_merge_status == "conflict" {
                    Some("Resolve merge conflicts in the provider first.")
                } else if matches!(
                    mr.detailed_merge_status.as_str(),
                    "checking"
                        | "unchecked"
                        | "preparing"
                        | "cannot_be_merged"
                        | "cannot_be_merged_recheck"
                ) {
                    Some("GitLab has not confirmed mergeability yet.")
                } else {
                    None
                };
                Ok(ActionOptions {
                    change_request: None,
                    revision: Some(mr.sha),
                    actions: vec![
                        AvailableAction::new(
                            SourceAction::UpdateBranch,
                            "Rebase branch",
                            "Rebase the source branch onto the latest target branch? This rewrites source-branch commits; it does not merge or approve the merge request.",
                            reason,
                        ),
                        AvailableAction::new(
                            SourceAction::MergeChangeRequest,
                            "Merge PR",
                            "Merge this merge request into its target branch using the project's merge settings? This may trigger deployments. No auto-merge or source-branch deletion will be requested.",
                            reason.or_else(|| {
                                if mr.draft != Some(false) {
                                    Some("Draft status must be confirmed as ready for review.")
                                } else if mr.detailed_merge_status != "mergeable" {
                                    Some("GitLab reports outstanding merge requirements.")
                                } else {
                                    None
                                }
                            }),
                        ),
                    ],
                })
            }
        }
    }

    pub(super) async fn perform_action(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        target: &ActionTarget,
        action: SourceAction,
        revision: Option<&str>,
    ) -> Result<(), ActionFailure> {
        match (target, action) {
            (
                ActionTarget::ChangeRequest {
                    number,
                },
                SourceAction::MergeChangeRequest,
            ) => {
                let response = write_response(self.request(config, token, &["projects", &repo.id, "merge_requests", &number.to_string(), "merge"] )?, Method::PUT, Some(json!({"sha": revision.ok_or(ActionFailure::Conflict)?, "auto_merge": false, "should_remove_source_branch": false}))).await?;
                #[derive(Deserialize)]
                struct ResultBody {
                    state: String,
                }
                let result: ResultBody = response
                    .json()
                    .await
                    .map_err(|_| ActionFailure::OutcomeUnknown)?;
                if result.state == "merged" {
                    Ok(())
                } else {
                    Err(ActionFailure::OutcomeUnknown)
                }
            }
            (
                ActionTarget::WorkflowRun {
                    run_id,
                },
                SourceAction::RerunWorkflow,
            ) => {
                write(
                    self.request(
                        config,
                        token,
                        &["projects", &repo.id, "pipelines", run_id, "retry"],
                    )?,
                    Method::POST,
                    None,
                )
                .await
            }
            (
                ActionTarget::ChangeRequest {
                    number,
                },
                SourceAction::UpdateBranch,
            ) => {
                write(
                    self.request(
                        config,
                        token,
                        &[
                            "projects",
                            &repo.id,
                            "merge_requests",
                            &number.to_string(),
                            "rebase",
                        ],
                    )?,
                    Method::PUT,
                    None,
                )
                .await
            }
            _ => Err(ActionFailure::Unsupported),
        }
    }
}
