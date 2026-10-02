use super::*;
use crate::domain::{
    ChangeRequest, ChangeRequestCheck, ChangeRequestCheckStatus as Check, ChangeRequestCommit,
    ChangeRequestDetails, ChangeRequestMergeStatus as Merge, ChangeRequestReview,
    ChangeRequestReviewStatus as Review, ChangeRequestState, Relationships,
};

mod models;
use models::*;

impl GitLabClient {
    pub(super) async fn pull_requests(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
    ) -> Result<Option<Vec<ChangeRequest>>, Failure> {
        let requests: Vec<MergeRequest> = self
            .pages(
                config,
                token,
                &["projects", &repo.id, "merge_requests"],
                &[
                    ("state", "opened"),
                    ("scope", "all"),
                    ("order_by", "updated_at"),
                    ("sort", "desc"),
                ],
                10,
                false,
            )
            .await?;
        let mut summaries = Vec::new();
        for request in requests {
            // GitLab list responses omit head_pipeline. Fetch the current signal
            // data, but leave descriptions/commits to the on-demand details path.
            if let Some((request, approvals)) =
                self.pull_signals(config, token, repo, request.iid).await?
            {
                let mut summary = request.summary(approvals.as_ref())?;
                summary.relationships = self
                    .pull_relationships(config, token, repo, &request, approvals.as_ref())
                    .await;
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
    ) -> Result<Option<(MergeRequest, Option<Approvals>)>, Failure> {
        let number = number.to_string();
        let Some(request) = http::optional_json::<MergeRequest>(self.request(
            config,
            token,
            &["projects", &repo.id, "merge_requests", &number],
        )?)
        .await?
        else {
            return Ok(None);
        };
        let response = http::send(self.request(
            config,
            token,
            &["projects", &repo.id, "merge_requests", &number, "approvals"],
        )?)
        .await?;
        // Approval APIs can be unavailable for this edition or account. That is
        // unknown review information, not evidence that the request is approved.
        let approvals = match response.status() {
            reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::NOT_FOUND => None,
            status if status.is_success() => Some(http::decode(response).await?),
            status => return Err(http::status_failure(status)),
        };
        Ok(Some((request, approvals)))
    }

    async fn pull_relationships(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        request: &MergeRequest,
        approvals: Option<&Approvals>,
    ) -> Relationships {
        let mut result = request.relationships(approvals);
        if let Some(history) = self.review_history(config, token, repo, request.iid).await {
            result.reviewers.ids.extend(history.ids);
            result.reviewers.complete = history.complete && result.completed_reviewers.complete;
        }
        result
    }

    pub(super) async fn pull_details(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Result<Option<ChangeRequestDetails>, Failure> {
        let Some((request, approvals)) = self.pull_signals(config, token, repo, number).await?
        else {
            return Ok(None);
        };
        let mut summary = request.summary(approvals.as_ref())?;
        summary.relationships = self
            .pull_relationships(config, token, repo, &request, approvals.as_ref())
            .await;
        let mut reviews: Vec<_> = approvals
            .as_ref()
            .map(|approval| {
                approval
                    .approved_by
                    .iter()
                    .map(|entry| ChangeRequestReview {
                        reviewer: Some(entry.user.username.clone()),
                        status: Review::Approved,
                        submitted_at: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        for reviewer in request.reviewers.iter().flatten() {
            if !reviews
                .iter()
                .any(|review| review.reviewer.as_deref() == Some(&reviewer.username))
            {
                reviews.push(ChangeRequestReview {
                    reviewer: Some(reviewer.username.clone()),
                    status: Review::ReviewRequired,
                    submitted_at: None,
                });
            }
        }
        let checks = request
            .head_pipeline
            .as_ref()
            .map(|pipeline| {
                vec![ChangeRequestCheck {
                    name: pipeline.name.clone().unwrap_or_else(|| "Pipeline".into()),
                    status: pipeline_status(&pipeline.status),
                    web_url: Some(pipeline.web_url.clone()),
                    // Fork pipelines may belong to a different project. The run dialog
                    // resolves IDs in the selected repository, so only link proven matches.
                    workflow_run_id: pipeline
                        .project_id
                        .filter(|id| id.to_string() == repo.id)
                        .map(|_| pipeline.id.to_string()),
                }]
            })
            .unwrap_or_default();
        let latest_commit = if let (Some(project), Some(sha)) = (
            request.source_project_id,
            request.sha.as_deref().filter(|sha| !sha.is_empty()),
        ) {
            http::optional_json::<Commit>(self.request(
                config,
                token,
                &[
                    "projects",
                    &project.to_string(),
                    "repository",
                    "commits",
                    sha,
                ],
            )?)
            .await?
            .map(|commit| ChangeRequestCommit {
                sha: commit.id,
                title: commit.title,
                author: commit.author_name,
                committed_at: commit.committed_date,
            })
        } else {
            None
        };
        Ok(Some(ChangeRequestDetails {
            change_request: summary,
            body: request.description,
            labels: request.labels,
            reviews,
            checks,
            latest_commit,
        }))
    }
}

#[cfg(test)]
mod tests;
