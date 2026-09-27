//! SQLite persistence for metadata, source snapshots, and encrypted server-side credentials.

mod source_cache;

use crate::application::{
    ActivityEventRepository, ChangeRequestActivityEvent, ConnectionRepository,
    DEFAULT_RECENT_RUNS_PER_WORKFLOW, DEFAULT_SYNCHRONIZATION_INTERVAL_SECONDS,
    LatestRunNotificationState, MonitoringSettings, NotificationStateRepository,
    PersistenceFailure, ProviderToken, RepositorySelection, RepositorySelectionRepository,
    SecretReference, SecretStore, SettingsRepository, SourceRepositorySelection, StoredConnection,
    ValidatedAccount,
};
use chacha20poly1305::aead::{Aead, Generate, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rusqlite::{Connection, OptionalExtension, params};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use zeroize::Zeroizing;

const MASTER_KEY_LENGTH: usize = 32;

#[derive(Clone)]
pub struct SqliteDatabase {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceFailure> {
        let connection = Connection::open(path).map_err(|_| PersistenceFailure)?;
        Self::initialize(connection)
    }

    pub fn in_memory() -> Result<Self, PersistenceFailure> {
        let connection = Connection::open_in_memory().map_err(|_| PersistenceFailure)?;
        Self::initialize(connection)
    }

    fn initialize(connection: Connection) -> Result<Self, PersistenceFailure> {
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE IF NOT EXISTS connections (
                   id TEXT PRIMARY KEY,
                   provider TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   login TEXT NOT NULL,
                   display_name TEXT,
                   profile_url TEXT NOT NULL,
                   secret_reference TEXT NOT NULL,
                   configuration TEXT NOT NULL DEFAULT '{}',
                   connection_key TEXT NOT NULL DEFAULT '',
                   connection_label TEXT NOT NULL DEFAULT '',
                   updated_at INTEGER NOT NULL DEFAULT (unixepoch())
                 );
                 CREATE TABLE IF NOT EXISTS encrypted_secrets (
                   reference TEXT PRIMARY KEY,
                   key_nonce BLOB NOT NULL,
                   wrapped_key BLOB NOT NULL,
                   value_nonce BLOB NOT NULL,
                   ciphertext BLOB NOT NULL,
                   updated_at INTEGER NOT NULL DEFAULT (unixepoch())
                 );
                 CREATE TABLE IF NOT EXISTS repository_selections (
                   source_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   selected_at INTEGER NOT NULL DEFAULT (unixepoch()),
                   PRIMARY KEY (source_id, repository_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS repository_cache_sync (
                   source_id TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   refreshed_at INTEGER NOT NULL,
                   PRIMARY KEY (source_id, account_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS repositories (
                   source_id TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   owner TEXT NOT NULL,
                   name TEXT NOT NULL,
                   description TEXT,
                   visibility TEXT NOT NULL,
                   web_url TEXT NOT NULL,
                   PRIMARY KEY (source_id, account_id, repository_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS workflow_cache_sync (
                   source_id TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   refreshed_at INTEGER NOT NULL,
                   PRIMARY KEY (source_id, account_id, repository_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS workflows (
                   source_id TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   workflow_id TEXT NOT NULL,
                   name TEXT NOT NULL,
                   path TEXT NOT NULL,
                   state TEXT NOT NULL,
                   web_url TEXT NOT NULL,
                   PRIMARY KEY (source_id, account_id, repository_id, workflow_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS workflow_run_cache_sync (
                   source_id TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   last_attempted_at INTEGER NOT NULL,
                   last_successful_at INTEGER,
                   last_error TEXT,
                   PRIMARY KEY (source_id, account_id, repository_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS workflow_runs (
                   source_id TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   run_id TEXT NOT NULL,
                   workflow_id TEXT NOT NULL,
                   run_number INTEGER NOT NULL,
                   attempt INTEGER NOT NULL,
                   title TEXT NOT NULL,
                   lifecycle TEXT NOT NULL,
                   outcome TEXT NOT NULL,
                   branch TEXT,
                   commit_sha TEXT NOT NULL,
                   actor TEXT,
                   trigger TEXT NOT NULL,
                   created_at TEXT NOT NULL,
                   started_at TEXT,
                   updated_at TEXT NOT NULL,
                   web_url TEXT NOT NULL,
                   provider_status TEXT NOT NULL,
                   provider_conclusion TEXT,
                   PRIMARY KEY (source_id, account_id, repository_id, run_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS change_request_cache_sync (
                   source_id TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   refreshed_at INTEGER NOT NULL,
                   PRIMARY KEY (source_id, account_id, repository_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS change_requests (
                   source_id TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   change_request_id TEXT NOT NULL,
                   number INTEGER NOT NULL,
                   title TEXT NOT NULL,
                   author TEXT,
                   source_branch TEXT NOT NULL,
                   target_branch TEXT NOT NULL,
                   state TEXT NOT NULL,
                   draft INTEGER NOT NULL,
                   review_status TEXT NOT NULL,
                   check_status TEXT NOT NULL,
                   merge_status TEXT NOT NULL,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   web_url TEXT NOT NULL,
                   PRIMARY KEY (source_id, account_id, repository_id, change_request_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS change_request_details (
                   source_id TEXT NOT NULL,
                   account_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   number INTEGER NOT NULL,
                   refreshed_at INTEGER NOT NULL,
                   payload TEXT NOT NULL,
                   PRIMARY KEY (source_id, account_id, repository_id, number),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS change_request_activity_states (
                   source_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   payload TEXT NOT NULL,
                   observed_at INTEGER NOT NULL DEFAULT (unixepoch()),
                   PRIMARY KEY (source_id, repository_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS change_request_activity_events (
                   id TEXT PRIMARY KEY,
                   source_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   occurred_at TEXT NOT NULL,
                   payload TEXT NOT NULL,
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS change_request_activity_occurred_at
                   ON change_request_activity_events(occurred_at DESC);
                 CREATE TABLE IF NOT EXISTS audit_events (
                   id INTEGER PRIMARY KEY AUTOINCREMENT,
                   event_type TEXT NOT NULL,
                   connection_id TEXT NOT NULL,
                   occurred_at INTEGER NOT NULL DEFAULT (unixepoch())
                 );
                 CREATE TABLE IF NOT EXISTS monitoring_settings (
                   id INTEGER PRIMARY KEY CHECK (id = 1),
                   synchronization_interval_seconds INTEGER NOT NULL,
                   recent_runs_per_workflow INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS notification_repository_states (
                   source_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   observed_at INTEGER NOT NULL DEFAULT (unixepoch()),
                   PRIMARY KEY (source_id, repository_id),
                   FOREIGN KEY (source_id) REFERENCES connections(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS notification_workflow_states (
                   source_id TEXT NOT NULL,
                   repository_id TEXT NOT NULL,
                   workflow_id TEXT NOT NULL,
                   run_id TEXT,
                   attempt INTEGER,
                   failed INTEGER NOT NULL,
                   PRIMARY KEY (source_id, repository_id, workflow_id),
                   FOREIGN KEY (source_id, repository_id)
                     REFERENCES notification_repository_states(source_id, repository_id)
                     ON DELETE CASCADE
                 );
                 ",
            )
            .map_err(|_| PersistenceFailure)?;
        ensure_connection_column(&connection, "configuration", "TEXT NOT NULL DEFAULT '{}'")?;
        ensure_connection_column(&connection, "connection_key", "TEXT NOT NULL DEFAULT ''")?;
        ensure_connection_column(&connection, "connection_label", "TEXT NOT NULL DEFAULT ''")?;
        connection
            .execute_batch(
                "UPDATE connections
                   SET configuration = CASE
                         WHEN provider = 'github' THEN '{\"serverUrl\":\"https://github.com\"}'
                         ELSE '{}'
                       END,
                       connection_key = CASE
                         WHEN provider = 'github' THEN 'https://github.com'
                         ELSE id
                       END,
                       connection_label = CASE
                         WHEN provider = 'github' THEN 'GitHub.com'
                         ELSE provider
                       END
                 WHERE connection_key = '';
                 CREATE UNIQUE INDEX IF NOT EXISTS connections_provider_key
                   ON connections(provider, connection_key);",
            )
            .map_err(|_| PersistenceFailure)?;
        connection
            .execute(
                "INSERT OR IGNORE INTO monitoring_settings (
                   id, synchronization_interval_seconds, recent_runs_per_workflow
                 ) VALUES (1, ?1, ?2)",
                params![
                    i64::try_from(DEFAULT_SYNCHRONIZATION_INTERVAL_SECONDS)
                        .map_err(|_| PersistenceFailure)?,
                    i64::try_from(DEFAULT_RECENT_RUNS_PER_WORKFLOW)
                        .map_err(|_| PersistenceFailure)?,
                ],
            )
            .map_err(|_| PersistenceFailure)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, PersistenceFailure> {
        self.connection.lock().map_err(|_| PersistenceFailure)
    }

    fn store_encrypted_secret(
        &self,
        reference: &SecretReference,
        key_nonce: &[u8],
        wrapped_key: &[u8],
        value_nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<(), PersistenceFailure> {
        self.lock()?
            .execute(
                "INSERT INTO encrypted_secrets (
                   reference, key_nonce, wrapped_key, value_nonce, ciphertext, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, unixepoch())
                 ON CONFLICT(reference) DO UPDATE SET
                   key_nonce = excluded.key_nonce,
                   wrapped_key = excluded.wrapped_key,
                   value_nonce = excluded.value_nonce,
                   ciphertext = excluded.ciphertext,
                   updated_at = unixepoch()",
                params![
                    reference.expose(),
                    key_nonce,
                    wrapped_key,
                    value_nonce,
                    ciphertext
                ],
            )
            .map(|_| ())
            .map_err(|_| PersistenceFailure)
    }

    fn encrypted_secret(
        &self,
        reference: &SecretReference,
    ) -> Result<Option<EncryptedSecret>, PersistenceFailure> {
        self.lock()?
            .query_row(
                "SELECT key_nonce, wrapped_key, value_nonce, ciphertext
                 FROM encrypted_secrets WHERE reference = ?1",
                [reference.expose()],
                |row| {
                    Ok(EncryptedSecret {
                        key_nonce: row.get(0)?,
                        wrapped_key: row.get(1)?,
                        value_nonce: row.get(2)?,
                        ciphertext: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|_| PersistenceFailure)
    }

    #[cfg(test)]
    fn encrypted_bytes(&self, reference: &SecretReference) -> Result<Vec<u8>, PersistenceFailure> {
        let secret = self
            .encrypted_secret(reference)?
            .ok_or(PersistenceFailure)?;
        Ok([
            secret.key_nonce,
            secret.wrapped_key,
            secret.value_nonce,
            secret.ciphertext,
        ]
        .concat())
    }

    #[cfg(test)]
    fn audit_event_count(&self) -> Result<i64, PersistenceFailure> {
        self.lock()?
            .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
            .map_err(|_| PersistenceFailure)
    }
}

fn ensure_connection_column(
    connection: &Connection,
    name: &str,
    definition: &str,
) -> Result<(), PersistenceFailure> {
    let exists = connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM pragma_table_info('connections') WHERE name = ?1
             )",
            [name],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|_| PersistenceFailure)?;
    if !exists {
        connection
            .execute_batch(&format!(
                "ALTER TABLE connections ADD COLUMN {name} {definition}"
            ))
            .map_err(|_| PersistenceFailure)?;
    }
    Ok(())
}

impl ConnectionRepository for SqliteDatabase {
    fn save(&self, connection: &StoredConnection) -> Result<(), PersistenceFailure> {
        let configuration =
            serde_json::to_string(&connection.configuration).map_err(|_| PersistenceFailure)?;
        let mut database = self.lock()?;
        let transaction = database.transaction().map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "INSERT INTO connections (
                   id, provider, account_id, login, display_name, profile_url,
                   secret_reference, configuration, connection_key, connection_label, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, unixepoch())
                 ON CONFLICT(id) DO UPDATE SET
                   provider = excluded.provider,
                   account_id = excluded.account_id,
                   login = excluded.login,
                   display_name = excluded.display_name,
                   profile_url = excluded.profile_url,
                   secret_reference = excluded.secret_reference,
                   configuration = excluded.configuration,
                   connection_key = excluded.connection_key,
                   connection_label = excluded.connection_label,
                   updated_at = unixepoch()",
                params![
                    connection.id,
                    connection.source_id,
                    connection.account.external_id,
                    connection.account.handle.as_deref().unwrap_or(""),
                    connection.account.name,
                    connection.account.profile_url.as_deref().unwrap_or(""),
                    connection.secret_reference.expose(),
                    configuration,
                    connection.unique_key,
                    connection.label,
                ],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "INSERT INTO audit_events (event_type, connection_id)
                 VALUES ('connection_saved', ?1)",
                [&connection.id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction.commit().map_err(|_| PersistenceFailure)
    }

    fn get(&self, connection_id: &str) -> Result<Option<StoredConnection>, PersistenceFailure> {
        self.lock()?
            .query_row(
                "SELECT id, provider, connection_key, connection_label, configuration,
                        account_id, login, display_name, profile_url, secret_reference
                 FROM connections WHERE id = ?1",
                [connection_id],
                stored_connection,
            )
            .optional()
            .map_err(|_| PersistenceFailure)
    }

    fn find(
        &self,
        source_id: &str,
        unique_key: &str,
    ) -> Result<Option<StoredConnection>, PersistenceFailure> {
        self.lock()?
            .query_row(
                "SELECT id, provider, connection_key, connection_label, configuration,
                        account_id, login, display_name, profile_url, secret_reference
                 FROM connections WHERE provider = ?1 AND connection_key = ?2",
                params![source_id, unique_key],
                stored_connection,
            )
            .optional()
            .map_err(|_| PersistenceFailure)
    }

    fn list(&self) -> Result<Vec<StoredConnection>, PersistenceFailure> {
        let database = self.lock()?;
        let mut statement = database
            .prepare(
                "SELECT id, provider, connection_key, connection_label, configuration,
                        account_id, login, display_name, profile_url, secret_reference
                 FROM connections ORDER BY provider, connection_label, id",
            )
            .map_err(|_| PersistenceFailure)?;
        let rows = statement
            .query_map([], stored_connection)
            .map_err(|_| PersistenceFailure)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| PersistenceFailure)
    }

    fn delete(&self, connection_id: &str) -> Result<(), PersistenceFailure> {
        let mut database = self.lock()?;
        let transaction = database.transaction().map_err(|_| PersistenceFailure)?;
        transaction
            .execute("DELETE FROM connections WHERE id = ?1", [connection_id])
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "INSERT INTO audit_events (event_type, connection_id)
                 VALUES ('connection_deleted', ?1)",
                [connection_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction.commit().map_err(|_| PersistenceFailure)
    }
}

fn stored_connection(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredConnection> {
    let configuration_json: String = row.get(4)?;
    let configuration = serde_json::from_str(&configuration_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let handle: String = row.get(6)?;
    let profile_url: String = row.get(8)?;
    Ok(StoredConnection {
        id: row.get(0)?,
        source_id: row.get(1)?,
        unique_key: row.get(2)?,
        label: row.get(3)?,
        configuration,
        account: ValidatedAccount {
            external_id: row.get(5)?,
            handle: (!handle.is_empty()).then_some(handle),
            name: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
            profile_url: (!profile_url.is_empty()).then_some(profile_url),
        },
        secret_reference: SecretReference::from_stored(row.get(9)?),
    })
}

impl RepositorySelectionRepository for SqliteDatabase {
    fn list(&self) -> Result<Vec<RepositorySelection>, PersistenceFailure> {
        let database = self.lock()?;
        let mut statement = database
            .prepare(
                "SELECT source_id, repository_id
                 FROM repository_selections
                 ORDER BY source_id, repository_id",
            )
            .map_err(|_| PersistenceFailure)?;
        let rows = statement
            .query_map([], |row| {
                Ok(RepositorySelection {
                    source_id: row.get(0)?,
                    repository_id: row.get(1)?,
                })
            })
            .map_err(|_| PersistenceFailure)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| PersistenceFailure)
    }

    fn replace_for_sources(
        &self,
        selections: &[SourceRepositorySelection],
    ) -> Result<(), PersistenceFailure> {
        let mut database = self.lock()?;
        let transaction = database.transaction().map_err(|_| PersistenceFailure)?;
        for selection in selections {
            transaction
                .execute(
                    "DELETE FROM repository_selections WHERE source_id = ?1",
                    [&selection.source_id],
                )
                .map_err(|_| PersistenceFailure)?;
            for repository_id in &selection.repository_ids {
                transaction
                    .execute(
                        "INSERT INTO repository_selections (source_id, repository_id)
                         VALUES (?1, ?2)",
                        params![selection.source_id, repository_id],
                    )
                    .map_err(|_| PersistenceFailure)?;
            }
            transaction
                .execute(
                    "INSERT INTO audit_events (event_type, connection_id)
                     VALUES ('repository_selection_saved', ?1)",
                    [&selection.source_id],
                )
                .map_err(|_| PersistenceFailure)?;
        }
        transaction.commit().map_err(|_| PersistenceFailure)
    }
}

impl SettingsRepository for SqliteDatabase {
    fn load_settings(&self) -> Result<MonitoringSettings, PersistenceFailure> {
        let (synchronization_interval_seconds, recent_runs_per_workflow) = self
            .lock()?
            .query_row(
                "SELECT synchronization_interval_seconds, recent_runs_per_workflow
                 FROM monitoring_settings WHERE id = 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .map_err(|_| PersistenceFailure)?;
        Ok(MonitoringSettings {
            synchronization_interval_seconds: synchronization_interval_seconds
                .try_into()
                .map_err(|_| PersistenceFailure)?,
            recent_runs_per_workflow: recent_runs_per_workflow
                .try_into()
                .map_err(|_| PersistenceFailure)?,
        })
    }

    fn save_settings(&self, settings: MonitoringSettings) -> Result<(), PersistenceFailure> {
        let synchronization_interval_seconds: i64 = settings
            .synchronization_interval_seconds
            .try_into()
            .map_err(|_| PersistenceFailure)?;
        let recent_runs_per_workflow: i64 = settings
            .recent_runs_per_workflow
            .try_into()
            .map_err(|_| PersistenceFailure)?;
        self.lock()?
            .execute(
                "UPDATE monitoring_settings SET
                   synchronization_interval_seconds = ?1,
                   recent_runs_per_workflow = ?2
                 WHERE id = 1",
                params![synchronization_interval_seconds, recent_runs_per_workflow,],
            )
            .map(|_| ())
            .map_err(|_| PersistenceFailure)
    }
}

impl ActivityEventRepository for SqliteDatabase {
    fn load_change_request_state(
        &self,
        source_id: &str,
        repository_id: &str,
    ) -> Result<Option<Vec<crate::domain::ChangeRequest>>, PersistenceFailure> {
        self.lock()?
            .query_row(
                "SELECT payload FROM change_request_activity_states
                 WHERE source_id = ?1 AND repository_id = ?2",
                params![source_id, repository_id],
                |row| {
                    let payload: String = row.get(0)?;
                    serde_json::from_str(&payload).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })
                },
            )
            .optional()
            .map_err(|_| PersistenceFailure)
    }

    fn save_change_request_observation(
        &self,
        source_id: &str,
        repository_id: &str,
        observed: &[crate::domain::ChangeRequest],
        events: &[ChangeRequestActivityEvent],
    ) -> Result<(), PersistenceFailure> {
        let mut database = self.lock()?;
        let transaction = database.transaction().map_err(|_| PersistenceFailure)?;
        let observed = serde_json::to_string(observed).map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "INSERT INTO change_request_activity_states (
                   source_id, repository_id, payload, observed_at
                 ) VALUES (?1, ?2, ?3, unixepoch())
                 ON CONFLICT(source_id, repository_id) DO UPDATE SET
                   payload = excluded.payload,
                   observed_at = unixepoch()",
                params![source_id, repository_id, observed],
            )
            .map_err(|_| PersistenceFailure)?;
        for event in events {
            let payload = serde_json::to_string(event).map_err(|_| PersistenceFailure)?;
            transaction
                .execute(
                    "INSERT OR IGNORE INTO change_request_activity_events (
                       id, source_id, repository_id, occurred_at, payload
                     ) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        event.id,
                        source_id,
                        repository_id,
                        event.occurred_at,
                        payload
                    ],
                )
                .map_err(|_| PersistenceFailure)?;
        }
        transaction
            .execute(
                "DELETE FROM change_request_activity_events
                 WHERE id NOT IN (
                   SELECT id FROM change_request_activity_events
                   ORDER BY occurred_at DESC, id DESC LIMIT 5000
                 )",
                [],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction.commit().map_err(|_| PersistenceFailure)
    }

    fn list_change_request_events(
        &self,
    ) -> Result<Vec<ChangeRequestActivityEvent>, PersistenceFailure> {
        let database = self.lock()?;
        let mut statement = database
            .prepare(
                "SELECT payload FROM change_request_activity_events
                 ORDER BY occurred_at DESC, id DESC LIMIT 5000",
            )
            .map_err(|_| PersistenceFailure)?;
        let rows = statement
            .query_map([], |row| {
                let payload: String = row.get(0)?;
                serde_json::from_str(&payload).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            })
            .map_err(|_| PersistenceFailure)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| PersistenceFailure)
    }
}

impl NotificationStateRepository for SqliteDatabase {
    fn load(
        &self,
        source_id: &str,
        repository_id: &str,
    ) -> Result<Option<Vec<LatestRunNotificationState>>, PersistenceFailure> {
        let database = self.lock()?;
        let initialized = database
            .query_row(
                "SELECT 1 FROM notification_repository_states
                 WHERE source_id = ?1 AND repository_id = ?2",
                params![source_id, repository_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|_| PersistenceFailure)?
            .is_some();
        if !initialized {
            return Ok(None);
        }

        let mut statement = database
            .prepare(
                "SELECT workflow_id, run_id, attempt, failed
                 FROM notification_workflow_states
                 WHERE source_id = ?1 AND repository_id = ?2
                 ORDER BY workflow_id",
            )
            .map_err(|_| PersistenceFailure)?;
        let rows = statement
            .query_map(params![source_id, repository_id], |row| {
                let attempt = row
                    .get::<_, Option<i64>>(2)?
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })?;
                Ok(LatestRunNotificationState {
                    workflow_id: row.get(0)?,
                    run_id: row.get(1)?,
                    attempt,
                    failed: row.get::<_, i64>(3)? != 0,
                })
            })
            .map_err(|_| PersistenceFailure)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map(Some)
            .map_err(|_| PersistenceFailure)
    }

    fn replace(
        &self,
        source_id: &str,
        repository_id: &str,
        states: &[LatestRunNotificationState],
    ) -> Result<(), PersistenceFailure> {
        let mut database = self.lock()?;
        let transaction = database.transaction().map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "INSERT INTO notification_repository_states (
                   source_id, repository_id, observed_at
                 ) VALUES (?1, ?2, unixepoch())
                 ON CONFLICT(source_id, repository_id) DO UPDATE SET
                   observed_at = unixepoch()",
                params![source_id, repository_id],
            )
            .map_err(|_| PersistenceFailure)?;
        transaction
            .execute(
                "DELETE FROM notification_workflow_states
                 WHERE source_id = ?1 AND repository_id = ?2",
                params![source_id, repository_id],
            )
            .map_err(|_| PersistenceFailure)?;
        for state in states {
            let attempt = state
                .attempt
                .map(i64::try_from)
                .transpose()
                .map_err(|_| PersistenceFailure)?;
            transaction
                .execute(
                    "INSERT INTO notification_workflow_states (
                       source_id, repository_id, workflow_id, run_id, attempt, failed
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        source_id,
                        repository_id,
                        state.workflow_id,
                        state.run_id,
                        attempt,
                        i64::from(state.failed),
                    ],
                )
                .map_err(|_| PersistenceFailure)?;
        }
        transaction.commit().map_err(|_| PersistenceFailure)
    }
}

struct EncryptedSecret {
    key_nonce: Vec<u8>,
    wrapped_key: Vec<u8>,
    value_nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

pub struct ServerMasterKey(Zeroizing<[u8; MASTER_KEY_LENGTH]>);

impl ServerMasterKey {
    pub fn generate() -> Result<Self, PersistenceFailure> {
        let mut key = Zeroizing::new([0_u8; MASTER_KEY_LENGTH]);
        getrandom::fill(&mut key[..]).map_err(|_| PersistenceFailure)?;
        Ok(Self(key))
    }

    pub fn load_or_create(path: impl AsRef<Path>) -> Result<Self, PersistenceFailure> {
        let path = path.as_ref();
        match fs::read(path) {
            Ok(bytes) => {
                validate_master_key_permissions(path)?;
                Self::from_bytes(&bytes)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                let key = Self::generate()?;
                write_new_master_key(path, &key.0[..])?;
                Ok(key)
            }
            Err(_) => Err(PersistenceFailure),
        }
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, PersistenceFailure> {
        if bytes.len() != MASTER_KEY_LENGTH {
            return Err(PersistenceFailure);
        }
        let mut key = Zeroizing::new([0_u8; MASTER_KEY_LENGTH]);
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }
}

#[cfg(unix)]
fn validate_master_key_permissions(path: &Path) -> Result<(), PersistenceFailure> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .map_err(|_| PersistenceFailure)?
        .permissions()
        .mode();
    if mode & 0o077 == 0 {
        Ok(())
    } else {
        Err(PersistenceFailure)
    }
}

#[cfg(not(unix))]
fn validate_master_key_permissions(_path: &Path) -> Result<(), PersistenceFailure> {
    Ok(())
}

fn write_new_master_key(path: &Path, bytes: &[u8]) -> Result<(), PersistenceFailure> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| PersistenceFailure)?;
    }

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|_| PersistenceFailure)?;
    file.write_all(bytes).map_err(|_| PersistenceFailure)?;
    file.sync_all().map_err(|_| PersistenceFailure)
}

#[derive(Clone)]
pub struct EncryptedSecretStore {
    database: SqliteDatabase,
    master_key: Arc<ServerMasterKey>,
}

impl EncryptedSecretStore {
    #[must_use]
    pub fn new(database: SqliteDatabase, master_key: ServerMasterKey) -> Self {
        Self {
            database,
            master_key: Arc::new(master_key),
        }
    }
}

impl SecretStore for EncryptedSecretStore {
    fn store(
        &self,
        reference: &SecretReference,
        token: &ProviderToken,
    ) -> Result<(), PersistenceFailure> {
        let mut data_key = Zeroizing::new([0_u8; MASTER_KEY_LENGTH]);
        getrandom::fill(&mut data_key[..]).map_err(|_| PersistenceFailure)?;
        let value_nonce = XNonce::generate();
        let value_cipher =
            XChaCha20Poly1305::new_from_slice(&data_key[..]).map_err(|_| PersistenceFailure)?;
        let ciphertext = value_cipher
            .encrypt(
                &value_nonce,
                Payload {
                    msg: token.expose().as_bytes(),
                    aad: reference.expose().as_bytes(),
                },
            )
            .map_err(|_| PersistenceFailure)?;

        let key_nonce = XNonce::generate();
        let master_cipher = XChaCha20Poly1305::new_from_slice(&self.master_key.0[..])
            .map_err(|_| PersistenceFailure)?;
        let wrapped_key = master_cipher
            .encrypt(
                &key_nonce,
                Payload {
                    msg: &data_key[..],
                    aad: reference.expose().as_bytes(),
                },
            )
            .map_err(|_| PersistenceFailure)?;

        self.database.store_encrypted_secret(
            reference,
            &key_nonce,
            &wrapped_key,
            &value_nonce,
            &ciphertext,
        )
    }

    fn retrieve(&self, reference: &SecretReference) -> Result<ProviderToken, PersistenceFailure> {
        let encrypted = self
            .database
            .encrypted_secret(reference)?
            .ok_or(PersistenceFailure)?;
        let key_nonce =
            XNonce::try_from(encrypted.key_nonce.as_slice()).map_err(|_| PersistenceFailure)?;
        let master_cipher = XChaCha20Poly1305::new_from_slice(&self.master_key.0[..])
            .map_err(|_| PersistenceFailure)?;
        let data_key = Zeroizing::new(
            master_cipher
                .decrypt(
                    &key_nonce,
                    Payload {
                        msg: &encrypted.wrapped_key,
                        aad: reference.expose().as_bytes(),
                    },
                )
                .map_err(|_| PersistenceFailure)?,
        );
        let value_cipher =
            XChaCha20Poly1305::new_from_slice(&data_key).map_err(|_| PersistenceFailure)?;
        let value_nonce =
            XNonce::try_from(encrypted.value_nonce.as_slice()).map_err(|_| PersistenceFailure)?;
        let plaintext = Zeroizing::new(
            value_cipher
                .decrypt(
                    &value_nonce,
                    Payload {
                        msg: &encrypted.ciphertext,
                        aad: reference.expose().as_bytes(),
                    },
                )
                .map_err(|_| PersistenceFailure)?,
        );
        let token = String::from_utf8(plaintext.to_vec()).map_err(|_| PersistenceFailure)?;
        Ok(ProviderToken::new(token))
    }

    fn delete(&self, reference: &SecretReference) -> Result<(), PersistenceFailure> {
        self.database
            .lock()?
            .execute(
                "DELETE FROM encrypted_secrets WHERE reference = ?1",
                [reference.expose()],
            )
            .map(|_| ())
            .map_err(|_| PersistenceFailure)
    }
}

#[cfg(test)]
mod tests;
