use opscope_core::application::{
    DEFAULT_SYNCHRONIZATION_INTERVAL, GetMonitoringSettings, NotifyRepositoryFailures,
    SynchronizeSources,
};
use opscope_core::integrations::registered_sources;
use opscope_core::persistence::{
    EncryptedSecretStore, ServerMasterKey, SqliteDatabase, prepare_database_path,
};
use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let bind_address =
        env::var("OPSCOPE_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:4317".to_owned());
    let listener = TcpListener::bind(&bind_address).await?;
    let authentication = opscope_server::auth::WebAuthentication::from_environment().await?;
    if !authentication.is_enabled() {
        eprintln!("OIDC authentication is disabled; access must be restricted by the deployment");
    }
    let sources = registered_sources()?;
    let configured_directory = env::var_os("OPSCOPE_DATA_DIR").map(PathBuf::from);
    let legacy_directory = configured_directory
        .is_none()
        .then(|| PathBuf::from(".opsscope-data"));
    let data_dir = configured_directory.unwrap_or_else(|| PathBuf::from(".opscope-data"));
    let database = SqliteDatabase::open(prepare_database_path(
        &data_dir,
        legacy_directory.as_deref(),
    )?)?;
    let master_key_path = env::var_os("OPSCOPE_MASTER_KEY_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("master.key"));
    let master_key = ServerMasterKey::load_or_create(master_key_path)?;
    let secrets = EncryptedSecretStore::new(database.clone(), master_key);
    let database = Arc::new(database);
    let notification_sink = opscope_server::notifications::notification_sink_from_environment()?;
    let work_item_notifications = Some(opscope_core::application::NotifyWorkItems::new(
        database.clone(),
        database.clone(),
        database.clone(),
        notification_sink.clone(),
    ));
    let failure_notifications = NotifyRepositoryFailures::new(database.clone(), notification_sink)
        .with_settings(database.clone());
    let application = opscope_server::application(opscope_server::ServerDependencies {
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
    let opscope_server::ServerApplication {
        router,
        synchronizer,
        settings,
    } = application;
    let scheduler = tokio::spawn(run_synchronization_schedule(synchronizer, settings));
    let scheduler_abort = scheduler.abort_handle();
    let router = match env::var_os("OPSCOPE_WEB_DIR") {
        Some(directory) => opscope_server::serve_web_application(router, PathBuf::from(directory)),
        None => router,
    };
    println!("OpScope server listening on http://{bind_address}");
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
