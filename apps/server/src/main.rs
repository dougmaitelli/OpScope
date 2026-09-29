use opsscope_core::application::{
    DEFAULT_SYNCHRONIZATION_INTERVAL, GetMonitoringSettings, NotifyRepositoryFailures,
    SynchronizeSources,
};
use opsscope_core::integrations::registered_sources;
use opsscope_core::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let bind_address =
        env::var("OPSSCOPE_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:4317".to_owned());
    let listener = TcpListener::bind(&bind_address).await?;
    let authentication = opsscope_server::auth::WebAuthentication::from_environment().await?;
    if !authentication.is_enabled() {
        eprintln!("OIDC authentication is disabled; access must be restricted by the deployment");
    }
    let sources = registered_sources()?;
    let data_dir = env::var_os("OPSSCOPE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".opsscope-data"));
    fs::create_dir_all(&data_dir)?;
    let database = SqliteDatabase::open(data_dir.join("opsscope.sqlite3"))?;
    let master_key_path = env::var_os("OPSSCOPE_MASTER_KEY_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("master.key"));
    let master_key = ServerMasterKey::load_or_create(master_key_path)?;
    let secrets = EncryptedSecretStore::new(database.clone(), master_key);
    let database = Arc::new(database);
    let notification_sink = opsscope_server::notifications::notification_sink_from_environment()?;
    let work_item_notifications = Some(opsscope_core::application::NotifyWorkItems::new(
        database.clone(),
        database.clone(),
        database.clone(),
        notification_sink.clone(),
    ));
    let failure_notifications = NotifyRepositoryFailures::new(database.clone(), notification_sink)
        .with_settings(database.clone());
    let application = opsscope_server::application(opsscope_server::ServerDependencies {
        registry: sources,
        connections: database.clone(),
        secrets: Arc::new(secrets),
        repository_selections: database.clone(),
        source_data_cache: database.clone(),
        settings: database.clone(),
        activity_events: database,
        failure_notifications,
        work_item_notifications,
        authentication,
    });
    let opsscope_server::ServerApplication {
        router,
        synchronizer,
        settings,
    } = application;
    let scheduler = tokio::spawn(run_synchronization_schedule(synchronizer, settings));
    let scheduler_abort = scheduler.abort_handle();
    let router = match env::var_os("OPSSCOPE_WEB_DIR") {
        Some(directory) => opsscope_server::serve_web_application(router, PathBuf::from(directory)),
        None => router,
    };
    println!("OpsScope server listening on http://{bind_address}");
    axum::serve(listener, router)
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
