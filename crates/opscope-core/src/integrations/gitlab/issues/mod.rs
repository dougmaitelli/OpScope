use super::*;
use crate::domain::{Issue, IssueComment, IssueDetails, IssueState, Relationships};

mod models;
use models::*;

impl GitLabClient {
    async fn issue_tracker_enabled(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
    ) -> Result<bool, Failure> {
        let tracker: Tracker =
            http::json(self.request(config, token, &["projects", &repo.id])?).await?;
        // Only an explicit disabled setting means unsupported. A denied request
        // must remain an error so it cannot erase the last successful snapshot.
        Ok(tracker.issues_access_level.as_deref() != Some("disabled")
            && tracker.issues_enabled != Some(false))
    }

    async fn issue_notes(
        &self,
        config: &ConnectionConfiguration,
        token: &ProviderToken,
        repo: &Repository,
        number: u64,
    ) -> Result<Vec<Note>, Failure> {
        self.pages(
            config,
            token,
            &["projects", &repo.id, "issues", &number.to_string(), "notes"],
            &[("order_by", "created_at"), ("sort", "asc")],
            10,
            false,
        )
        .await
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
                &["projects", &repo.id, "issues"],
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
        let owner = if values.is_empty() {
            None
        } else {
            self.relationship_viewer(config, token)
                .await
                .map(|owner| owner.id)
        };
        let mut issues = Vec::new();
        for mut value in values {
            let mut issue = value.summary()?;
            // Some list responses omit the viewer's subscription flag.
            // This optional lookup must not break ordinary monitoring.
            if owner.is_some()
                && value.subscribed.is_none()
                && let Ok(request) = self.request(
                    config,
                    token,
                    &["projects", &repo.id, "issues", &value.iid.to_string()],
                )
                && let Ok(detail) = http::json::<ProviderIssue>(request).await
            {
                value.subscribed = detail.subscribed;
            }
            let notes = self.issue_notes(config, token, repo, value.iid).await.ok();
            issue.relationships = value.relationships(owner, notes.as_deref());
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
            &["projects", &repo.id, "issues", &number.to_string()],
        )?)
        .await?
        else {
            return Ok(None);
        };
        let mut issue = value.summary()?;
        let owner = self
            .relationship_viewer(config, token)
            .await
            .map(|owner| owner.id);
        let notes = self.issue_notes(config, token, repo, number).await?;
        issue.relationships = value.relationships(owner, Some(&notes));
        let mut comments: Vec<_> = notes
            .into_iter()
            .filter(|note| !note.system)
            .map(IssueComment::from)
            .collect();
        // Match the existing dialog's recent-comment window.
        comments.drain(..comments.len().saturating_sub(100));
        Ok(Some(IssueDetails {
            issue,
            body: value.description.filter(|body| !body.trim().is_empty()),
            milestone: value.milestone.map(|milestone| milestone.title),
            comments,
        }))
    }
}

#[cfg(test)]
mod tests;
