use super::{
    ConnectionConfiguration, ConnectionValidationFailure, ProviderToken, SourceDescriptor,
    SourceRegistry, ValidatedAccount,
};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretReference(String);

impl SecretReference {
    #[must_use]
    pub fn for_connection(connection_id: &str, nonce: &str) -> Self {
        Self(format!("connection:{connection_id}:{nonce}"))
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub(crate) fn from_stored(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredConnection {
    pub id: String,
    pub source_id: String,
    pub unique_key: String,
    pub label: String,
    pub configuration: ConnectionConfiguration,
    pub account: ValidatedAccount,
    pub secret_reference: SecretReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistenceFailure;

impl Display for PersistenceFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("connection storage unavailable")
    }
}

impl Error for PersistenceFailure {}

pub trait ConnectionRepository: Send + Sync {
    fn save(&self, connection: &StoredConnection) -> Result<(), PersistenceFailure>;
    fn get(&self, connection_id: &str) -> Result<Option<StoredConnection>, PersistenceFailure>;
    fn find(
        &self,
        source_id: &str,
        unique_key: &str,
    ) -> Result<Option<StoredConnection>, PersistenceFailure>;
    fn list(&self) -> Result<Vec<StoredConnection>, PersistenceFailure>;
    fn delete(&self, connection_id: &str) -> Result<(), PersistenceFailure>;
}

pub trait SecretStore: Send + Sync {
    fn store(
        &self,
        reference: &SecretReference,
        token: &ProviderToken,
    ) -> Result<(), PersistenceFailure>;
    fn retrieve(&self, reference: &SecretReference) -> Result<ProviderToken, PersistenceFailure>;
    fn delete(&self, reference: &SecretReference) -> Result<(), PersistenceFailure>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectSourceFailure {
    UnknownSource,
    UnknownConnection,
    DuplicateConnection,
    Validation(ConnectionValidationFailure),
    StorageUnavailable,
}

impl Display for ConnectSourceFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSource => formatter.write_str("source module is not registered"),
            Self::UnknownConnection => formatter.write_str("source connection does not exist"),
            Self::DuplicateConnection => formatter.write_str("source connection already exists"),
            Self::Validation(failure) => Display::fmt(failure, formatter),
            Self::StorageUnavailable => formatter.write_str("connection storage unavailable"),
        }
    }
}

impl Error for ConnectSourceFailure {}

#[derive(Clone)]
pub struct ConnectSource {
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
}

