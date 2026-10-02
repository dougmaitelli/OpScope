//! Upgrade derived, viewer-specific cache values to unknown relationship evidence.
//! The next synchronization collects facts; old decisions are never treated as facts.
use crate::application::PersistenceFailure;
use rusqlite::Connection;

pub(super) fn migrate(connection: &Connection) -> Result<(), PersistenceFailure> {
    let legacy = connection
        .prepare("PRAGMA table_info(workflow_runs)")
        .map_err(|_| PersistenceFailure)?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|_| PersistenceFailure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| PersistenceFailure)?
        .iter()
        .any(|column| column == "relevance");
    if !legacy {
        return Ok(());
    }
    let transaction = connection
        .unchecked_transaction()
        .map_err(|_| PersistenceFailure)?;
    for (table, column) in [
        ("change_request_details", "payload"),
        ("issue_cache", "payload"),
        ("issue_details", "payload"),
        ("change_request_activity_states", "payload"),
        ("change_request_activity_events", "payload"),
        ("work_item_notification_states", "snapshot"),
    ] {
        let rows = transaction
            .prepare(&format!("SELECT rowid, {column} FROM {table}"))
            .map_err(|_| PersistenceFailure)?
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| PersistenceFailure)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PersistenceFailure)?;
        for (id, payload) in rows {
            let mut value: serde_json::Value =
                serde_json::from_str(&payload).map_err(|_| PersistenceFailure)?;
            remove_decisions(&mut value);
            transaction
                .execute(
                    &format!("UPDATE {table} SET {column} = ?1 WHERE rowid = ?2"),
                    rusqlite::params![value.to_string(), id],
                )
                .map_err(|_| PersistenceFailure)?;
        }
    }
    transaction
        .execute_batch(
            "ALTER TABLE workflow_runs DROP COLUMN relevance;
         ALTER TABLE change_requests DROP COLUMN relevance;
         UPDATE workflow_run_cache_sync SET last_successful_at = 0;
         UPDATE change_request_cache_sync SET refreshed_at = 0;
         UPDATE change_request_details SET refreshed_at = 0;
         UPDATE issue_cache SET refreshed_at = 0;
         UPDATE issue_details SET refreshed_at = 0;",
        )
        .map_err(|_| PersistenceFailure)?;
    transaction.commit().map_err(|_| PersistenceFailure)
}

fn remove_decisions(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            object.remove("relevance");
            for value in object.values_mut() {
                remove_decisions(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                remove_decisions(value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
