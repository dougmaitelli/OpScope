#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod keyring;
mod notifications;
mod scheduler;
mod state;
mod tray;

use crate::commands::{
    connect_source, disconnect_source, get_settings, health, list_change_requests,
    list_repositories, list_sources, list_workflows, save_repository_selection,
    synchronization_status, synchronize_sources, update_settings, update_status, workflow_run_logs,
};
use crate::keyring::KeyringSecretStore;
use crate::notifications::DesktopNotificationSink;
use crate::scheduler::SynchronizationScheduler;
use crate::state::DesktopState;
use opsscope_core::application::NotifyRepositoryFailures;
use opsscope_core::integrations::registered_sources;
use opsscope_core::persistence::SqliteDatabase;
use std::fs;
use std::sync::Arc;
use tauri::Manager;

fn main() {
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            tray::setup(app)?;
            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let database = Arc::new(SqliteDatabase::open(data_dir.join("opsscope.sqlite3"))?);
            let sources = registered_sources()?;
            let secrets = Arc::new(KeyringSecretStore);
            let failure_notifications = NotifyRepositoryFailures::new(
                database.clone(),
                Arc::new(DesktopNotificationSink::new(app.handle().clone())),
            );
            let state = DesktopState::new(
                sources,
                database.clone(),
                secrets,
                database.clone(),
                database.clone(),
                database,
                failure_notifications,
            );
            let scheduler = SynchronizationScheduler::start(
                state.synchronize_sources.clone(),
                state.get_settings.clone(),
            );
            app.manage(state);
            app.manage(scheduler);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health,
            update_status,
            list_workflows,
            list_change_requests,
            workflow_run_logs,
            synchronize_sources,
            synchronization_status,
            get_settings,
            update_settings,
            connect_source,
            list_sources,
            list_repositories,
            save_repository_selection,
            disconnect_source
        ])
        .on_window_event(tray::hide_when_minimized)
        .build(tauri::generate_context!())
        .expect("desktop runtime failed");
    application.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app.state::<SynchronizationScheduler>().stop();
        }
    });
}
