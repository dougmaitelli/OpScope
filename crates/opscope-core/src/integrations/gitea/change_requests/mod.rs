use super::*;
use crate::domain::{
    ChangeRequest, ChangeRequestCheck, ChangeRequestCheckStatus as Check, ChangeRequestCommit,
    ChangeRequestDetails, ChangeRequestMergeStatus as Merge, ChangeRequestReview,
    ChangeRequestReviewStatus as Review, ChangeRequestState, Relationships,
};

mod models;
use models::*;

struct Signals {
    reviews: Vec<ChangeRequestReview>,
    checks: Vec<ChangeRequestCheck>,
    check_status: Check,
    relationships: Relationships,
}

impl GiteaClient {
    pub(super) async fn pull_requests(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
    ) -> Result<Option<Vec<ChangeRequest>>, Failure> {
        let requests: Vec<PullRequest> = self
            .pages(
                config,
                token,
                &["repos", &repo.owner, &repo.name, "pulls"],
                &[("state", "open"), ("sort", "recentupdate")],
            )
            .await?;
        let mut summaries = Vec::new();
        for request in requests {
            let signals = self.pull_signals(config, token, repo, &request).await?;
            let mut summary = request.summary(&signals.reviews, signals.check_status)?;
            summary.relationships = signals.relationships;
            summaries.push(summary);
        }
        Ok(Some(summaries))
    }

    async fn pull_signals(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        request: &PullRequest,
    ) -> Result<Signals, Failure> {
        let reviews = self
            .pages(
                config,
                token,
                &[
                    "repos",
                    &repo.owner,
                    &repo.name,
                    "pulls",
                    &request.number.to_string(),
                    "reviews",
                ],
                &[],
            )
            .await?;
        let relationships = request.relationships(&reviews);
        let reviews = current_reviews(
            reviews,
            request.requested_reviewers.as_deref().unwrap_or_default(),
        );
        let mut checks = Vec::new();
        let mut check_status = Check::Unknown;
        if let Some(head) = &request.head {
            for page in 1..=10 {
                let data: Option<CombinedStatus> = http::optional_json(
                    self.request(
                        config,
                        token,
                        &[
                            "repos",
                            &repo.owner,
                            &repo.name,
                            "commits",
                            &head.sha,
                            "status",
                        ],
                    )?
                    .query(&[("limit", http::PAGE_SIZE), ("page", page)]),
                )
                .await?;
                let Some(data) = data else {
                    check_status = Check::Unknown;
                    break;
                };
                if page == 1 {
                    // Use Gitea's combined state, not a local reconstruction
                    // from the paginated check details.
                    check_status = data.check_status();
                }
                if data.statuses.is_empty() && checks.len() < data.total_count {
                    return Err(Failure::UnexpectedResponse);
                }
                checks.extend(data.statuses.into_iter().map(CommitStatus::into_check));
                if checks.len() >= data.total_count {
                    break;
                }
                if page == 10 {
                    return Err(Failure::UnexpectedResponse);
                }
            }
        }
        Ok(Signals {
            reviews,
            checks,
            check_status,
            relationships,
        })
    }

    pub(super) async fn pull_details(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Result<Option<ChangeRequestDetails>, Failure> {
        let request: Option<PullRequest> = http::optional_json(self.request(
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
        let Some(request) = request else {
            return Ok(None);
        };
        let Signals {
            reviews,
            mut checks,
            check_status,
            relationships,
        } = self.pull_signals(config, token, repo, &request).await?;
        let mut summary = request.summary(&reviews, check_status)?;
        summary.relationships = relationships;
        let latest_commit = if let Some(head) = &request.head {
            self.link_pull_runs(config, token, repo, &head.sha, &mut checks)
                .await?;
            http::optional_json::<Commit>(self.request(
                config,
                token,
                &[
                    "repos",
                    &repo.owner,
                    &repo.name,
                    "git",
                    "commits",
                    &head.sha,
                ],
            )?)
            .await?
            .map(|commit| ChangeRequestCommit {
                sha: commit.sha,
                title: commit
                    .commit
                    .message
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
                author: Some(commit.commit.author.name),
                committed_at: commit.commit.committer.date,
            })
        } else {
            None
        };
        Ok(Some(ChangeRequestDetails {
            change_request: summary,
            body: request.body,
            labels: request
                .labels
                .unwrap_or_default()
                .into_iter()
                .map(|label| label.name)
                .collect(),
            reviews,
            checks,
            latest_commit,
        }))
    }

    async fn link_pull_runs(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        sha: &str,
        checks: &mut [ChangeRequestCheck],
    ) -> Result<(), Failure> {
        let prefix = format!("{}/actions/runs/", repo.web_url.trim_end_matches('/'));
        if !checks.iter().any(|check| {
            check
                .web_url
                .as_ref()
                .is_some_and(|url| url.starts_with(&prefix))
        }) {
            return Ok(());
        }
        for page in 1..=http::RUN_PAGES {
            let response = http::send(
                self.request(
                    config,
                    token,
                    &["repos", &repo.owner, &repo.name, "actions", "runs"],
                )?
                .query(&[("head_sha", sha)])
                .query(&[("limit", http::PAGE_SIZE), ("page", page)]),
            )
            .await?;
            if matches!(
                response.status(),
                reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND
            ) {
                break;
            }
            if !response.status().is_success() {
                return Err(http::status_failure(response.status()));
            }
            let runs: Runs = http::decode(response).await?;
            for run in &runs.workflow_runs {
                if run.head_sha != sha {
                    continue;
                }
                for check in checks.iter_mut() {
                    if check.web_url.as_ref().is_some_and(|url| {
                        url == &run.html_url
                            || url.starts_with(&format!(
                                "{}/jobs/",
                                run.html_url.trim_end_matches('/')
                            ))
                    }) {
                        check.workflow_run_id = Some(run.id.to_string());
                    }
                }
            }
            if runs.workflow_runs.is_empty() || page * http::PAGE_SIZE >= runs.total_count {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
