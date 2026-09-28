use super::*;

impl ReadThroughSourceData {
    pub(super) async fn load_issues(
        &self,
        source_id: &str,
        repository: &Repository,
        refresh: RefreshMode,
    ) -> Result<Option<Vec<Issue>>, SourceDataFailure> {
        let (connection, module) = self.connection(source_id)?;
        if !module
            .descriptor()
            .supports(crate::application::SourceCapability::Issues)
        {
            return Ok(None);
        }
        let now = Self::now();
        let cached = self.cache.as_ref().and_then(|cache| {
            cache
                .issues(source_id, &connection.account.external_id, &repository.id)
                .ok()
                .flatten()
        });
        if let Some(snapshot) = &cached
            && (refresh == RefreshMode::CacheFirst
                || refresh == RefreshMode::IfStale
                    && Self::is_fresh(snapshot.refreshed_at, now, self.cache_policy.issues))
        {
            return Ok(Some(snapshot.issues.clone()));
        }

        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?;
        let issues = match module
            .list_issues(&connection.configuration, &token, repository)
            .await
        {
            Ok(Some(issues)) => issues,
            Ok(None) => return Ok(None),
            Err(_) if refresh == RefreshMode::IfStale && cached.is_some() => {
                return Ok(cached.map(|snapshot| snapshot.issues));
            }
            Err(failure) => return Err(SourceDataFailure::Source(failure)),
        };
        if let Some(cache) = &self.cache {
            _ = cache.replace_issues(
                source_id,
                &connection.account.external_id,
                &repository.id,
                &IssueSnapshot {
                    refreshed_at: Self::now(),
                    issues: issues.clone(),
                },
            );
        }
        Ok(Some(issues))
    }

    pub(super) async fn load_issue_details(
        &self,
        source_id: &str,
        repository: &Repository,
        number: u64,
        refresh: RefreshMode,
    ) -> Result<Option<IssueDetails>, SourceDataFailure> {
        let (connection, module) = self.connection(source_id)?;
        if !module
            .descriptor()
            .supports(crate::application::SourceCapability::Issues)
        {
            return Ok(None);
        }
        let now = Self::now();
        let cached = self.cache.as_ref().and_then(|cache| {
            cache
                .issue_details(
                    source_id,
                    &connection.account.external_id,
                    &repository.id,
                    number,
                )
                .ok()
                .flatten()
        });
        if let Some(snapshot) = &cached
            && (refresh == RefreshMode::CacheFirst
                || refresh == RefreshMode::IfStale
                    && Self::is_fresh(snapshot.refreshed_at, now, self.cache_policy.issue_details))
        {
            return Ok(Some(snapshot.details.clone()));
        }

        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?;
        let details = match module
            .issue_details(&connection.configuration, &token, repository, number)
            .await
        {
            Ok(Some(details)) => details,
            Ok(None) => return Ok(None),
            Err(_) if refresh == RefreshMode::IfStale && cached.is_some() => {
                return Ok(cached.map(|snapshot| snapshot.details));
            }
            Err(failure) => return Err(SourceDataFailure::Source(failure)),
        };
        if let Some(cache) = &self.cache {
            _ = cache.replace_issue_details(
                source_id,
                &connection.account.external_id,
                &repository.id,
                number,
                &IssueDetailsSnapshot {
                    refreshed_at: Self::now(),
                    details: details.clone(),
                },
            );
        }
        Ok(Some(details))
    }
}
