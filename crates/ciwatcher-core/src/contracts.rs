//! Data transfer objects shared by HTTP and desktop IPC.

use crate::application::{
    ConnectSourceFailure, ConnectionValidationFailure, SourceState, ValidatedAccount,
};
use crate::domain::{Monitor, MonitorStatus as DomainMonitorStatus};
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

pub const CONTRACT_VERSION: u8 = 1;
pub const HEALTH_HTTP_PATH: &str = "/api/health";
pub const LIST_MONITORS_HTTP_PATH: &str = "/api/monitors";
pub const SOURCES_HTTP_PATH: &str = "/api/sources";
pub const CONNECTIONS_HTTP_PATH: &str = "/api/connections";
pub const HEALTH_DESKTOP_COMMAND: &str = "health";
pub const LIST_MONITORS_DESKTOP_COMMAND: &str = "list_monitors";
pub const LIST_SOURCES_DESKTOP_COMMAND: &str = "list_sources";
pub const CONNECT_SOURCE_DESKTOP_COMMAND: &str = "connect_source";
pub const DISCONNECT_SOURCE_DESKTOP_COMMAND: &str = "disconnect_source";

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

/// A credential accepted for one validation attempt.
///
/// This request intentionally omits `Debug`, `Clone`, and `Serialize` in Rust.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectSourceRequest {
    pub source_id: String,
    pub credential: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSummary {
    pub external_id: String,
    pub name: String,
    pub handle: Option<String>,
    pub profile_url: Option<String>,
    pub credential_stored: bool,
}

impl From<ValidatedAccount> for ConnectionSummary {
    fn from(account: ValidatedAccount) -> Self {
        Self {
            external_id: account.external_id,
            name: account.name,
            handle: account.handle,
            profile_url: account.profile_url,
            credential_stored: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CredentialFieldSummary {
    pub label: String,
    pub placeholder: String,
    pub help: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub abbreviation: String,
    pub credential: CredentialFieldSummary,
    pub connection: Option<ConnectionSummary>,
}

impl From<SourceState> for SourceSummary {
    fn from(source: SourceState) -> Self {
        Self {
            id: source.descriptor.id,
            name: source.descriptor.name,
            description: source.descriptor.description,
            abbreviation: source.descriptor.abbreviation,
            credential: CredentialFieldSummary {
                label: source.descriptor.credential.label,
                placeholder: source.descriptor.credential.placeholder,
                help: source.descriptor.credential.help,
            },
            connection: source.account.map(ConnectionSummary::from),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListSourcesResponse {
    pub sources: Vec<SourceSummary>,
}

impl ListSourcesResponse {
    #[must_use]
    pub fn from_domain(sources: Vec<SourceState>) -> Self {
        Self {
            sources: sources.into_iter().map(SourceSummary::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectSourceRequest {
    pub source_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectSourceResponse {
    pub disconnected: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionValidationErrorCode {
    InvalidCredentials,
    RateLimited,
    ProviderUnavailable,
    UnexpectedResponse,
    StorageUnavailable,
    UnknownSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionValidationErrorResponse {
    pub code: ConnectionValidationErrorCode,
    pub message: String,
}

impl From<ConnectionValidationFailure> for ConnectionValidationErrorResponse {
    fn from(failure: ConnectionValidationFailure) -> Self {
        let (code, message) = match failure {
            ConnectionValidationFailure::InvalidCredentials => (
                ConnectionValidationErrorCode::InvalidCredentials,
                "The source rejected this credential. Check it and try again.",
            ),
            ConnectionValidationFailure::RateLimited => (
                ConnectionValidationErrorCode::RateLimited,
                "The source rate limit was reached. Try again later.",
            ),
            ConnectionValidationFailure::ProviderUnavailable => (
                ConnectionValidationErrorCode::ProviderUnavailable,
                "The source could not be reached. Try again.",
            ),
            ConnectionValidationFailure::UnexpectedResponse => (
                ConnectionValidationErrorCode::UnexpectedResponse,
                "The source returned an unexpected response.",
            ),
        };

        Self {
            code,
            message: message.to_owned(),
        }
    }
}

impl From<ConnectSourceFailure> for ConnectionValidationErrorResponse {
    fn from(failure: ConnectSourceFailure) -> Self {
        match failure {
            ConnectSourceFailure::UnknownSource => Self {
                code: ConnectionValidationErrorCode::UnknownSource,
                message: "This source module is not registered.".to_owned(),
            },
            ConnectSourceFailure::Validation(failure) => failure.into(),
            ConnectSourceFailure::StorageUnavailable => Self {
                code: ConnectionValidationErrorCode::StorageUnavailable,
                message: "The connection was validated but could not be stored securely."
                    .to_owned(),
            },
        }
    }
}

#[must_use]
pub fn render_typescript_contract() -> String {
    let config = Config::default();
    format!(
        "// Generated from crates/ciwatcher-core/src/contracts.rs. Do not edit.\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport const httpRoutes = {{\n  health: \"{HEALTH_HTTP_PATH}\",\n  listMonitors: \"{LIST_MONITORS_HTTP_PATH}\",\n  sources: \"{SOURCES_HTTP_PATH}\",\n  connections: \"{CONNECTIONS_HTTP_PATH}\",\n}} as const;\n\nexport const desktopCommands = {{\n  health: \"{HEALTH_DESKTOP_COMMAND}\",\n  listMonitors: \"{LIST_MONITORS_DESKTOP_COMMAND}\",\n  listSources: \"{LIST_SOURCES_DESKTOP_COMMAND}\",\n  connectSource: \"{CONNECT_SOURCE_DESKTOP_COMMAND}\",\n  disconnectSource: \"{DISCONNECT_SOURCE_DESKTOP_COMMAND}\",\n}} as const;\n\nexport interface ApplicationClient {{\n  health(): Promise<HealthResponse>;\n  listMonitors(): Promise<ListMonitorsResponse>;\n  listSources(): Promise<ListSourcesResponse>;\n  connectSource(request: ConnectSourceRequest): Promise<ConnectionSummary>;\n  disconnectSource(request: DisconnectSourceRequest): Promise<DisconnectSourceResponse>;\n}}\n",
        HealthResponse::decl(&config),
        MonitorStatus::decl(&config),
        MonitorSummary::decl(&config),
        ListMonitorsResponse::decl(&config),
        ConnectSourceRequest::decl(&config),
        ConnectionSummary::decl(&config),
        CredentialFieldSummary::decl(&config),
        SourceSummary::decl(&config),
        ListSourcesResponse::decl(&config),
        DisconnectSourceRequest::decl(&config),
        DisconnectSourceResponse::decl(&config),
        ConnectionValidationErrorCode::decl(&config),
        ConnectionValidationErrorResponse::decl(&config),
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

    #[test]
    fn validation_response_contains_identity_and_status_but_no_secret() {
        let response = ConnectionSummary::from(ValidatedAccount {
            external_id: "42".to_owned(),
            name: "The Octocat".to_owned(),
            handle: Some("octocat".to_owned()),
            profile_url: Some("https://example.com/octocat".to_owned()),
        });
        let serialized = serde_json::to_string(&response).expect("response serializes");

        assert!(serialized.contains("octocat"));
        assert!(!serialized.contains("token"));
        assert!(!serialized.contains("secret"));
    }
}
