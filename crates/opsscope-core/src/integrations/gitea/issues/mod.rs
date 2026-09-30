use super::*;
use crate::domain::{Issue, IssueComment, IssueDetails, IssueState, Relationships};

mod models;
use models::*;

impl GiteaClient {
    async fn issue_tracker_enabled(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
    ) -> Result<bool, Failure> {
        let tracker: Tracker =
            http::json(self.request(config, token, &["repos", &repo.owner, &repo.name])?).await?;
        Ok(tracker.has_issues != Some(false) && tracker.external_tracker.is_none())
    }

    async fn issue_comments(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Result<Vec<Comment>, Failure> {
        self.pages(
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
            &[],
        )
        .await
    }

    async fn issue_subscription(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Option<Subscription> {
        let request = self
            .request(
                config,
                token,
                &[
                    "repos",
                    &repo.owner,
                    &repo.name,
                    "issues",
                    &number.to_string(),
                    "subscriptions",
                    "check",
                ],
            )
            .ok()?;
        // 404 can mean absent subscription or inaccessible metadata. Never
        // mistake missing metadata for a proven negative personal relationship.
        http::json(request).await.ok()
    }

    pub(super) async fn issues(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
    ) -> Result<Option<Vec<Issue>>, Failure> {
        if !self.issue_tracker_enabled(config, token, repo).await? {
            return Ok(None);
        }
        let values: Vec<ProviderIssue> = self
            .pages(
                config,
                token,
                &["repos", &repo.owner, &repo.name, "issues"],
                &[("state", "open"), ("type", "issues")],
            )
            .await?;
        // Gitea shares issue numbering and endpoints with PRs. Filter both in
        // the request and defensively in the response.
        let values: Vec<_> = values
            .into_iter()
            .filter(|value| value.pull_request.is_none())
            .collect();
        let owner = if values.is_empty() {
            None
        } else {
            self.relationship_viewer(config, token)
                .await
                .map(|owner| owner.id)
        };
        let mut issues = Vec::new();
        for value in values {
            let mut issue = value.summary()?;
            let subscription = if owner.is_some() {
                self.issue_subscription(config, token, repo, value.number)
                    .await
            } else {
                None
            };
            let comments = self
                .issue_comments(config, token, repo, value.number)
                .await
                .ok();
            issue.relationships =
                value.relationships(owner, subscription.as_ref(), comments.as_deref());
            issues.push(issue);
        }
        Ok(Some(issues))
    }

    pub(super) async fn issue(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Result<Option<IssueDetails>, Failure> {
        if !self.issue_tracker_enabled(config, token, repo).await? {
            return Ok(None);
        }
        let Some(value) = http::optional_json::<ProviderIssue>(self.request(
            config,
            token,
            &[
                "repos",
                &repo.owner,
                &repo.name,
                "issues",
                &number.to_string(),
            ],
        )?)
        .await?
        else {
            return Ok(None);
        };
        if value.pull_request.is_some() {
            return Ok(None);
        }
        let mut issue = value.summary()?;
        let owner = self
            .relationship_viewer(config, token)
            .await
            .map(|owner| owner.id);
        let subscription = if owner.is_some() {
            self.issue_subscription(config, token, repo, number).await
        } else {
            None
        };
        let comments = self.issue_comments(config, token, repo, number).await?;
        issue.relationships = value.relationships(owner, subscription.as_ref(), Some(&comments));
        let mut comments: Vec<_> = comments.into_iter().map(IssueComment::from).collect();
        comments.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        comments.drain(..comments.len().saturating_sub(100));
        Ok(Some(IssueDetails {
            issue,
            body: value.body.filter(|body| !body.trim().is_empty()),
            milestone: value.milestone.map(|milestone| milestone.title),
            comments,
        }))
    }
}

#[cfg(test)]
mod tests;
