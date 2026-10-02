use super::*;

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

impl SqliteDatabase {
    pub(super) fn cached_workflows(
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

    pub(super) fn cached_replace_workflows(
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

    pub(super) fn cached_workflow_runs(
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
                        web_url, provider_status, provider_conclusion, relationships
                 FROM workflow_runs
                 WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3
                 ORDER BY run_number DESC, attempt DESC, created_at DESC",
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
                    relationships: serde_json::from_str(&row.get::<_, String>(17)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
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

    pub(super) fn cached_replace_workflow_runs(
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
            store_run(&transaction, source_id, account_id, repository_id, run)?;
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

    pub(super) fn cached_update_workflow_run(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        run: &WorkflowRun,
    ) -> Result<(), PersistenceFailure> {
        let database = self.lock()?;
        store_run(&database, source_id, account_id, repository_id, run)
    }
}

fn store_run(
    transaction: &rusqlite::Connection,
    source_id: &str,
    account_id: &str,
    repository_id: &str,
    run: &WorkflowRun,
) -> Result<(), PersistenceFailure> {
    let run_number = i64::try_from(run.run_number).map_err(|_| PersistenceFailure)?;
    let attempt = i64::try_from(run.attempt).map_err(|_| PersistenceFailure)?;
    transaction
        .execute(
            "INSERT OR REPLACE INTO workflow_runs (
               source_id, account_id, repository_id, run_id, workflow_id, run_number,
               attempt, title, lifecycle, outcome, branch, commit_sha, actor, trigger,
               created_at, started_at, updated_at, web_url, provider_status,
               provider_conclusion, relationships
             ) VALUES (
               ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
               ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
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
                serde_json::to_string(&run.relationships).map_err(|_| PersistenceFailure)?,
            ],
        )
        .map_err(|_| PersistenceFailure)?;
    Ok(())
}
