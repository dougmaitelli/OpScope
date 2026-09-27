use super::*;
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
fn connection_metadata_round_trips_through_sqlite() -> Result<(), Box<dyn std::error::Error>> {
    let database = SqliteDatabase::in_memory()?;
    let expected = test_connection();
    database.save(&expected)?;
    assert_eq!(database.get("example")?, Some(expected));
    assert_eq!(database.audit_event_count()?, 1);
    Ok(())
}

#[test]
fn connection_keys_are_unique_within_a_source_module() -> Result<(), Box<dyn std::error::Error>> {
    let database = SqliteDatabase::in_memory()?;
    let first = test_connection();
    let mut second = test_connection();
    second.id = "enterprise".to_owned();
    second.unique_key = "https://github.example.com".to_owned();
    second.label = "github.example.com".to_owned();
    database.save(&first)?;
    database.save(&second)?;

    assert_eq!(ConnectionRepository::list(&database)?.len(), 2);
    assert_eq!(
        database
            .find("example", "https://github.example.com")?
            .map(|connection| connection.id),
        Some("enterprise".to_owned())
    );

    let mut duplicate = second;
    duplicate.id = "duplicate".to_owned();
    assert!(database.save(&duplicate).is_err());
    Ok(())
}

#[test]
fn legacy_github_connection_is_migrated_to_the_default_server()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let database_path = directory.path().join("legacy.sqlite3");
    let legacy = Connection::open(&database_path)?;
    legacy.execute_batch(
        "CREATE TABLE connections (
               id TEXT PRIMARY KEY,
               provider TEXT NOT NULL,
               account_id TEXT NOT NULL,
               login TEXT NOT NULL,
               display_name TEXT,
               profile_url TEXT NOT NULL,
               secret_reference TEXT NOT NULL,
               updated_at INTEGER NOT NULL DEFAULT (unixepoch())
             );
             INSERT INTO connections (
               id, provider, account_id, login, display_name, profile_url, secret_reference
             ) VALUES (
               'github', 'github', '42', 'octocat', 'The Octocat',
               'https://github.com/octocat', 'github:42'
             );",
    )?;
    drop(legacy);

    let database = SqliteDatabase::open(&database_path)?;
    let migrated = database.get("github")?.expect("migrated connection");
    assert_eq!(migrated.unique_key, "https://github.com");
    assert_eq!(migrated.label, "GitHub.com");
    assert_eq!(
        migrated.configuration.get("serverUrl").map(String::as_str),
        Some("https://github.com")
    );
    Ok(())
}

#[test]
fn encrypted_secret_round_trips_without_plaintext_storage() -> Result<(), Box<dyn std::error::Error>>
{
    let database = SqliteDatabase::in_memory()?;
    let store = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
    let reference = SecretReference::for_connection("example", "test");
    let plaintext = "github_pat_super_secret";

    store.store(&reference, &ProviderToken::new(plaintext.to_owned()))?;
    let retrieved = store.retrieve(&reference)?;

    assert_eq!(retrieved.expose(), plaintext);
    assert!(
        !database
            .encrypted_bytes(&reference)?
            .windows(plaintext.len())
            .any(|window| window == plaintext.as_bytes())
    );
    Ok(())
}

#[test]
fn encrypted_secret_survives_database_and_key_reload() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let database_path = directory.path().join("opsscope.sqlite3");
    let key_path = directory.path().join("master.key");
    let reference = SecretReference::for_connection("example", "test");

    {
        let database = SqliteDatabase::open(&database_path)?;
        let key = ServerMasterKey::load_or_create(&key_path)?;
        let store = EncryptedSecretStore::new(database, key);
        store.store(
            &reference,
            &ProviderToken::new("github_pat_restart_test".to_owned()),
        )?;
    }

    let database = SqliteDatabase::open(&database_path)?;
    let key = ServerMasterKey::load_or_create(&key_path)?;
    let store = EncryptedSecretStore::new(database, key);
    assert_eq!(
        store.retrieve(&reference)?.expose(),
        "github_pat_restart_test"
    );
    Ok(())
}

#[test]
fn repository_selection_round_trips_and_is_removed_with_connection()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let database_path = directory.path().join("opsscope.sqlite3");
    {
        let database = SqliteDatabase::open(&database_path)?;
        database.save(&test_connection())?;
        database.replace_for_sources(&[SourceRepositorySelection {
            source_id: "example".to_owned(),
            repository_ids: vec!["repository-1".to_owned(), "repository-2".to_owned()],
        }])?;
    }

    let database = SqliteDatabase::open(&database_path)?;
    assert_eq!(RepositorySelectionRepository::list(&database)?.len(), 2);
    database.delete("example")?;
    assert!(RepositorySelectionRepository::list(&database)?.is_empty());
    Ok(())
}

#[test]
fn monitoring_settings_have_defaults_and_persist_updates() -> Result<(), Box<dyn std::error::Error>>
{
    let database = SqliteDatabase::in_memory()?;
    assert_eq!(
        SettingsRepository::load_settings(&database)?,
        MonitoringSettings::default()
    );

    let updated = MonitoringSettings {
        synchronization_interval_seconds: 120,
        recent_runs_per_workflow: 25,
    };
    database.save_settings(updated)?;

    assert_eq!(SettingsRepository::load_settings(&database)?, updated);
    Ok(())
}

#[test]
fn notification_state_distinguishes_an_empty_baseline_and_replaces_workflows()
-> Result<(), Box<dyn std::error::Error>> {
    let database = SqliteDatabase::in_memory()?;
    database.save(&test_connection())?;
    assert!(NotificationStateRepository::load(&database, "example", "repository-1")?.is_none());

    NotificationStateRepository::replace(&database, "example", "repository-1", &[])?;
    assert_eq!(
        NotificationStateRepository::load(&database, "example", "repository-1")?,
        Some(Vec::new())
    );

    let expected = vec![LatestRunNotificationState {
        workflow_id: "workflow-1".to_owned(),
        run_id: Some("run-1".to_owned()),
        attempt: Some(2),
        failed: true,
    }];
    NotificationStateRepository::replace(&database, "example", "repository-1", &expected)?;
    assert_eq!(
        NotificationStateRepository::load(&database, "example", "repository-1")?,
        Some(expected)
    );
    Ok(())
}
