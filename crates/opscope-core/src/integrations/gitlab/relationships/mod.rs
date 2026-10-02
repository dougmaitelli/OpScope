//! GitLab token-owner matching, isolated from normal monitoring failures.
use super::*;
use crate::domain::{AccountSet, Relationships};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Deserialize)]
pub(super) struct Identity {
    pub id: u64,
    email: Option<String>,
    public_email: Option<String>,
    commit_email: Option<String>,
}

impl GitLabClient {
    pub(super) async fn relationship_viewer(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
    ) -> Option<Identity> {
        let result = http::json(self.request(config, token, &["user"]).ok()?).await;
        if result.is_err() {
            eprintln!("GitLab personal relevance unavailable; token owner could not be verified");
        }
        result.ok()
    }

    pub(super) async fn review_history(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Option<AccountSet> {
        let notes: Vec<Value> = self
            .pages(
                config,
                token,
                &[
                    "projects",
                    &repo.id,
                    "merge_requests",
                    &number.to_string(),
                    "notes",
                ],
                &[],
                10,
                false,
            )
            .await
            .ok()?;
        let relevant: Vec<_> = notes
            .iter()
            .filter(|note| note["system"] == false)
            .collect();
        let ids: Vec<_> = relevant
            .iter()
            .filter_map(|note| note.pointer("/author/id").and_then(Value::as_u64))
            .map(|id| id.to_string())
            .collect();
        let complete =
            ids.len() == relevant.len() && notes.iter().all(|note| note["system"].is_boolean());
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
        let owner = self.relationship_viewer(config, token).await;
        let mut identities = Vec::new();
        if let Some(owner) = owner {
            let emails = self
                .pages::<Value>(config, token, &["user", "emails"], &[], 10, false)
                .await
                .ok();
            let mut known_emails = [&owner.email, &owner.public_email, &owner.commit_email]
                .into_iter()
                .filter_map(|v| v.clone())
                .filter(|v| !v.is_empty())
                .collect::<Vec<_>>();
            for email in emails.as_ref().into_iter().flatten() {
                if email["confirmed_at"].is_string()
                    && let Some(email) = email["email"].as_str()
                {
                    known_emails.push(email.into());
                }
            }
            identities.push(crate::domain::AccountEmails {
                account_id: owner.id.to_string(),
                emails: known_emails,
                complete: emails.is_some(),
            });
        }
        let mut commits = HashMap::new();
        for run in runs {
            if !commits.contains_key(&run.commit_sha) {
                let mut relevance = self
                    .commit_relationships(config, token, repo, &run.commit_sha)
                    .await;
                relevance.identities = identities.clone();
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
            &["projects", &repo.id, "repository", "commits", sha],
        ) {
            Ok(request) => http::json::<Value>(request).await.ok(),
            Err(_) => None,
        };
        let pulls = self
            .pages::<Value>(
                config,
                token,
                &[
                    "projects",
                    &repo.id,
                    "repository",
                    "commits",
                    sha,
                    "merge_requests",
                ],
                &[],
                10,
                false,
            )
            .await
            .ok();
        let authors: Vec<_> = pulls
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|v| v.pointer("/author/id").and_then(Value::as_u64))
            .map(|id| id.to_string())
            .collect();
        Relationships {
            linked_change_requests: pulls
                .iter()
                .flatten()
                .map(|pull| crate::domain::ChangeRequestReference {
                    id: pull["id"].as_u64().map(|id| id.to_string()),
                    number: pull["iid"].as_u64(),
                    web_url: pull["web_url"].as_str().map(str::to_owned),
                    author_id: pull
                        .pointer("/author/id")
                        .and_then(Value::as_u64)
                        .map(|id| id.to_string()),
                })
                .collect(),
            commit_author_email: commit
                .as_ref()
                .and_then(|v| v["author_email"].as_str())
                .filter(|v| !v.is_empty())
                .map(str::to_owned),
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
