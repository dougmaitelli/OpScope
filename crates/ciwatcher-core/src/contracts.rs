//! Secret-free data transfer objects shared by HTTP and desktop IPC.

use crate::domain::{Monitor, MonitorStatus as DomainMonitorStatus};
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

pub const CONTRACT_VERSION: u8 = 1;
pub const HEALTH_HTTP_PATH: &str = "/api/health";
pub const LIST_MONITORS_HTTP_PATH: &str = "/api/monitors";
pub const HEALTH_DESKTOP_COMMAND: &str = "health";
pub const LIST_MONITORS_DESKTOP_COMMAND: &str = "list_monitors";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub contract_version: u8,
}

impl HealthResponse {
    #[must_use]
    pub fn ready() -> Self {
        Self {
            status: "ok".to_owned(),
            service: "ciwatcher".to_owned(),
            contract_version: CONTRACT_VERSION,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MonitorStatus {
    Unknown,
    Passing,
    Failing,
    Running,
}

impl From<DomainMonitorStatus> for MonitorStatus {
    fn from(status: DomainMonitorStatus) -> Self {
        match status {
            DomainMonitorStatus::Unknown => Self::Unknown,
            DomainMonitorStatus::Passing => Self::Passing,
            DomainMonitorStatus::Failing => Self::Failing,
            DomainMonitorStatus::Running => Self::Running,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSummary {
    pub id: String,
    pub name: String,
    pub status: MonitorStatus,
}

impl From<Monitor> for MonitorSummary {
    fn from(monitor: Monitor) -> Self {
        Self {
            id: monitor.id,
            name: monitor.name,
            status: monitor.status.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListMonitorsResponse {
    pub monitors: Vec<MonitorSummary>,
}

impl ListMonitorsResponse {
    #[must_use]
    pub fn from_domain(monitors: Vec<Monitor>) -> Self {
        Self {
            monitors: monitors.into_iter().map(MonitorSummary::from).collect(),
        }
    }
}

#[must_use]
pub fn render_typescript_contract() -> String {
    let config = Config::default();
    format!(
        "// Generated from crates/ciwatcher-core/src/contracts.rs. Do not edit.\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport const httpRoutes = {{\n  health: \"{HEALTH_HTTP_PATH}\",\n  listMonitors: \"{LIST_MONITORS_HTTP_PATH}\",\n}} as const;\n\nexport const desktopCommands = {{\n  health: \"{HEALTH_DESKTOP_COMMAND}\",\n  listMonitors: \"{LIST_MONITORS_DESKTOP_COMMAND}\",\n}} as const;\n\nexport interface ApplicationClient {{\n  health(): Promise<HealthResponse>;\n  listMonitors(): Promise<ListMonitorsResponse>;\n}}\n",
        HealthResponse::decl(&config),
        MonitorStatus::decl(&config),
        MonitorSummary::decl(&config),
        ListMonitorsResponse::decl(&config),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_domain_values_without_exposing_infrastructure() {
        let response = ListMonitorsResponse::from_domain(vec![Monitor {
            id: "provider:monitor:42".to_owned(),
            name: "Build".to_owned(),
            status: DomainMonitorStatus::Running,
        }]);

        assert_eq!(response.monitors[0].status, MonitorStatus::Running);
    }
}
