use super::SqliteDatabase;
use crate::application::{ConnectionValidationFailure, PersistenceFailure};
use crate::domain::{
    ChangeRequest, ChangeRequestCheckStatus, ChangeRequestMergeStatus, ChangeRequestReviewStatus,
    ChangeRequestState, Repository, RepositoryVisibility, RunLifecycle, RunOutcome, Workflow,
    WorkflowRun, WorkflowState,
};
use crate::source_data::{
    ChangeRequestDetailsSnapshot, ChangeRequestSnapshot, RepositorySnapshot, SourceDataCache,
    WorkflowRunSnapshot, WorkflowSnapshot,
};
use rusqlite::{OptionalExtension, params};

fn validation_failure_name(failure: ConnectionValidationFailure) -> &'static str {
    match failure {
        ConnectionValidationFailure::InvalidConfiguration => "invalid_configuration",
        ConnectionValidationFailure::InvalidCredentials => "invalid_credentials",
        ConnectionValidationFailure::PermissionDenied => "permission_denied",
        ConnectionValidationFailure::RateLimited => "rate_limited",
        ConnectionValidationFailure::ProviderUnavailable => "provider_unavailable",
        ConnectionValidationFailure::UnexpectedResponse => "unexpected_response",
    }
}

fn parse_validation_failure(
    value: &str,
) -> Result<ConnectionValidationFailure, PersistenceFailure> {
    match value {
        "invalid_configuration" => Ok(ConnectionValidationFailure::InvalidConfiguration),
        "invalid_credentials" => Ok(ConnectionValidationFailure::InvalidCredentials),
        "permission_denied" => Ok(ConnectionValidationFailure::PermissionDenied),
        "rate_limited" => Ok(ConnectionValidationFailure::RateLimited),
        "provider_unavailable" => Ok(ConnectionValidationFailure::ProviderUnavailable),
        "unexpected_response" => Ok(ConnectionValidationFailure::UnexpectedResponse),
        _ => Err(PersistenceFailure),
    }
}

fn lifecycle_name(lifecycle: RunLifecycle) -> &'static str {
    match lifecycle {
        RunLifecycle::Queued => "queued",
        RunLifecycle::Running => "running",
        RunLifecycle::Completed => "completed",
        RunLifecycle::Unknown => "unknown",
    }
}

