use super::*;
use crate::integrations::http::actions::write_response;

#[derive(Deserialize)]
struct Pull {
    state: String,
    draft: Option<bool>,
    #[serde(default)]
    queued: bool,
    source: Branch,
    destination: Branch,
}
#[derive(Deserialize)]
struct Branch {
    commit: Commit,
    branch: Option<BranchName>,
}
#[derive(Deserialize)]
struct BranchName {
    name: String,
}
#[derive(Deserialize)]
struct Commit {
    hash: String,
}
#[derive(Deserialize)]
struct Checks {
    values: Vec<Check>,
    size: Option<usize>,
    next: Option<String>,
}
#[derive(Deserialize)]
struct Check {
    #[serde(rename = "type")]
    kind: String,
    status: String,
    blocking: bool,
}

impl BitbucketClient {
    pub(super) async fn merge_options(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Result<ActionOptions, ActionFailure> {
        let pull: Pull = read(self.request(
            config,
            token,
            &[
                "repositories",
                &repo.owner,
                &repo.name,
                "pullrequests",
                &number.to_string(),
            ],
        )?)
        .await?;
        let reason = if pull.state != "OPEN" {
            Some("The pull request is closed.")
        } else if pull.draft != Some(false) {
            Some("Draft status must be confirmed as ready for review.")
        } else if pull.queued {
            Some("The pull request is already queued for merge.")
        } else {
            let checks: Checks = read(self.request(
                config,
                token,
                &[
                    "repositories",
                    &repo.owner,
                    &repo.name,
                    "pullrequests",
                    &number.to_string(),
                    "mergeability",
                    "checks",
                ],
            )?)
            .await?;
            if checks.next.is_some()
                || checks.size.is_some_and(|size| size > checks.values.len())
                || !checks
                    .values
                    .iter()
                    .any(|check| check.kind == "git_mergeability_check" && check.status == "PASSED")
            {
                Some("Bitbucket has not confirmed conflict-free mergeability.")
            } else if checks
                .values
                .iter()
                .any(|check| check.blocking || check.kind == "merge_queue_check")
            {
                Some(
                    "Bitbucket reports outstanding merge requirements or requires a merge queue. Merge in Bitbucket instead.",
                )
            } else {
                None
            }
        };
        Ok(ActionOptions {
            revision: Some(format!(
                "{}:{}",
                pull.source.commit.hash, pull.destination.commit.hash
            )),
            actions: vec![AvailableAction::new(
                SourceAction::MergeChangeRequest,
                "Merge PR",
                "Merge this PR into its target branch using its configured default strategy (squash if unset)? This may trigger deployments. Bitbucket cannot atomically lock the checked commit: a concurrent push may be included. The source branch will be kept.",
                reason,
            )],
        })
    }

    pub(super) async fn merge_pull(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
        revision: Option<&str>,
    ) -> Result<(), ActionFailure> {
        if revision.is_none() {
            return Err(ActionFailure::Conflict);
        }
        let pull: Pull = read(self.request(
            config,
            token,
            &[
                "repositories",
                &repo.owner,
                &repo.name,
                "pullrequests",
                &number.to_string(),
            ],
        )?)
        .await?;
        if pull.state != "OPEN"
            || pull.draft != Some(false)
            || pull.queued
            || Some(format!(
                "{}:{}",
                pull.source.commit.hash, pull.destination.commit.hash
            ))
            .as_deref()
                != revision
        {
            return Err(ActionFailure::Conflict);
        }
        let branch = pull.destination.branch.ok_or(ActionFailure::Conflict)?;
        #[derive(Deserialize)]
        struct Settings {
            default_merge_strategy: Option<String>,
        }
        let settings: Settings = read(self.request(
            config,
            token,
            &[
                "repositories",
                &repo.owner,
                &repo.name,
                "refs",
                "branches",
                &branch.name,
            ],
        )?)
        .await?;
        let strategy = settings
            .default_merge_strategy
            .as_deref()
            .filter(|strategy| !strategy.is_empty())
            .unwrap_or("squash");
        if !matches!(
            strategy,
            "merge_commit"
                | "squash"
                | "fast_forward"
                | "squash_fast_forward"
                | "rebase_fast_forward"
                | "rebase_merge"
        ) {
            return Err(ActionFailure::Unsupported);
        }
        let response = write_response(self.request(config, token, &["repositories", &repo.owner, &repo.name, "pullrequests", &number.to_string(), "merge"] )?, Method::POST, Some(json!({"type":"pullrequest", "merge_strategy":strategy, "close_source_branch":false}))).await?;
        if response.status() == reqwest::StatusCode::ACCEPTED {
            return Ok(());
        }
        #[derive(Deserialize)]
        struct ResultBody {
            state: String,
        }
        let result: ResultBody = response
            .json()
            .await
            .map_err(|_| ActionFailure::OutcomeUnknown)?;
        if result.state == "MERGED" {
            Ok(())
        } else {
            Err(ActionFailure::OutcomeUnknown)
        }
    }
}
