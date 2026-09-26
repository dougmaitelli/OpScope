use super::SqliteDatabase;
use crate::application::PersistenceFailure;
use crate::domain::{Repository, RepositoryVisibility, Workflow, WorkflowState};
use crate::source_data::{RepositorySnapshot, SourceDataCache, WorkflowSnapshot};
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
            source_id: "example".to_owned(),
            account: ValidatedAccount {
                external_id: "42".to_owned(),
                name: "The Octocat".to_owned(),
                handle: Some("octocat".to_owned()),
                profile_url: Some("https://example.com/octocat".to_owned()),
            },
            secret_reference: SecretReference::for_source("example", "42"),
        }
    }

    #[test]
    fn snapshots_persist_and_support_empty_results() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let database_path = directory.path().join("ciwatcher.sqlite3");
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

        {
            let database = SqliteDatabase::open(&database_path)?;
            database.save(&test_connection())?;
            database.replace_repositories("example", "42", &repositories)?;
            database.replace_workflows("example", "42", "repository-1", &workflows)?;
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
        database.delete("example")?;
        assert!(database.repositories("example", "42")?.is_none());
        assert!(
            database
                .workflows("example", "42", "repository-1")?
                .is_none()
        );
        Ok(())
    }
}
