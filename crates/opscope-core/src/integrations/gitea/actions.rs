use super::*;
#[cfg(test)]
mod tests;
use crate::application::{
    ActionFailure, ActionOptions, ActionTarget, AvailableAction, SourceAction,
};
use crate::integrations::http::actions::{read, write};
use reqwest::Method;
use serde_json::json;

#[derive(Deserialize)]
struct Pull {
    state: String,
    merged: bool,
    mergeable: Option<bool>,
    draft: Option<bool>,
    head: Head,
}
#[derive(Deserialize)]
struct Head {
    sha: String,
}
#[derive(Deserialize)]
struct Run {
    status: String,
    run_attempt: u64,
}

impl GiteaClient {
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
                let run: Run = read(self.request(
                    config,
                    token,
                    &["repos", &repo.owner, &repo.name, "actions", "runs", run_id],
                )?)
                .await?;
                let completed = matches!(
                    run.status.as_str(),
                    "completed" | "success" | "failure" | "cancelled" | "skipped"
                );
                Ok(ActionOptions {
                    change_request: None,
                    revision: Some(run.run_attempt.to_string()),
                    actions: vec![AvailableAction::new(
                        SourceAction::RerunWorkflow,
                        "Re-run workflow",
                        "Re-run all jobs for this workflow run? This can execute deployments and other workflow side effects.",
                        (!completed).then_some("Wait for the current run to finish."),
                    )],
                })
            }
            ActionTarget::ChangeRequest {
                number,
            } => {
                let pull: Pull = read(self.request(
                    config,
                    token,
                    &[
                        "repos",
                        &repo.owner,
                        &repo.name,
                        "pulls",
                        &number.to_string(),
                    ],
                )?)
                .await?;
                let reason = if pull.state != "open" || pull.merged {
                    Some("The pull request is closed.")
                } else if pull.mergeable != Some(true) {
                    Some("Mergeability is unknown or there are conflicts.")
                } else {
                    None
                };
                Ok(ActionOptions {
                    change_request: None,
                    revision: Some(pull.head.sha),
                    actions: vec![
                        AvailableAction::new(
                            SourceAction::UpdateBranch,
                            "Update branch",
                            "Merge the latest target branch into this PR branch? This does not merge or approve the PR.",
                            reason,
                        ),
                        AvailableAction::new(
                            SourceAction::MergeChangeRequest,
                            "Merge PR",
                            "Merge this PR into its target branch using the repository's default merge style (squash if unset)? This may trigger deployments. No force-merge, auto-merge, or source-branch deletion will be requested.",
                            reason.or((pull.draft != Some(false))
                                .then_some("Draft status must be confirmed as ready for review.")),
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
                #[derive(Deserialize)]
                struct Settings {
                    default_merge_style: Option<String>,
                }
                let settings: Settings =
                    read(self.request(config, token, &["repos", &repo.owner, &repo.name])?).await?;
                let style = settings
                    .default_merge_style
                    .as_deref()
                    .filter(|style| !style.is_empty())
                    .unwrap_or("squash");
                if !matches!(
                    style,
                    "merge" | "squash" | "rebase" | "rebase-merge" | "fast-forward-only"
                ) {
                    return Err(ActionFailure::Unsupported);
                }
                write(self.request(config, token, &["repos", &repo.owner, &repo.name, "pulls", &number.to_string(), "merge"] )?, Method::POST, Some(json!({"do": style, "head_commit_id": revision.ok_or(ActionFailure::Conflict)?, "force_merge": false, "merge_when_checks_succeed": false, "delete_branch_after_merge": false}))).await
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
                        &[
                            "repos",
                            &repo.owner,
                            &repo.name,
                            "actions",
                            "runs",
                            run_id,
                            "rerun",
                        ],
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
                            "repos",
                            &repo.owner,
                            &repo.name,
                            "pulls",
                            &number.to_string(),
                            "update",
                        ],
                    )?
                    .query(&[("style", "merge")]),
                    Method::POST,
                    None,
                )
                .await
            }
            _ => Err(ActionFailure::Unsupported),
        }
    }
}
