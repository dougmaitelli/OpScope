use super::*;
use crate::domain::{
    ChangeRequest, ChangeRequestCheck, ChangeRequestCheckStatus as Check, ChangeRequestCommit,
    ChangeRequestDetails, ChangeRequestMergeStatus as Merge, ChangeRequestReview,
    ChangeRequestReviewStatus as Review, ChangeRequestState, Relationships,
};
mod aggregation;
use aggregation as aggregate;

mod models;
use models::*;

impl BitbucketClient {
    pub(super) async fn pull_requests(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
    ) -> Result<Option<Vec<ChangeRequest>>, Failure> {
        let requests: Vec<PullId> = self
            .pages(
                config,
                token,
                &["repositories", &repo.owner, &repo.id, "pullrequests"],
                &[("state", "OPEN"), ("sort", "-updated_on")],
                10,
                false,
            )
            .await?;
        let mut summaries = Vec::new();
        for request in requests {
            // Participants/reviewers are not reliably populated by list endpoints.
            if let Some((request, checks)) =
                self.pull_signals(config, token, repo, request.id).await?
            {
                let mut summary = request.summary(&checks)?;
                summary.relationships =
                    self.pull_relationships(config, token, repo, &request).await;
                summaries.push(summary);
            }
        }
        Ok(Some(summaries))
    }

    async fn pull_signals(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Result<Option<(PullRequest, Vec<ChangeRequestCheck>)>, Failure> {
        let number = number.to_string();
        let Some(request) = http::optional_json::<PullRequest>(self.request(
            config,
            token,
            &[
                "repositories",
                &repo.owner,
                &repo.id,
                "pullrequests",
                &number,
            ],
        )?)
        .await?
        else {
            return Ok(None);
        };
        let statuses: Vec<Status> = self
            .pages(
                config,
                token,
                &[
                    "repositories",
                    &repo.owner,
                    &repo.id,
                    "pullrequests",
                    &number,
                    "statuses",
                ],
                &[],
                10,
                false,
            )
            .await?;
        Ok(Some((
            request,
            statuses.into_iter().map(Status::into_check).collect(),
        )))
    }

    pub(super) async fn pull_details(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Result<Option<ChangeRequestDetails>, Failure> {
        let Some((request, mut checks)) = self.pull_signals(config, token, repo, number).await?
        else {
            return Ok(None);
        };
        let mut summary = request.summary(&checks)?;
        summary.relationships = self.pull_relationships(config, token, repo, &request).await;
        let reviews = request.reviews();
        let latest_commit = if let Some(source) = &request.source {
            self.link_pull_pipelines(config, token, repo, &source.commit.hash, &mut checks)
                .await?;
            let page: Page<Commit> = http::json(
                self.request(
                    config,
                    token,
                    &[
                        "repositories",
                        &repo.owner,
                        &repo.id,
                        "pullrequests",
                        &number.to_string(),
                        "commits",
                    ],
                )?
                .query(&[("pagelen", 1)]),
            )
            .await?;
            page.values
                .into_iter()
                .find(|commit| commit.hash == source.commit.hash)
                .map(|commit| ChangeRequestCommit {
                    sha: commit.hash,
                    title: commit.message.lines().next().unwrap_or_default().to_owned(),
                    author: commit.author.and_then(|author| {
                        author.user.map(|user| user.display_name).or(author.raw)
                    }),
                    committed_at: commit.date,
                })
        } else {
            None
        };
        Ok(Some(ChangeRequestDetails {
            change_request: summary,
            body: request
                .description
                .or_else(|| request.summary.and_then(|summary| summary.raw)),
            labels: Vec::new(),
            reviews,
            checks,
            latest_commit,
        }))
    }

    async fn pull_relationships(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        request: &PullRequest,
    ) -> Relationships {
        let mut result = request.relationships();
        if let Some(history) = self.review_history(config, token, repo, request.id).await {
            result.reviewers.ids.extend(history.ids);
            result.reviewers.complete = history.complete && request.participants.is_some();
        }
        result
    }

    async fn link_pull_pipelines(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        sha: &str,
        checks: &mut [ChangeRequestCheck],
    ) -> Result<(), Failure> {
        let prefix = repo.web_url.trim_end_matches('/');
        let candidates = checks
            .iter()
            .filter_map(|check| check.web_url.as_deref())
            .any(|url| {
                url.starts_with(&format!("{prefix}/pipelines/results/"))
                    || url.starts_with(&format!("{prefix}/addon/pipelines/home#!/results/"))
            });
        if !candidates {
            return Ok(());
        }
        // API-derived SHAs are still untrusted input to Bitbucket's query DSL.
        if sha.is_empty() || !sha.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(Failure::UnexpectedResponse);
        }
        let query = format!("target.commit.hash=\"{sha}\"");
        let runs: Vec<PipelineLink> = match self
            .pages(
                config,
                token,
                &["repositories", &repo.owner, &repo.id, "pipelines"],
                &[("q", &query), ("sort", "-created_on")],
                http::RUN_PAGES,
                true,
            )
            .await
        {
            Ok(runs) => runs,
            Err(Failure::PermissionDenied) => return Ok(()),
            Err(error) => return Err(error),
        };
        for run in runs {
            if run.target.commit.hash != sha {
                continue;
            }
            let current = format!("{prefix}/pipelines/results/{}", run.build_number);
            let legacy = format!(
                "{prefix}/addon/pipelines/home#!/results/{}",
                run.build_number
            );
            for check in checks.iter_mut() {
                if check.web_url.as_deref().is_some_and(|url| {
                    [current.as_str(), legacy.as_str()].iter().any(|expected| {
                        url == *expected
                            || url.strip_prefix(expected).is_some_and(|suffix| {
                                suffix.starts_with('/')
                                    || suffix.starts_with('?')
                                    || suffix.starts_with('#')
                            })
                    })
                }) {
                    check.workflow_run_id = Some(run.uuid.clone());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
