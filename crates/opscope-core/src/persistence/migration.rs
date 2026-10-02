use crate::application::PersistenceFailure;
use std::fs;
use std::path::{Path, PathBuf};

const DATABASE_NAME: &str = "opscope.sqlite3";
const LEGACY_DATABASE_NAME: &str = "opsscope.sqlite3";

/// Adopt storage from the former application name without replacing existing data.
pub fn prepare_database_path(
    directory: &Path,
    legacy_directory: Option<&Path>,
) -> Result<PathBuf, PersistenceFailure> {
    if let Some(legacy) = legacy_directory
        && legacy != directory
        && legacy.try_exists().map_err(|_| PersistenceFailure)?
    {
        let directory_exists = directory.try_exists().map_err(|_| PersistenceFailure)?;
        if !directory_exists
            || fs::read_dir(directory)
                .map_err(|_| PersistenceFailure)?
                .next()
                .is_none()
        {
            if directory_exists {
                fs::remove_dir(directory).map_err(|_| PersistenceFailure)?;
            }
            fs::rename(legacy, directory).map_err(|_| PersistenceFailure)?;
        }
    }
    fs::create_dir_all(directory).map_err(|_| PersistenceFailure)?;
    let database_path = directory.join(DATABASE_NAME);
    if database_path.try_exists().map_err(|_| PersistenceFailure)? {
        return Ok(database_path);
    }

    let legacy_path = directory.join(LEGACY_DATABASE_NAME);
    if legacy_path.try_exists().map_err(|_| PersistenceFailure)? {
        let files = ["-wal", "-shm", ""].map(|suffix| {
            (
                directory.join(format!("{LEGACY_DATABASE_NAME}{suffix}")),
                directory.join(format!("{DATABASE_NAME}{suffix}")),
            )
        });
        for (old, new) in &files {
            if old.try_exists().map_err(|_| PersistenceFailure)?
                && new.try_exists().map_err(|_| PersistenceFailure)?
            {
                return Err(PersistenceFailure);
            }
        }
        // Move SQLite recovery files before the main database so an interrupted
        // migration can finish before the renamed database is opened.
        for (old, new) in files {
            if old.try_exists().map_err(|_| PersistenceFailure)? {
                fs::rename(old, new).map_err(|_| PersistenceFailure)?;
            }
        }
    }
    Ok(database_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{ProviderToken, SecretReference, SecretStore, SettingsRepository};
    use crate::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};

    #[test]
    fn adopts_existing_database_settings_and_master_key() {
        let temporary = tempfile::tempdir().unwrap();
        let old = temporary.path().join("old");
        let new = temporary.path().join("new");
        fs::create_dir(&old).unwrap();
        let database = SqliteDatabase::open(old.join(LEGACY_DATABASE_NAME)).unwrap();
        let mut settings = database.load_settings().unwrap();
        settings.recent_runs_per_workflow = 15;
        database.save_settings(settings).unwrap();
        let reference = SecretReference::for_connection("example", "test");
        let key = ServerMasterKey::load_or_create(old.join("master.key")).unwrap();
        let store = EncryptedSecretStore::new(database, key);
        store
            .store(
                &reference,
                &ProviderToken::new("migration-test-token".to_owned()),
            )
            .unwrap();
        drop(store);
        let key_bytes = fs::read(old.join("master.key")).unwrap();

        let path = prepare_database_path(&new, Some(&old)).unwrap();
        assert_eq!(path, new.join(DATABASE_NAME));
        assert!(!old.exists());
        assert_eq!(fs::read(new.join("master.key")).unwrap(), key_bytes);
        let database = SqliteDatabase::open(&path).unwrap();
        assert_eq!(
            database.load_settings().unwrap().recent_runs_per_workflow,
            15
        );
        let store = EncryptedSecretStore::new(
            database,
            ServerMasterKey::load_or_create(new.join("master.key")).unwrap(),
        );
        assert_eq!(
            store.retrieve(&reference).unwrap().expose(),
            "migration-test-token"
        );
        assert_eq!(prepare_database_path(&new, Some(&old)).unwrap(), path);
    }

    #[test]
    fn adopts_legacy_storage_when_the_new_directory_is_empty() {
        let temporary = tempfile::tempdir().unwrap();
        let old = temporary.path().join("old");
        let new = temporary.path().join("new");
        fs::create_dir(&old).unwrap();
        fs::create_dir(&new).unwrap();
        fs::write(old.join(LEGACY_DATABASE_NAME), "legacy").unwrap();
        let path = prepare_database_path(&new, Some(&old)).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "legacy");
        assert!(!old.exists());
    }

    #[test]
    fn moves_recovery_files_and_resumes_an_interrupted_rename() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join(LEGACY_DATABASE_NAME), "database").unwrap();
        fs::write(directory.path().join(format!("{DATABASE_NAME}-wal")), "wal").unwrap();
        fs::write(
            directory.path().join(format!("{LEGACY_DATABASE_NAME}-shm")),
            "shm",
        )
        .unwrap();
        prepare_database_path(directory.path(), None).unwrap();
        for (suffix, contents) in [("", "database"), ("-wal", "wal"), ("-shm", "shm")] {
            assert_eq!(
                fs::read_to_string(directory.path().join(format!("{DATABASE_NAME}{suffix}")))
                    .unwrap(),
                contents
            );
        }
    }

    #[test]
    fn never_overwrites_a_current_database_or_conflicting_recovery_file() {
        let directory = tempfile::tempdir().unwrap();
        let current = directory.path().join(DATABASE_NAME);
        let legacy = directory.path().join(LEGACY_DATABASE_NAME);
        fs::write(&current, "current").unwrap();
        fs::write(&legacy, "legacy").unwrap();
        assert_eq!(
            prepare_database_path(directory.path(), None).unwrap(),
            current
        );
        assert_eq!(fs::read_to_string(&current).unwrap(), "current");
        assert_eq!(fs::read_to_string(&legacy).unwrap(), "legacy");

        fs::remove_file(&current).unwrap();
        fs::write(
            directory.path().join(format!("{DATABASE_NAME}-wal")),
            "current wal",
        )
        .unwrap();
        fs::write(
            directory.path().join(format!("{LEGACY_DATABASE_NAME}-wal")),
            "legacy wal",
        )
        .unwrap();
        assert!(prepare_database_path(directory.path(), None).is_err());
        assert_eq!(fs::read_to_string(legacy).unwrap(), "legacy");
    }
}