impl ConnectSource {
    #[must_use]
    pub fn new(
        registry: SourceRegistry,
        connections: Arc<dyn ConnectionRepository>,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            registry,
            connections,
            secrets,
        }
    }

    pub async fn execute(
        &self,
        source_id: &str,
        connection_id: Option<&str>,
        configuration: &ConnectionConfiguration,
        token: String,
    ) -> Result<ConnectionState, ConnectSourceFailure> {
        let module = self
            .registry
            .get(source_id)
            .ok_or(ConnectSourceFailure::UnknownSource)?;
        let configured = module
            .configure(configuration)
            .map_err(ConnectSourceFailure::Validation)?;
        if token.trim().is_empty() {
            return Err(ConnectSourceFailure::Validation(
                ConnectionValidationFailure::InvalidCredentials,
            ));
        }

        let old_connection = if let Some(connection_id) = connection_id {
            let connection = self
                .connections
                .get(connection_id)
                .map_err(|_| ConnectSourceFailure::StorageUnavailable)?
                .ok_or(ConnectSourceFailure::UnknownConnection)?;
            if connection.source_id != source_id {
                return Err(ConnectSourceFailure::UnknownConnection);
            }
            Some(connection)
        } else {
            None
        };
        if old_connection
            .as_ref()
            .is_some_and(|connection| connection.unique_key != configured.unique_key)
        {
            return Err(ConnectSourceFailure::Validation(
                ConnectionValidationFailure::InvalidConfiguration,
            ));
        }
        if self
            .connections
            .find(source_id, &configured.unique_key)
            .map_err(|_| ConnectSourceFailure::StorageUnavailable)?
            .is_some_and(|existing| Some(existing.id.as_str()) != connection_id)
        {
            return Err(ConnectSourceFailure::DuplicateConnection);
        }

        let token = ProviderToken::new(token);
        let account = module
            .validate(&configured.configuration, &token)
            .await
            .map_err(ConnectSourceFailure::Validation)?;
        let connection_id = old_connection
            .as_ref()
            .map_or_else(random_connection_id, |connection| Ok(connection.id.clone()))
            .map_err(|_| ConnectSourceFailure::StorageUnavailable)?;
        let secret_nonce =
            random_identifier().map_err(|_| ConnectSourceFailure::StorageUnavailable)?;
        let secret_reference = SecretReference::for_connection(&connection_id, &secret_nonce);

        self.secrets
            .store(&secret_reference, &token)
            .map_err(|_| ConnectSourceFailure::StorageUnavailable)?;

        let connection = StoredConnection {
            id: connection_id,
            source_id: source_id.to_owned(),
            unique_key: configured.unique_key,
            label: configured.label,
            configuration: configured.configuration,
            account: account.clone(),
            secret_reference: secret_reference.clone(),
        };
        if self.connections.save(&connection).is_err() {
            if old_connection
                .as_ref()
                .is_none_or(|old| old.secret_reference != secret_reference)
            {
                _ = self.secrets.delete(&secret_reference);
            }
            return Err(ConnectSourceFailure::StorageUnavailable);
        }

        if let Some(old) = old_connection
            && old.secret_reference != secret_reference
        {
            _ = self.secrets.delete(&old.secret_reference);
        }

        Ok(ConnectionState::from(connection))
    }
}

fn random_connection_id() -> Result<String, PersistenceFailure> {
    random_identifier().map(|identifier| format!("source-{identifier}"))
}

fn random_identifier() -> Result<String, PersistenceFailure> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| PersistenceFailure)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionState {
    pub id: String,
    pub label: String,
    pub configuration: ConnectionConfiguration,
    pub account: ValidatedAccount,
}

impl From<StoredConnection> for ConnectionState {
    fn from(connection: StoredConnection) -> Self {
        Self {
            id: connection.id,
            label: connection.label,
            configuration: connection.configuration,
            account: connection.account,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceState {
    pub descriptor: SourceDescriptor,
    pub connections: Vec<ConnectionState>,
}

#[derive(Clone)]
pub struct ListSources {
    registry: SourceRegistry,
    connections: Arc<dyn ConnectionRepository>,
}

impl ListSources {
    #[must_use]
    pub fn new(registry: SourceRegistry, connections: Arc<dyn ConnectionRepository>) -> Self {
        Self {
            registry,
            connections,
        }
    }

    pub fn execute(&self) -> Result<Vec<SourceState>, PersistenceFailure> {
        let connections = self.connections.list()?;
        Ok(self
            .registry
            .modules
            .iter()
            .map(|module| {
                let descriptor = module.descriptor();
                let source_connections = connections
                    .iter()
                    .filter(|connection| connection.source_id == descriptor.id)
                    .cloned()
                    .map(ConnectionState::from)
                    .collect();
                SourceState {
                    descriptor,
                    connections: source_connections,
                }
            })
            .collect())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectedSource {
    pub id: String,
    pub descriptor: SourceDescriptor,
    pub label: String,
}

#[derive(Clone)]
pub struct DisconnectSource {
    connections: Arc<dyn ConnectionRepository>,
    secrets: Arc<dyn SecretStore>,
}

impl DisconnectSource {
    #[must_use]
    pub fn new(connections: Arc<dyn ConnectionRepository>, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            connections,
            secrets,
        }
    }

    pub fn execute(&self, connection_id: &str) -> Result<bool, PersistenceFailure> {
        let Some(connection) = self.connections.get(connection_id)? else {
            return Ok(false);
        };
        self.secrets.delete(&connection.secret_reference)?;
        self.connections.delete(connection_id)?;
        Ok(true)
    }
}
