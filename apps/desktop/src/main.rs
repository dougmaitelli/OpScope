#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod keyring;
mod scheduler;
mod state;

use crate::commands::{
    connect_source, disconnect_source, health, list_repositories, list_sources, list_workflows,
    save_repository_selection, synchronization_status, synchronize_sources, workflow_run_logs,
};
use crate::keyring::KeyringSecretStore;
use crate::scheduler::SynchronizationScheduler;
use crate::state::DesktopState;
use ciwatcher_core::integrations::registered_sources;
use ciwatcher_core::persistence::SqliteDatabase;
use std::fs;
use std::sync::Arc;
use tauri::Manager;

fn main() {
    let application = tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let database = Arc::new(SqliteDatabase::open(data_dir.join("ciwatcher.sqlite3"))?);
            let sources = registered_sources()?;
            let secrets = Arc::new(KeyringSecretStore);
            let state = DesktopState::new(
                sources,
                database.clone(),
                secrets,
                database.clone(),
                database,
            );
            let scheduler = SynchronizationScheduler::start(state.synchronize_sources.clone());
            app.manage(state);
            app.manage(scheduler);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health,
            list_workflows,
            workflow_run_logs,
            synchronize_sources,
            synchronization_status,
            connect_source,
            list_sources,
            list_repositories,
            save_repository_selection,
            disconnect_source
        ])
        .build(tauri::generate_context!())
        .expect("desktop runtime failed");
    application.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app.state::<SynchronizationScheduler>().stop();
        }
    });
}
