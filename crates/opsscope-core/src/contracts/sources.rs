use super::*;

/// A credential accepted for one validation attempt.
///
/// This request intentionally omits `Debug`, `Clone`, and `Serialize` in Rust.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectSourceRequest {
    pub source_id: String,
    pub connection_id: Option<String>,
    pub configuration: BTreeMap<String, String>,
    pub credential: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSummary {
    pub id: String,
    pub label: String,
    pub configuration: BTreeMap<String, String>,
    pub external_id: String,
    pub name: String,
    pub handle: Option<String>,
    pub profile_url: Option<String>,
    pub credential_stored: bool,
}

impl From<ConnectionState> for ConnectionSummary {
    fn from(connection: ConnectionState) -> Self {
        Self {
            id: connection.id,
            label: connection.label,
            configuration: connection.configuration,
            external_id: connection.account.external_id,
            name: connection.account.name,
            handle: connection.account.handle,
            profile_url: connection.account.profile_url,
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
pub struct ConnectionFieldSummary {
    pub key: String,
    pub label: String,
    pub placeholder: String,
    pub help: String,
    pub default_value: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SourceCapability {
    Workflows,
    ChangeRequests,
    Issues,
}

impl From<DomainSourceCapability> for SourceCapability {
    fn from(capability: DomainSourceCapability) -> Self {
        match capability {
            DomainSourceCapability::Workflows => Self::Workflows,
            DomainSourceCapability::ChangeRequests => Self::ChangeRequests,
            DomainSourceCapability::Issues => Self::Issues,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub abbreviation: String,
    pub capabilities: Vec<SourceCapability>,
    pub credential: CredentialFieldSummary,
    pub connection_fields: Vec<ConnectionFieldSummary>,
    pub connections: Vec<ConnectionSummary>,
}

impl From<SourceState> for SourceSummary {
    fn from(source: SourceState) -> Self {
        Self {
            id: source.descriptor.id,
            name: source.descriptor.name,
            description: source.descriptor.description,
            abbreviation: source.descriptor.abbreviation,
            capabilities: source
                .descriptor
                .capabilities
                .into_iter()
                .map(SourceCapability::from)
                .collect(),
            credential: CredentialFieldSummary {
                label: source.descriptor.credential.label,
                placeholder: source.descriptor.credential.placeholder,
                help: source.descriptor.credential.help,
            },
            connection_fields: source
                .descriptor
                .connection_fields
                .into_iter()
                .map(|field| ConnectionFieldSummary {
                    key: field.key,
                    label: field.label,
                    placeholder: field.placeholder,
                    help: field.help,
                    default_value: field.default_value,
                })
                .collect(),
            connections: source
                .connections
                .into_iter()
                .map(ConnectionSummary::from)
                .collect(),
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
    pub connection_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectSourceResponse {
    pub disconnected: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionValidationErrorCode {
    InvalidConfiguration,
    InvalidCredentials,
    PermissionDenied,
    RateLimited,
    ProviderUnavailable,
    UnexpectedResponse,
    StorageUnavailable,
    UnknownSource,
    UnknownConnection,
    DuplicateConnection,
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
            ConnectionValidationFailure::InvalidConfiguration => (
                ConnectionValidationErrorCode::InvalidConfiguration,
                "The source configuration is invalid. Check the server URL and try again.",
            ),
            ConnectionValidationFailure::InvalidCredentials => (
                ConnectionValidationErrorCode::InvalidCredentials,
                "The source rejected this credential. Check it and try again.",
            ),
            ConnectionValidationFailure::PermissionDenied => (
                ConnectionValidationErrorCode::PermissionDenied,
                "The source credential does not have permission to read workflow metadata.",
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
            ConnectSourceFailure::UnknownConnection => Self {
                code: ConnectionValidationErrorCode::UnknownConnection,
                message: "This source connection no longer exists. Refresh and try again."
                    .to_owned(),
            },
            ConnectSourceFailure::DuplicateConnection => Self {
                code: ConnectionValidationErrorCode::DuplicateConnection,
                message: "A connection for this source and server already exists.".to_owned(),
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
