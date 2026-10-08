#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod keyring;
mod notifications;
mod scheduler;
mod state;
mod tray;

use crate::commands::{
    action_options, change_request_details, connect_source, disconnect_source, execute_action,
    get_settings, health, issue_details, list_activity, list_change_requests, list_issues,
    list_repositories, list_sources, list_workflows, save_repository_selection,
    synchronization_status, synchronize_sources, update_settings, update_status, workflow_run_logs,
};
use crate::keyring::KeyringSecretStore;
use crate::notifications::DesktopNotificationSink;
use crate::scheduler::SynchronizationScheduler;
use crate::state::{DesktopState, DesktopStateDependencies};
use opscope_core::application::NotifyRepositoryFailures;
use opscope_core::integrations::registered_sources;
use opscope_core::persistence::{SqliteDatabase, prepare_database_path};
use std::sync::Arc;
use tauri::Manager;

fn main() {
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            tray::setup(app)?;
            let data_dir = app.path().app_data_dir()?;
            let legacy_data_dir = data_dir.with_file_name("dev.opsscope.desktop");
            let database = Arc::new(SqliteDatabase::open(prepare_database_path(
                &data_dir,
                Some(&legacy_data_dir),
            )?)?);
            let sources = registered_sources()?;
            let secrets = Arc::new(KeyringSecretStore);
            let notification_sink = Arc::new(DesktopNotificationSink::new(app.handle().clone()));
            let work_item_notifications = Some(opscope_core::application::NotifyWorkItems::new(
                database.clone(),
                database.clone(),
                database.clone(),
                notification_sink.clone(),
            ));
            let failure_notifications =
                NotifyRepositoryFailures::new(database.clone(), notification_sink)
                    .with_settings(database.clone());
            let state = DesktopState::new(DesktopStateDependencies {
                sources,
                connections: database.clone(),
                secrets,
                repository_selections: database.clone(),
                source_data_cache: database.clone(),
                settings: database.clone(),
                activity_events: database,
                failure_notifications,
                work_item_notifications,
            });
            let scheduler = SynchronizationScheduler::start(
                state.synchronize_sources.clone(),
                state.get_settings.clone(),
            );
            app.manage(state);
            app.manage(scheduler);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            action_options,
            execute_action,
            health,
            update_status,
            list_workflows,
            list_change_requests,
            list_issues,
            list_activity,
            change_request_details,
            issue_details,
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
