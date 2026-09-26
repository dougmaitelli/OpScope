#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ciwatcher_core::application::ListMonitors;
use ciwatcher_core::contracts::{HealthResponse, ListMonitorsResponse};
use ciwatcher_core::integrations::fake::FakeMonitorSource;
use tauri::State;

#[derive(Debug)]
struct DesktopState {
    list_monitors: ListMonitors<FakeMonitorSource>,
}

#[tauri::command]
fn health() -> HealthResponse {
    HealthResponse::ready()
}

#[tauri::command]
async fn list_monitors(state: State<'_, DesktopState>) -> Result<ListMonitorsResponse, String> {
    state
        .list_monitors
        .execute()
        .await
        .map(ListMonitorsResponse::from_domain)
        .map_err(|_| "monitor source unavailable".to_owned())
}

fn main() {
    tauri::Builder::default()
        .manage(DesktopState {
            list_monitors: ListMonitors::new(FakeMonitorSource),
        })
        .invoke_handler(tauri::generate_handler![health, list_monitors])
        .run(tauri::generate_context!())
        .expect("desktop runtime failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_uses_the_shared_contract() {
        assert_eq!(health(), HealthResponse::ready());
    }
}
