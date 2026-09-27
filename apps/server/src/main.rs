use ciwatcher_core::application::{
    DEFAULT_SYNCHRONIZATION_INTERVAL, GetMonitoringSettings, NotifyRepositoryFailures,
    SynchronizeSources,
};
use ciwatcher_core::integrations::registered_sources;
use ciwatcher_core::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:4317").await?;
    let authentication = ciwatcher_server::auth::WebAuthentication::from_environment().await?;
    if !authentication.is_enabled() {
        eprintln!("OIDC authentication is disabled; access must be restricted by the deployment");
    }
    let sources = registered_sources()?;
    let data_dir = env::var_os("CIWATCHER_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".ciwatcher-data"));
    fs::create_dir_all(&data_dir)?;
    let database = SqliteDatabase::open(data_dir.join("ciwatcher.sqlite3"))?;
    let master_key_path = env::var_os("CIWATCHER_MASTER_KEY_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("master.key"));
    let master_key = ServerMasterKey::load_or_create(master_key_path)?;
    let secrets = EncryptedSecretStore::new(database.clone(), master_key);
    let database = Arc::new(database);
    let failure_notifications = NotifyRepositoryFailures::new(
        database.clone(),
        ciwatcher_server::notifications::notification_sink_from_environment()?,
    );
    let application = ciwatcher_server::application(ciwatcher_server::ServerDependencies {
        registry: sources,
        connections: database.clone(),
        secrets: Arc::new(secrets),
        repository_selections: database.clone(),
        source_data_cache: database.clone(),
        settings: database,
        failure_notifications,
        authentication,
    });
    let scheduler = tokio::spawn(run_synchronization_schedule(
        application.synchronizer,
        application.settings,
    ));
    let scheduler_abort = scheduler.abort_handle();
    println!("CI Watcher development server listening on http://127.0.0.1:4317");
    axum::serve(listener, application.router)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            scheduler_abort.abort();
        })
        .await?;
    _ = scheduler.await;
    Ok(())
}

async fn run_synchronization_schedule(
    synchronizer: SynchronizeSources,
    settings: GetMonitoringSettings,
) {
    loop {
        let interval = settings
            .execute()
            .map(|settings| {
                std::time::Duration::from_secs(settings.synchronization_interval_seconds)
            })
            .unwrap_or(DEFAULT_SYNCHRONIZATION_INTERVAL);
        tokio::time::sleep(interval).await;
        _ = synchronizer.execute().await;
    }
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("failed to install shutdown signal handler: {error}");
    }
}
