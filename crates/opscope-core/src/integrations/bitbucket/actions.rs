use super::*;
mod merge;
#[cfg(test)]
mod tests;
use crate::application::{
    ActionFailure, ActionOptions, ActionTarget, AvailableAction, SourceAction,
};
use crate::integrations::http::actions::{read, write_response};
use reqwest::Method;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Pipeline {
    state: State,
    target: Value,
    completed_on: Option<String>,
}
#[derive(Deserialize)]
struct State {
    name: String,
}

fn repeat_target(target: &Value) -> Option<Value> {
    // Pin the original commit, never silently execute the latest branch HEAD.
    let kind = target.get("type")?.as_str()?;
    if !matches!(
        kind,
        "pipeline_ref_target" | "pipeline_commit_target" | "pipeline_pullrequest_target"
    ) {
        return None;
    }
    let hash = target.pointer("/commit/hash")?.as_str()?;
    if hash.is_empty() {
        return None;
    }
    let mut result = json!({"type": kind, "commit": {"type": "commit", "hash": hash}});
    if kind == "pipeline_ref_target" {
        result["ref_name"] = Value::String(target.get("ref_name")?.as_str()?.to_owned());
        result["ref_type"] = Value::String(target.get("ref_type")?.as_str()?.to_owned());
    }
    if kind == "pipeline_pullrequest_target" {
        result["source"] = Value::String(target.get("source")?.as_str()?.to_owned());
        result["destination"] = Value::String(target.get("destination")?.as_str()?.to_owned());
        let destination_hash = target.pointer("/destination_commit/hash")?.as_str()?;
        if destination_hash.is_empty() {
            return None;
        }
        result["destination_commit"] = json!({"hash": destination_hash});
        let id = target.pointer("/pullrequest/id")?;
        if id.as_u64().is_none() && !id.as_str().is_some_and(|id| id.parse::<u64>().is_ok()) {
            return None;
        }
        result["pullrequest"] = json!({"id": id});
    }
    if let Some(selector) = target.get("selector") {
        result["selector"] = selector.clone();
    }
    Some(result)
}

impl BitbucketClient {
    pub(super) async fn actions(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        target: &ActionTarget,
    ) -> Result<ActionOptions, ActionFailure> {
        if let ActionTarget::ChangeRequest {
            number,
        } = target
        {
            return self.merge_options(config, token, repo, *number).await;
        }
        let ActionTarget::WorkflowRun {
            run_id,
        } = target
        else {
            return Ok(ActionOptions::default());
        };
        let pipeline: Pipeline = read(self.request(
            config,
            token,
            &["repositories", &repo.owner, &repo.name, "pipelines", run_id],
        )?)
        .await?;
        let reason = if pipeline.state.name != "COMPLETED" {
            Some("Wait for the current pipeline to finish.")
        } else if repeat_target(&pipeline.target).is_none() {
            Some(
                "This pipeline target cannot be safely repeated through the Bitbucket API. Use Bitbucket instead.",
            )
        } else {
            None
        };
        Ok(ActionOptions {
            change_request: None,
            revision: pipeline.completed_on,
            actions: vec![AvailableAction::new(
                SourceAction::RerunWorkflow,
                "Run pipeline again",
                "Create a new pipeline for the original commit and selector? Custom run variables are not copied. This can execute deployments and other pipeline side effects.",
                reason,
            )],
        })
    }

    pub(super) async fn perform_action(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        target: &ActionTarget,
        action: SourceAction,
        revision: Option<&str>,
    ) -> Result<Option<ActionTarget>, ActionFailure> {
        if let (
            ActionTarget::ChangeRequest {
                number,
            },
            SourceAction::MergeChangeRequest,
        ) = (target, action)
        {
            return self
                .merge_pull(config, token, repo, *number, revision)
                .await
                .map(|()| None);
        }
        let (
            ActionTarget::WorkflowRun {
                run_id,
            },
            SourceAction::RerunWorkflow,
        ) = (target, action)
        else {
            return Err(ActionFailure::Unsupported);
        };
        let pipeline: Pipeline = read(self.request(
            config,
            token,
            &["repositories", &repo.owner, &repo.name, "pipelines", run_id],
        )?)
        .await?;
        if pipeline.state.name != "COMPLETED" || pipeline.completed_on.as_deref() != revision {
            return Err(ActionFailure::Conflict);
        }
        let target = repeat_target(&pipeline.target).ok_or(ActionFailure::Unsupported)?;
        let response = write_response(
            self.request(
                config,
                token,
                &["repositories", &repo.owner, &repo.name, "pipelines"],
            )?,
            Method::POST,
            Some(json!({"target": target})),
        )
        .await?;
        // A rerun creates a new pipeline. Refresh that pipeline, not the original.
        // An unreadable successful response must not make the mutation retryable.
        let payload = response.json::<Value>().await.ok();
        Ok(payload.and_then(|payload| {
            payload
                .get("uuid")
                .and_then(Value::as_str)
                .map(|run_id| ActionTarget::WorkflowRun {
                    run_id: run_id.to_owned(),
                })
        }))
    }
}