fn parse_lifecycle(value: &str) -> Result<RunLifecycle, rusqlite::Error> {
    match value {
        "queued" => Ok(RunLifecycle::Queued),
        "running" => Ok(RunLifecycle::Running),
        "completed" => Ok(RunLifecycle::Completed),
        "unknown" => Ok(RunLifecycle::Unknown),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn outcome_name(outcome: RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Success => "success",
        RunOutcome::Warning => "warning",
        RunOutcome::Failure => "failure",
        RunOutcome::Cancelled => "cancelled",
        RunOutcome::Skipped => "skipped",
        RunOutcome::Unknown => "unknown",
    }
}

fn parse_outcome(value: &str) -> Result<RunOutcome, rusqlite::Error> {
    match value {
        "success" => Ok(RunOutcome::Success),
        "warning" => Ok(RunOutcome::Warning),
        "failure" => Ok(RunOutcome::Failure),
        "cancelled" => Ok(RunOutcome::Cancelled),
        "skipped" => Ok(RunOutcome::Skipped),
        "unknown" => Ok(RunOutcome::Unknown),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

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

impl SourceDataCache for SqliteDatabase {
    fn repositories(
        &self,
        source_id: &str,
        account_id: &str,
    ) -> Result<Option<RepositorySnapshot>, PersistenceFailure> {
        let database = self.lock()?;
        let refreshed_at = database
            .query_row(
                "SELECT refreshed_at FROM repository_cache_sync
                 WHERE source_id = ?1 AND account_id = ?2",
                params![source_id, account_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|_| PersistenceFailure)?;
        let Some(refreshed_at) = refreshed_at else {
            return Ok(None);
        };
        let mut statement = database
            .prepare(
                "SELECT repository_id, owner, name, description, visibility, web_url
                 FROM repositories
                 WHERE source_id = ?1 AND account_id = ?2
                 ORDER BY owner, name",
            )
            .map_err(|_| PersistenceFailure)?;
        let rows = statement
            .query_map(params![source_id, account_id], |row| {
                let visibility = match row.get::<_, String>(4)?.as_str() {
                    "public" => RepositoryVisibility::Public,
                    "private" => RepositoryVisibility::Private,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                Ok(Repository {
                    id: row.get(0)?,
                    owner: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    visibility,
                    web_url: row.get(5)?,
                })
            })
            .map_err(|_| PersistenceFailure)?;
        let repositories = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PersistenceFailure)?;
        Ok(Some(RepositorySnapshot {
            refreshed_at: refreshed_at.try_into().map_err(|_| PersistenceFailure)?,
            repositories,
        }))
    }

    fn replace_repositories(
        &self,
        source_id: &str,
        account_id: &str,
        snapshot: &RepositorySnapshot,
    ) -> Result<(), PersistenceFailure> {
        let refreshed_at = i64::try_from(snapshot.refreshed_at).map_err(|_| PersistenceFailure)?;
        let mut database = self.lock()?;
        let transaction = database.transaction().map_err(|_| PersistenceFailure)?;
        transaction
            .execute("DELETE FROM repositories WHERE source_id = ?1", [source_id])
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM repository_cache_sync WHERE source_id = ?1",
                [source_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM workflows WHERE source_id = ?1 AND account_id <> ?2",
                params![source_id, account_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM workflow_cache_sync WHERE source_id = ?1 AND account_id <> ?2",
                params![source_id, account_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM workflow_runs WHERE source_id = ?1 AND account_id <> ?2",
                params![source_id, account_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM workflow_run_cache_sync
                 WHERE source_id = ?1 AND account_id <> ?2",
                params![source_id, account_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM change_requests WHERE source_id = ?1 AND account_id <> ?2",
                params![source_id, account_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM change_request_cache_sync
                 WHERE source_id = ?1 AND account_id <> ?2",
                params![source_id, account_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM change_request_details WHERE source_id = ?1 AND account_id <> ?2",
                params![source_id, account_id],
            )
            .map_err(|_| PersistenceFailure)?;
        for repository in &snapshot.repositories {
            let visibility = match repository.visibility {
                RepositoryVisibility::Public => "public",
                RepositoryVisibility::Private => "private",
            };
            transaction
                .execute(
                    "INSERT INTO repositories (
                       source_id, account_id, repository_id, owner, name, description,
                       visibility, web_url
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        source_id,
                        account_id,
                        repository.id,
                        repository.owner,
                        repository.name,
                        repository.description,
                        visibility,
                        repository.web_url,
                    ],
                )
                .map_err(|_| PersistenceFailure)?;
        }
        transaction
            .execute(
                "INSERT INTO repository_cache_sync (source_id, account_id, refreshed_at)
                 VALUES (?1, ?2, ?3)",
                params![source_id, account_id, refreshed_at],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction.commit().map_err(|_| PersistenceFailure)
    }

    fn workflows(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
    ) -> Result<Option<WorkflowSnapshot>, PersistenceFailure> {
        let database = self.lock()?;
        let refreshed_at = database
            .query_row(
                "SELECT refreshed_at FROM workflow_cache_sync
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
                "SELECT workflow_id, name, path, state, web_url
                 FROM workflows
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3
                 ORDER BY name",
            )
            .map_err(|_| PersistenceFailure)?;
        let rows = statement
            .query_map(params![source_id, account_id, repository_id], |row| {
                let state = match row.get::<_, String>(3)?.as_str() {
                    "active" => WorkflowState::Active,
                    "disabled" => WorkflowState::Disabled,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                Ok(Workflow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    state,
                    web_url: row.get(4)?,
                })
            })
            .map_err(|_| PersistenceFailure)?;
        let workflows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PersistenceFailure)?;
        Ok(Some(WorkflowSnapshot {
            refreshed_at: refreshed_at.try_into().map_err(|_| PersistenceFailure)?,
            workflows,
        }))
    }

    fn replace_workflows(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        snapshot: &WorkflowSnapshot,
    ) -> Result<(), PersistenceFailure> {
        let refreshed_at = i64::try_from(snapshot.refreshed_at).map_err(|_| PersistenceFailure)?;
        let mut database = self.lock()?;
        let transaction = database.transaction().map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM workflows
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3",
                params![source_id, account_id, repository_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM workflow_cache_sync
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3",
                params![source_id, account_id, repository_id],
            )
            .map_err(|_| PersistenceFailure)?;
        for workflow in &snapshot.workflows {
            let state = match workflow.state {
                WorkflowState::Active => "active",
                WorkflowState::Disabled => "disabled",
            };
            transaction
                .execute(
                    "INSERT INTO workflows (
                       source_id, account_id, repository_id, workflow_id, name, path, state,
                       web_url
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        source_id,
                        account_id,
                        repository_id,
                        workflow.id,
                        workflow.name,
                        workflow.path,
                        state,
                        workflow.web_url,
                    ],
                )
                .map_err(|_| PersistenceFailure)?;
        }
        transaction
            .execute(
                "INSERT INTO workflow_cache_sync (
                   source_id, account_id, repository_id, refreshed_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![source_id, account_id, repository_id, refreshed_at],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction.commit().map_err(|_| PersistenceFailure)
    }

    fn workflow_runs(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
    ) -> Result<Option<WorkflowRunSnapshot>, PersistenceFailure> {
        let database = self.lock()?;
        let sync = database
            .query_row(
                "SELECT last_attempted_at, last_successful_at, last_error
                 FROM workflow_run_cache_sync
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3",
                params![source_id, account_id, repository_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| PersistenceFailure)?;
        let Some((last_attempted_at, last_successful_at, last_error)) = sync else {
            return Ok(None);
        };
        let mut statement = database
            .prepare(
                "SELECT run_id, workflow_id, run_number, attempt, title, lifecycle, outcome,
                        branch, commit_sha, actor, trigger, created_at, started_at, updated_at,
                        web_url, provider_status, provider_conclusion
                 FROM workflow_runs
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3
                 ORDER BY created_at DESC, run_number DESC, attempt DESC",
            )
            .map_err(|_| PersistenceFailure)?;
        let rows = statement
            .query_map(params![source_id, account_id, repository_id], |row| {
                let run_number = u64::try_from(row.get::<_, i64>(2)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let attempt = u64::try_from(row.get::<_, i64>(3)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                Ok(WorkflowRun {
                    id: row.get(0)?,
                    workflow_id: row.get(1)?,
                    run_number,
                    attempt,
                    title: row.get(4)?,
                    lifecycle: parse_lifecycle(&row.get::<_, String>(5)?)?,
                    outcome: parse_outcome(&row.get::<_, String>(6)?)?,
                    branch: row.get(7)?,
                    commit_sha: row.get(8)?,
                    actor: row.get(9)?,
                    trigger: row.get(10)?,
                    created_at: row.get(11)?,
                    started_at: row.get(12)?,
                    updated_at: row.get(13)?,
                    web_url: row.get(14)?,
                    provider_status: row.get(15)?,
                    provider_conclusion: row.get(16)?,
                })
            })
            .map_err(|_| PersistenceFailure)?;
        let runs = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PersistenceFailure)?;
        Ok(Some(WorkflowRunSnapshot {
            last_attempted_at: last_attempted_at
                .try_into()
                .map_err(|_| PersistenceFailure)?,
            last_successful_at: last_successful_at
                .map(u64::try_from)
                .transpose()
                .map_err(|_| PersistenceFailure)?,
            last_error: last_error
                .as_deref()
                .map(parse_validation_failure)
                .transpose()?,
            runs,
        }))
    }

    fn replace_workflow_runs(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        snapshot: &WorkflowRunSnapshot,
    ) -> Result<(), PersistenceFailure> {
        let last_attempted_at =
            i64::try_from(snapshot.last_attempted_at).map_err(|_| PersistenceFailure)?;
        let last_successful_at = snapshot
            .last_successful_at
            .map(i64::try_from)
            .transpose()
            .map_err(|_| PersistenceFailure)?;
        let mut database = self.lock()?;
        let transaction = database.transaction().map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM workflow_runs
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3",
                params![source_id, account_id, repository_id],
            )
            .map_err(|_| PersistenceFailure)?;
        for run in &snapshot.runs {
            let run_number = i64::try_from(run.run_number).map_err(|_| PersistenceFailure)?;
            let attempt = i64::try_from(run.attempt).map_err(|_| PersistenceFailure)?;
            transaction
                .execute(
                    "INSERT INTO workflow_runs (
                       source_id, account_id, repository_id, run_id, workflow_id, run_number,
                       attempt, title, lifecycle, outcome, branch, commit_sha, actor, trigger,
                       created_at, started_at, updated_at, web_url, provider_status,
                       provider_conclusion
                     ) VALUES (
                       ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                       ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
                     )",
                    params![
                        source_id,
                        account_id,
                        repository_id,
                        run.id,
                        run.workflow_id,
                        run_number,
                        attempt,
                        run.title,
                        lifecycle_name(run.lifecycle),
                        outcome_name(run.outcome),
                        run.branch,
                        run.commit_sha,
                        run.actor,
                        run.trigger,
                        run.created_at,
                        run.started_at,
                        run.updated_at,
                        run.web_url,
                        run.provider_status,
                        run.provider_conclusion,
                    ],
                )
                .map_err(|_| PersistenceFailure)?;
        }
        transaction
            .execute(
                "INSERT INTO workflow_run_cache_sync (
                   source_id, account_id, repository_id, last_attempted_at,
                   last_successful_at, last_error
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(source_id, account_id, repository_id) DO UPDATE SET
                   last_attempted_at = excluded.last_attempted_at,
                   last_successful_at = excluded.last_successful_at,
                   last_error = excluded.last_error",
                params![
                    source_id,
                    account_id,
                    repository_id,
                    last_attempted_at,
                    last_successful_at,
                    snapshot.last_error.map(validation_failure_name),
                ],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction.commit().map_err(|_| PersistenceFailure)
    }

    fn change_requests(
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
                        updated_at, web_url
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

    fn replace_change_requests(
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
                       check_status, merge_status, created_at, updated_at, web_url
                     ) VALUES (
                       ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                       ?15, ?16, ?17
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

    fn change_request_details(
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

    fn replace_change_request_details(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        ConnectionRepository, SecretReference, StoredConnection, ValidatedAccount,
    };
    use tempfile::tempdir;

    fn test_connection() -> StoredConnection {
        StoredConnection {
            id: "example".to_owned(),
            source_id: "example".to_owned(),
            unique_key: "example".to_owned(),
            label: "Example".to_owned(),
            configuration: Default::default(),
            account: ValidatedAccount {
                external_id: "42".to_owned(),
                name: "The Octocat".to_owned(),
                handle: Some("octocat".to_owned()),
                profile_url: Some("https://example.com/octocat".to_owned()),
            },
            secret_reference: SecretReference::for_connection("example", "test"),
        }
    }

    #[test]
    fn snapshots_persist_and_support_empty_results() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let database_path = directory.path().join("opsscope.sqlite3");
        let repositories = RepositorySnapshot {
            refreshed_at: 123,
            repositories: vec![Repository {
                id: "repository-1".to_owned(),
                owner: "octocat".to_owned(),
                name: "hello-world".to_owned(),
                description: Some("A cached repository".to_owned()),
                visibility: RepositoryVisibility::Private,
                web_url: "https://example.com/octocat/hello-world".to_owned(),
            }],
        };
        let workflows = WorkflowSnapshot {
            refreshed_at: 124,
            workflows: Vec::new(),
        };
        let runs = WorkflowRunSnapshot {
            last_attempted_at: 125,
            last_successful_at: Some(125),
            last_error: None,
            runs: vec![WorkflowRun {
                id: "run-1".to_owned(),
                workflow_id: "workflow-1".to_owned(),
                run_number: 7,
                attempt: 2,
                title: "Build main".to_owned(),
                lifecycle: RunLifecycle::Completed,
                outcome: RunOutcome::Success,
                branch: Some("main".to_owned()),
                commit_sha: "abcdef123456".to_owned(),
                actor: Some("octocat".to_owned()),
                trigger: "push".to_owned(),
                created_at: "2026-09-26T18:00:00Z".to_owned(),
                started_at: Some("2026-09-26T18:00:02Z".to_owned()),
                updated_at: "2026-09-26T18:03:00Z".to_owned(),
                web_url: "https://example.com/runs/1".to_owned(),
                provider_status: "completed".to_owned(),
                provider_conclusion: Some("success".to_owned()),
            }],
        };
        let change_requests = ChangeRequestSnapshot {
            refreshed_at: 126,
            change_requests: vec![ChangeRequest {
                id: "change-1".to_owned(),
                number: 42,
                title: "Harden authentication".to_owned(),
                author: Some("octocat".to_owned()),
                source_branch: "auth-fix".to_owned(),
                target_branch: "main".to_owned(),
                state: ChangeRequestState::Open,
                draft: false,
                review_status: ChangeRequestReviewStatus::Approved,
                check_status: ChangeRequestCheckStatus::Passed,
                merge_status: ChangeRequestMergeStatus::Ready,
                created_at: "2026-09-26T18:00:00Z".to_owned(),
                updated_at: "2026-09-27T18:00:00Z".to_owned(),
                web_url: "https://example.com/pulls/42".to_owned(),
            }],
        };
        let change_request_details = ChangeRequestDetailsSnapshot {
            refreshed_at: 127,
            details: crate::domain::ChangeRequestDetails {
                change_request: change_requests.change_requests[0].clone(),
                body: Some("Improves token handling.".to_owned()),
                labels: vec!["security".to_owned()],
                reviews: vec![crate::domain::ChangeRequestReview {
                    reviewer: Some("reviewer".to_owned()),
                    status: ChangeRequestReviewStatus::Approved,
                    submitted_at: Some("2026-09-27T17:00:00Z".to_owned()),
                }],
                checks: vec![crate::domain::ChangeRequestCheck {
                    name: "test".to_owned(),
                    status: ChangeRequestCheckStatus::Passed,
                    web_url: Some("https://example.com/checks/1".to_owned()),
                    workflow_run_id: Some("123".to_owned()),
                }],
                latest_commit: Some(crate::domain::ChangeRequestCommit {
                    sha: "abcdef123456".to_owned(),
                    title: "Harden tokens".to_owned(),
                    author: Some("octocat".to_owned()),
                    committed_at: "2026-09-27T16:00:00Z".to_owned(),
                }),
            },
        };

        {
            let database = SqliteDatabase::open(&database_path)?;
            database.save(&test_connection())?;
            database.replace_repositories("example", "42", &repositories)?;
            database.replace_workflows("example", "42", "repository-1", &workflows)?;
            database.replace_workflow_runs("example", "42", "repository-1", &runs)?;
            database.replace_change_requests("example", "42", "repository-1", &change_requests)?;
            database.replace_change_request_details(
                "example",
                "42",
                "repository-1",
                42,
                &change_request_details,
            )?;
        }

        let database = SqliteDatabase::open(&database_path)?;
        assert_eq!(database.repositories("example", "42")?, Some(repositories));
        assert!(
            database
                .repositories("example", "different-account")?
                .is_none()
        );
        assert_eq!(
            database.workflows("example", "42", "repository-1")?,
            Some(workflows)
        );
        assert_eq!(
            database.workflow_runs("example", "42", "repository-1")?,
            Some(runs)
        );
        assert_eq!(
            database.change_requests("example", "42", "repository-1")?,
            Some(change_requests)
        );
        assert_eq!(
            database.change_request_details("example", "42", "repository-1", 42)?,
            Some(change_request_details)
        );
        database.delete("example")?;
        assert!(database.repositories("example", "42")?.is_none());
        assert!(
            database
                .workflows("example", "42", "repository-1")?
                .is_none()
        );
        assert!(
            database
                .workflow_runs("example", "42", "repository-1")?
                .is_none()
        );
        assert!(
            database
                .change_requests("example", "42", "repository-1")?
                .is_none()
        );
        assert!(
            database
                .change_request_details("example", "42", "repository-1", 42)?
                .is_none()
        );
        Ok(())
    }
}
