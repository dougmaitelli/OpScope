//! Token-owner relationships, never inferred from the run's triggering actor.
use super::*;
use crate::domain::{AccountSet, Relationships};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Deserialize)]
pub(super) struct Identity {
    pub id: u64,
}

impl GiteaClient {
    pub(super) async fn relationship_viewer(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Option<Identity> {
        let result = http::json(self.request(config, token, &["user"]).ok()?).await;
        if result.is_err() {
            eprintln!("Gitea personal relevance unavailable; token owner could not be verified");
        }
        result.ok()
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
            &["repos", &repo.owner, &repo.name, "git", "commits", sha],
        ) {
            Ok(request) => http::json::<Value>(request).await.ok(),
            Err(_) => None,
        };

        let pulls = match self.request(
            config,
            token,
            &["repos", &repo.owner, &repo.name, "commits", sha, "pull"],
        ) {
            Ok(request) => http::optional_json::<Value>(request)
                .await
                .ok()
                .flatten()
                .map(|pull| vec![pull]),
            Err(_) => None,
        };
        let author = commit
            .as_ref()
            .and_then(|v| v.pointer("/author/id"))
            .and_then(|v| v.as_u64().map(|id| id.to_string()));
        let authors: Vec<_> = pulls
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|v| v.pointer("/user/id"))
            .filter_map(|v| v.as_u64().map(|id| id.to_string()))
            .collect();
        Relationships {
            linked_change_requests: pulls
                .iter()
                .flatten()
                .map(|pull| crate::domain::ChangeRequestReference {
                    id: pull["id"].as_u64().map(|id| id.to_string()),
                    number: pull["number"].as_u64(),
                    web_url: pull["html_url"].as_str().map(str::to_owned),
                    author_id: pull
                        .pointer("/user/id")
                        .and_then(Value::as_u64)
                        .map(|id| id.to_string()),
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
