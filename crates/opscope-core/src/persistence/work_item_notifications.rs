use super::*;
use crate::application::{WorkItemNotificationRepository, WorkItemNotificationState};

impl WorkItemNotificationRepository for SqliteDatabase {
    fn load_work_items(
        &self,
        source_id: &str,
        repository_id: &str,
        account_id: &str,
    ) -> Result<WorkItemNotificationState, PersistenceFailure> {
        let json: Option<String> = self.lock()?.query_row(
            "SELECT snapshot FROM work_item_notification_states WHERE source_id = ?1 AND repository_id = ?2 AND account_id = ?3",
            params![source_id, repository_id, account_id], |row| row.get(0),
        ).optional().map_err(|_| PersistenceFailure)?;
        json.map(|json| serde_json::from_str(&json).map_err(|_| PersistenceFailure))
            .transpose()
            .map(Option::unwrap_or_default)
    }

    fn save_work_items(
        &self,
        source_id: &str,
        repository_id: &str,
        account_id: &str,
        state: &WorkItemNotificationState,
    ) -> Result<(), PersistenceFailure> {
        let json = serde_json::to_string(state).map_err(|_| PersistenceFailure)?;
        self.lock()?.execute(
            "INSERT INTO work_item_notification_states (source_id, repository_id, account_id, snapshot)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_id, repository_id, account_id) DO UPDATE SET snapshot = excluded.snapshot",
            params![source_id, repository_id, account_id, json],
        ).map_err(|_| PersistenceFailure)?;
        Ok(())
    }
}
