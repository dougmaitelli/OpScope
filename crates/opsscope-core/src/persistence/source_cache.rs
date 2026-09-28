mod change_requests;
mod issues;
mod workflows;

use super::SqliteDatabase;
use crate::application::{ConnectionValidationFailure, PersistenceFailure};
use crate::domain::{
    ChangeRequest, ChangeRequestCheckStatus, ChangeRequestMergeStatus, ChangeRequestReviewStatus,
    ChangeRequestState, Repository, RepositoryVisibility, RunLifecycle, RunOutcome, Workflow,
    WorkflowRun, WorkflowState,
};
use crate::source_data::{
    ChangeRequestDetailsSnapshot, ChangeRequestSnapshot, IssueDetailsSnapshot, IssueSnapshot,
    RepositorySnapshot, SourceDataCache, WorkflowRunSnapshot, WorkflowSnapshot,
};
use rusqlite::{OptionalExtension, params};

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
        transaction
            .execute(
                "DELETE FROM issue_cache WHERE source_id = ?1 AND account_id <> ?2",
                params![source_id, account_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM issue_details WHERE source_id = ?1 AND account_id <> ?2",
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
        self.cached_workflows(source_id, account_id, repository_id)
    }

    fn replace_workflows(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        snapshot: &WorkflowSnapshot,
    ) -> Result<(), PersistenceFailure> {
        self.cached_replace_workflows(source_id, account_id, repository_id, snapshot)
    }

    fn workflow_runs(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
    ) -> Result<Option<WorkflowRunSnapshot>, PersistenceFailure> {
        self.cached_workflow_runs(source_id, account_id, repository_id)
    }

    fn replace_workflow_runs(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        snapshot: &WorkflowRunSnapshot,
    ) -> Result<(), PersistenceFailure> {
        self.cached_replace_workflow_runs(source_id, account_id, repository_id, snapshot)
    }

    fn change_requests(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
    ) -> Result<Option<ChangeRequestSnapshot>, PersistenceFailure> {
        self.cached_change_requests(source_id, account_id, repository_id)
    }

    fn replace_change_requests(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        snapshot: &ChangeRequestSnapshot,
    ) -> Result<(), PersistenceFailure> {
        self.cached_replace_change_requests(source_id, account_id, repository_id, snapshot)
    }

    fn change_request_details(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        number: u64,
    ) -> Result<Option<ChangeRequestDetailsSnapshot>, PersistenceFailure> {
        self.cached_change_request_details(source_id, account_id, repository_id, number)
    }

    fn replace_change_request_details(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        number: u64,
        snapshot: &ChangeRequestDetailsSnapshot,
    ) -> Result<(), PersistenceFailure> {
        self.cached_replace_change_request_details(
            source_id,
            account_id,
            repository_id,
            number,
            snapshot,
        )
    }

    fn issues(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
    ) -> Result<Option<IssueSnapshot>, PersistenceFailure> {
        issues::load(self, source_id, account_id, repository_id)
    }

    fn replace_issues(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        snapshot: &IssueSnapshot,
    ) -> Result<(), PersistenceFailure> {
        issues::replace(self, source_id, account_id, repository_id, snapshot)
    }

    fn issue_details(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        number: u64,
    ) -> Result<Option<IssueDetailsSnapshot>, PersistenceFailure> {
        issues::load_details(self, source_id, account_id, repository_id, number)
    }

    fn replace_issue_details(
        &self,
        source_id: &str,
        account_id: &str,
        repository_id: &str,
        number: u64,
        snapshot: &IssueDetailsSnapshot,
    ) -> Result<(), PersistenceFailure> {
        issues::replace_details(self, source_id, account_id, repository_id, number, snapshot)
    }
}

#[cfg(test)]
mod tests;
