use super::*;

fn change_request_state_name(state: ChangeRequestState) -> &'static str {
    match state {
        ChangeRequestState::Open => "open",
        ChangeRequestState::Closed => "closed",
        ChangeRequestState::Merged => "merged",
    }
}

fn parse_change_request_state(value: &str) -> Result<ChangeRequestState, rusqlite::Error> {
    match value {
        "open" => Ok(ChangeRequestState::Open),
        "closed" => Ok(ChangeRequestState::Closed),
        "merged" => Ok(ChangeRequestState::Merged),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn review_status_name(status: ChangeRequestReviewStatus) -> &'static str {
    match status {
        ChangeRequestReviewStatus::Approved => "approved",
        ChangeRequestReviewStatus::ChangesRequested => "changes_requested",
        ChangeRequestReviewStatus::ReviewRequired => "review_required",
        ChangeRequestReviewStatus::Unknown => "unknown",
    }
}

fn parse_review_status(value: &str) -> Result<ChangeRequestReviewStatus, rusqlite::Error> {
    match value {
        "approved" => Ok(ChangeRequestReviewStatus::Approved),
        "changes_requested" => Ok(ChangeRequestReviewStatus::ChangesRequested),
        "review_required" => Ok(ChangeRequestReviewStatus::ReviewRequired),
        "unknown" => Ok(ChangeRequestReviewStatus::Unknown),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn check_status_name(status: ChangeRequestCheckStatus) -> &'static str {
    match status {
        ChangeRequestCheckStatus::Passed => "passed",
        ChangeRequestCheckStatus::Failing => "failing",
        ChangeRequestCheckStatus::Running => "running",
        ChangeRequestCheckStatus::Unknown => "unknown",
    }
}

fn parse_check_status(value: &str) -> Result<ChangeRequestCheckStatus, rusqlite::Error> {
    match value {
        "passed" => Ok(ChangeRequestCheckStatus::Passed),
        "failing" => Ok(ChangeRequestCheckStatus::Failing),
        "running" => Ok(ChangeRequestCheckStatus::Running),
        "unknown" => Ok(ChangeRequestCheckStatus::Unknown),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn merge_status_name(status: ChangeRequestMergeStatus) -> &'static str {
    match status {
        ChangeRequestMergeStatus::Ready => "ready",
        ChangeRequestMergeStatus::Blocked => "blocked",
        ChangeRequestMergeStatus::Conflicting => "conflicting",
        ChangeRequestMergeStatus::Unknown => "unknown",
    }
}

fn parse_merge_status(value: &str) -> Result<ChangeRequestMergeStatus, rusqlite::Error> {
    match value {
        "ready" => Ok(ChangeRequestMergeStatus::Ready),
        "blocked" => Ok(ChangeRequestMergeStatus::Blocked),
        "conflicting" => Ok(ChangeRequestMergeStatus::Conflicting),
        "unknown" => Ok(ChangeRequestMergeStatus::Unknown),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

impl SqliteDatabase {
    pub(super) fn cached_change_requests(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
    ) -> Result<Option<ChangeRequestSnapshot>, PersistenceFailure> {
        let database = self.lock()?;
        let refreshed_at = database
            .query_row(
                "SELECT refreshed_at FROM change_request_cache_sync
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3",
                params![source_id, account_id, repository_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|_| PersistenceFailure)?;
        let Some(refreshed_at) = refreshed_at else {
            return Ok(None);
        };
        let mut statement = database
            .prepare(
                "SELECT change_request_id, number, title, author, source_branch, target_branch,
                        state, draft, review_status, check_status, merge_status, created_at,
                        updated_at, web_url, relationships
                 FROM change_requests
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3
                 ORDER BY updated_at DESC, number DESC",
            )
            .map_err(|_| PersistenceFailure)?;
        let rows = statement
            .query_map(params![source_id, account_id, repository_id], |row| {
                Ok(ChangeRequest {
                    id: row.get(0)?,
                    number: u64::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    title: row.get(2)?,
                    author: row.get(3)?,
                    source_branch: row.get(4)?,
                    target_branch: row.get(5)?,
                    state: parse_change_request_state(&row.get::<_, String>(6)?)?,
                    draft: row.get(7)?,
                    review_status: parse_review_status(&row.get::<_, String>(8)?)?,
                    check_status: parse_check_status(&row.get::<_, String>(9)?)?,
                    merge_status: parse_merge_status(&row.get::<_, String>(10)?)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    web_url: row.get(13)?,
                    relationships: serde_json::from_str(&row.get::<_, String>(14)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                })
            })
            .map_err(|_| PersistenceFailure)?;
        let change_requests = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PersistenceFailure)?;
        Ok(Some(ChangeRequestSnapshot {
            refreshed_at: refreshed_at.try_into().map_err(|_| PersistenceFailure)?,
            change_requests,
        }))
    }

    pub(super) fn cached_replace_change_requests(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        snapshot: &ChangeRequestSnapshot,
    ) -> Result<(), PersistenceFailure> {
        let refreshed_at = i64::try_from(snapshot.refreshed_at).map_err(|_| PersistenceFailure)?;
        let mut database = self.lock()?;
        let transaction = database.transaction().map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM change_requests
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3",
                params![source_id, account_id, repository_id],
            )
            .map_err(|_| PersistenceFailure)?;
        for change_request in &snapshot.change_requests {
            transaction
                .execute(
                    "INSERT INTO change_requests (
                       source_id, account_id, repository_id, change_request_id, number, title,
                       author, source_branch, target_branch, state, draft, review_status,
                       check_status, merge_status, created_at, updated_at, web_url, relationships
                     ) VALUES (
                       ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                       ?15, ?16, ?17, ?18
                     )",
                    params![
                        source_id,
                        account_id,
                        repository_id,
                        change_request.id,
                        i64::try_from(change_request.number).map_err(|_| PersistenceFailure)?,
                        change_request.title,
                        change_request.author,
                        change_request.source_branch,
                        change_request.target_branch,
                        change_request_state_name(change_request.state),
                        change_request.draft,
                        review_status_name(change_request.review_status),
                        check_status_name(change_request.check_status),
                        merge_status_name(change_request.merge_status),
                        change_request.created_at,
                        change_request.updated_at,
                        change_request.web_url,
                        serde_json::to_string(&change_request.relationships)
                            .map_err(|_| PersistenceFailure)?,
                    ],
                )
                .map_err(|_| PersistenceFailure)?;
        }
        transaction
            .execute(
                "INSERT INTO change_request_cache_sync (
                   source_id, account_id, repository_id, refreshed_at
                 ) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(source_id, account_id, repository_id) DO UPDATE SET
                   refreshed_at = excluded.refreshed_at",
                params![source_id, account_id, repository_id, refreshed_at],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction.commit().map_err(|_| PersistenceFailure)
    }

    pub(super) fn cached_change_request_details(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        number: u64,
    ) -> Result<Option<ChangeRequestDetailsSnapshot>, PersistenceFailure> {
        let number = i64::try_from(number).map_err(|_| PersistenceFailure)?;
        let database = self.lock()?;
        database
            .query_row(
                "SELECT refreshed_at, payload FROM change_request_details
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3 AND number = ?4",
                params![source_id, account_id, repository_id, number],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|_| PersistenceFailure)?
            .map(|(refreshed_at, payload)| {
                Ok(ChangeRequestDetailsSnapshot {
                    refreshed_at: refreshed_at.try_into().map_err(|_| PersistenceFailure)?,
                    details: serde_json::from_str(&payload).map_err(|_| PersistenceFailure)?,
                })
            })
            .transpose()
    }

    pub(super) fn cached_replace_change_request_details(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        number: u64,
        snapshot: &ChangeRequestDetailsSnapshot,
    ) -> Result<(), PersistenceFailure> {
        let number = i64::try_from(number).map_err(|_| PersistenceFailure)?;
        let refreshed_at = i64::try_from(snapshot.refreshed_at).map_err(|_| PersistenceFailure)?;
        let payload = serde_json::to_string(&snapshot.details).map_err(|_| PersistenceFailure)?;
        let database = self.lock()?;
        database
            .execute(
                "INSERT INTO change_request_details (
                   source_id, account_id, repository_id, number, refreshed_at, payload
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(source_id, account_id, repository_id, number) DO UPDATE SET
                   refreshed_at = excluded.refreshed_at,
                   payload = excluded.payload",
                params![
                    source_id,
                    account_id,
                    repository_id,
                    number,
                    refreshed_at,
                    payload,
                ],
            )
            .map(|_| ())
            .map_err(|_| PersistenceFailure)
    }
}
