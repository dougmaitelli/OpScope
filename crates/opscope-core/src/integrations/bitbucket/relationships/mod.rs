//! Relationships are matched by account UUID, never display name or trigger actor.
use super::*;
use crate::domain::{AccountSet, Relationships};
use serde_json::Value;
use std::collections::HashMap;

impl BitbucketClient {
    pub(super) async fn review_history(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Option<AccountSet> {
        let events: Vec<Value> = self
            .pages(
                config,
                token,
                &[
                    "repositories",
                    &repo.owner,
                    &repo.id,
                    "pullrequests",
                    &number.to_string(),
                    "activity",
                ],
                &[],
                10,
                false,
            )
            .await
            .ok()?;
        let relevant: Vec<_> = events
            .iter()
            .flat_map(|event| {
                ["approval", "changes_requested", "comment"]
                    .into_iter()
                    .map(move |kind| &event[kind])
                    .filter(|event| event.is_object() && event["deleted"] != true)
            })
            .collect();
        let ids: Vec<_> = relevant
            .iter()
            .filter_map(|event| event.pointer("/user/uuid").and_then(Value::as_str))
            .map(str::to_owned)
            .collect();
        let complete = ids.len() == relevant.len();
        Some(AccountSet::new(ids, complete))
    }

    pub(super) async fn run_relationships(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        runs: &mut [WorkflowRun],
    ) {
        if runs.is_empty() {
            return;
        }

        let mut commits = HashMap::new();
        for run in runs {
            if !commits.contains_key(&run.commit_sha) {
                let relevance = self
                    .commit_relationships(config, token, repo, &run.commit_sha)
                    .await;
                commits.insert(run.commit_sha.clone(), relevance);
            }
            run.relationships = commits[&run.commit_sha].clone();
        }
    }

    async fn commit_relationships(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        sha: &str,
    ) -> Relationships {
        if sha.is_empty() {
            return Relationships::default();
        }
        let commit = match self.request(
            config,
            token,
            &["repositories", &repo.owner, &repo.id, "commit", sha],
        ) {
            Ok(request) => http::json::<Value>(request).await.ok(),
            Err(_) => None,
        };
        let pulls = self
            .pages::<Value>(
                config,
                token,
                &[
                    "repositories",
                    &repo.owner,
                    &repo.id,
                    "commit",
                    sha,
                    "pullrequests",
                ],
                &[],
                10,
                false,
            )
            .await
            .ok();
        let author = commit
            .as_ref()
            .and_then(|v| v.pointer("/author/user/uuid"))
            .and_then(|v| v.as_str().map(str::to_owned));
        let authors: Vec<_> = pulls
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|v| v.pointer("/author/uuid"))
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        Relationships {
            linked_change_requests: pulls
                .iter()
                .flatten()
                .map(|pull| crate::domain::ChangeRequestReference {
                    id: pull["id"].as_u64().map(|id| id.to_string()),
                    number: pull["id"].as_u64(),
                    web_url: pull
                        .pointer("/links/html/href")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    author_id: pull
                        .pointer("/author/uuid")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                })
                .collect(),
            commit_authors: AccountSet::new(author.clone(), author.is_some()),
            change_request_authors: AccountSet::new(
                authors.clone(),
                pulls
                    .as_ref()
                    .is_some_and(|pulls| pulls.len() == authors.len()),
            ),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests;
