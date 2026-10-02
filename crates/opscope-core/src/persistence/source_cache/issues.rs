use super::super::SqliteDatabase;
use crate::application::PersistenceFailure;
use crate::source_data::{IssueDetailsSnapshot, IssueSnapshot};
use rusqlite::{OptionalExtension, params};

pub(super) fn load(
    database: &SqliteDatabase,
    source_id: &str,
    account_id: &str,
    repository_id: &str,
) -> Result<Option<IssueSnapshot>, PersistenceFailure> {
    database
        .lock()?
        .query_row(
            "SELECT refreshed_at, payload FROM issue_cache
             WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3",
            params![source_id, account_id, repository_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|_| PersistenceFailure)?
        .map(|(refreshed_at, payload)| {
            Ok(IssueSnapshot {
                refreshed_at: refreshed_at.try_into().map_err(|_| PersistenceFailure)?,
                issues: serde_json::from_str(&payload).map_err(|_| PersistenceFailure)?,
            })
        })
        .transpose()
}

pub(super) fn replace(
    database: &SqliteDatabase,
    source_id: &str,
    account_id: &str,
    repository_id: &str,
    snapshot: &IssueSnapshot,
) -> Result<(), PersistenceFailure> {
    let refreshed_at = i64::try_from(snapshot.refreshed_at).map_err(|_| PersistenceFailure)?;
    let payload = serde_json::to_string(&snapshot.issues).map_err(|_| PersistenceFailure)?;
    database
        .lock()?
        .execute(
            "INSERT INTO issue_cache (
               source_id, account_id, repository_id, refreshed_at, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(source_id, account_id, repository_id) DO UPDATE SET
               refreshed_at = excluded.refreshed_at,
               payload = excluded.payload",
            params![source_id, account_id, repository_id, refreshed_at, payload],
        )
        .map(|_| ())
        .map_err(|_| PersistenceFailure)
}

pub(super) fn load_details(
    database: &SqliteDatabase,
    source_id: &str,
    account_id: &str,
    repository_id: &str,
    number: u64,
) -> Result<Option<IssueDetailsSnapshot>, PersistenceFailure> {
    let number = i64::try_from(number).map_err(|_| PersistenceFailure)?;
    database
        .lock()?
        .query_row(
            "SELECT refreshed_at, payload FROM issue_details
             WHERE source_id = ?1 AND account_id = ?2 AND repository_id = ?3 AND number = ?4",
            params![source_id, account_id, repository_id, number],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|_| PersistenceFailure)?
        .map(|(refreshed_at, payload)| {
            Ok(IssueDetailsSnapshot {
                refreshed_at: refreshed_at.try_into().map_err(|_| PersistenceFailure)?,
                details: serde_json::from_str(&payload).map_err(|_| PersistenceFailure)?,
            })
        })
        .transpose()
}

pub(super) fn replace_details(
    database: &SqliteDatabase,
    source_id: &str,
    account_id: &str,
    repository_id: &str,
    number: u64,
    snapshot: &IssueDetailsSnapshot,
) -> Result<(), PersistenceFailure> {
    let number = i64::try_from(number).map_err(|_| PersistenceFailure)?;
    let refreshed_at = i64::try_from(snapshot.refreshed_at).map_err(|_| PersistenceFailure)?;
    let payload = serde_json::to_string(&snapshot.details).map_err(|_| PersistenceFailure)?;
    database
        .lock()?
        .execute(
            "INSERT INTO issue_details (
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
