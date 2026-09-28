use super::*;

impl ReadThroughSourceData {
    pub(super) async fn load_change_requests(
        &self,
        source_id: &str,
        repository: &Repository,
        refresh: RefreshMode,
    ) -> Result<Option<Vec<ChangeRequest>>, SourceDataFailure> {
        let (connection, module) = self.connection(source_id)?;
        if !module
            .descriptor()
            .supports(crate::application::SourceCapability::ChangeRequests)
        {
            return Ok(None);
        }
        let now = Self::now();
        let cached = self.cache.as_ref().and_then(|cache| {
            cache
                .change_requests(source_id, &connection.account.external_id, &repository.id)
                .ok()
                .flatten()
        });
        if let Some(snapshot) = &cached
            && (refresh == RefreshMode::CacheFirst
                || refresh == RefreshMode::IfStale
                    && Self::is_fresh(
                        snapshot.refreshed_at,
                        now,
                        self.cache_policy.change_requests,
                    ))
        {
            return Ok(Some(snapshot.change_requests.clone()));
        }

        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?;
        let change_requests = match module
            .list_change_requests(&connection.configuration, &token, repository)
            .await
        {
            Ok(Some(change_requests)) => change_requests,
            Ok(None) => return Ok(None),
            Err(_) if refresh == RefreshMode::IfStale && cached.is_some() => {
                return Ok(cached.map(|snapshot| snapshot.change_requests));
            }
            Err(failure) => return Err(SourceDataFailure::Source(failure)),
        };

        if let Some(cache) = &self.cache {
            _ = cache.replace_change_requests(
                source_id,
                &connection.account.external_id,
                &repository.id,
                &ChangeRequestSnapshot {
                    refreshed_at: Self::now(),
                    change_requests: change_requests.clone(),
                },
            );
        }
        Ok(Some(change_requests))
    }

    pub(super) async fn load_change_request_details(
        &self,
        source_id: &str,
        repository: &Repository,
        number: u64,
        refresh: RefreshMode,
    ) -> Result<Option<ChangeRequestDetails>, SourceDataFailure> {
        let (connection, module) = self.connection(source_id)?;
        if !module
            .descriptor()
            .supports(crate::application::SourceCapability::ChangeRequests)
        {
            return Ok(None);
        }
        let now = Self::now();
        let cached = self.cache.as_ref().and_then(|cache| {
            cache
                .change_request_details(
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
                    && Self::is_fresh(
                        snapshot.refreshed_at,
                        now,
                        self.cache_policy.change_request_details,
                    ))
        {
            return Ok(Some(snapshot.details.clone()));
        }
        let token = self
            .secrets
            .retrieve(&connection.secret_reference)
            .map_err(|_| SourceDataFailure::StorageUnavailable)?;
        let details = match module
            .change_request_details(&connection.configuration, &token, repository, number)
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
            _ = cache.replace_change_request_details(
                source_id,
                &connection.account.external_id,
                &repository.id,
                number,
                &ChangeRequestDetailsSnapshot {
                    refreshed_at: Self::now(),
                    details: details.clone(),
                },
            );
        }
        Ok(Some(details))
    }
}
