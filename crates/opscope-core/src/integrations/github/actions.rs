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
struct Pull {
    state: String,
    merged: bool,
    mergeable: Option<bool>,
    draft: Option<bool>,
    mergeable_state: Option<String>,
    head: Head,
    user: Author,
}
#[derive(Deserialize)]
struct Head {
    sha: String,
}
#[derive(Deserialize)]
struct Author {
    login: String,
    #[serde(rename = "type")]
    kind: String,
}
#[derive(Deserialize)]
struct Run {
    status: String,
    run_attempt: u64,
    event: Option<String>,
    path: Option<String>,
}

impl Run {
    fn rerun_disabled_reason(&self) -> Option<&'static str> {
        if self.event.as_deref() == Some("dynamic")
            || self
                .path
                .as_deref()
                .is_some_and(|path| path.starts_with("dynamic/"))
        {
            Some(
                "GitHub-managed workflows cannot be re-run here. Open GitHub to use the feature's own controls.",
            )
        } else if self.status != "completed" {
            Some("Wait for the current run to finish.")
        } else {
            None
        }
    }
}

impl GitHubClient {
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
                Ok(ActionOptions {
                    change_request: None,
                    revision: Some(run.run_attempt.to_string()),
                    actions: vec![AvailableAction::new(
                        SourceAction::RerunWorkflow,
                        "Re-run workflow",
                        "Re-run all jobs for this workflow run at its original commit? This can execute deployments and other workflow side effects.",
                        run.rerun_disabled_reason(),
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
                let closed = pull.state != "open" || pull.merged;
                let reason = if closed {
                    Some("The pull request is closed.")
                } else if pull.mergeable == Some(false) {
                    Some("Resolve merge conflicts in the provider first.")
                } else if pull.mergeable.is_none() {
                    Some("GitHub is still checking mergeability.")
                } else {
                    None
                };
                let mut actions = vec![AvailableAction::new(
                    SourceAction::UpdateBranch,
                    "Update branch",
                    "Merge the latest target branch into this PR branch? This does not merge or approve the PR.",
                    reason,
                )];
                let merge_reason = reason.or_else(|| {
                    if pull.draft != Some(false) {
                        Some("Draft status must be confirmed as ready for review.")
                    } else if pull.mergeable_state.as_deref() != Some("clean") {
                        Some("GitHub has not confirmed all merge requirements are satisfied.")
                    } else {
                        None
                    }
                });
                actions.push(AvailableAction::new(SourceAction::MergeChangeRequest, "Merge PR", "Merge this PR into its target branch using squash? This changes the target branch and may trigger deployments. Repository policies apply; no auto-merge or bypass will be requested.", merge_reason));
                if pull.user.kind == "Bot"
                    && matches!(pull.user.login.as_str(), "dependabot[bot]" | "dependabot")
                {
                    actions.push(AvailableAction::new(SourceAction::DependabotRebase, "Dependabot: rebase", "Post @dependabot rebase on this PR? Dependabot will process the request asynchronously.", closed.then_some("The pull request is closed.")));
                    actions.push(AvailableAction::new(SourceAction::DependabotRecreate, "Dependabot: recreate", "Post @dependabot recreate? This can overwrite manual edits to the PR branch. Dependabot will process the request asynchronously.", closed.then_some("The pull request is closed.")));
                }
                Ok(ActionOptions {
                    change_request: None,
                    actions,
                    revision: Some(pull.head.sha),
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
                let response = write_response(self.request(config, token, &["repos", &repo.owner, &repo.name, "pulls", &number.to_string(), "merge"] )?, Method::PUT, Some(json!({"sha": revision.ok_or(ActionFailure::Conflict)?, "merge_method": "squash"}))).await?;
                #[derive(Deserialize)]
                struct ResultBody {
                    merged: bool,
                }
                let result: ResultBody = response
                    .json()
                    .await
                    .map_err(|_| ActionFailure::OutcomeUnknown)?;
                if result.merged {
                    Ok(())
                } else {
                    Err(ActionFailure::Conflict)
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
                            "update-branch",
                        ],
                    )?,
                    Method::PUT,
                    Some(json!({"expected_head_sha": revision.ok_or(ActionFailure::Conflict)?})),
                )
                .await
            }
            (
                ActionTarget::ChangeRequest {
                    number,
                },
                SourceAction::DependabotRebase | SourceAction::DependabotRecreate,
            ) => {
                let command = if action == SourceAction::DependabotRebase {
                    "@dependabot rebase"
                } else {
                    "@dependabot recreate"
                };
                write(
                    self.request(
                        config,
                        token,
                        &[
                            "repos",
                            &repo.owner,
                            &repo.name,
                            "issues",
                            &number.to_string(),
                            "comments",
                        ],
                    )?,
                    Method::POST,
                    Some(json!({"body": command})),
                )
                .await
            }
            _ => Err(ActionFailure::Unsupported),
        }
    }
}
