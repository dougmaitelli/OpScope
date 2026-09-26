use ciwatcher_core::application::{
    PersistenceFailure, ProviderToken, SecretReference, SecretStore,
};

#[derive(Clone, Copy)]
pub(crate) struct KeyringSecretStore;

impl KeyringSecretStore {
    fn entry(reference: &SecretReference) -> Result<keyring::Entry, PersistenceFailure> {
        keyring::Entry::new("dev.ciwatcher.desktop", reference.expose())
            .map_err(|_| PersistenceFailure)
    }
}

impl SecretStore for KeyringSecretStore {
    fn store(
        &self,
        reference: &SecretReference,
        token: &ProviderToken,
    ) -> Result<(), PersistenceFailure> {
        Self::entry(reference)?
            .set_password(token.expose())
            .map_err(|_| PersistenceFailure)
    }

    fn retrieve(&self, reference: &SecretReference) -> Result<ProviderToken, PersistenceFailure> {
        Self::entry(reference)?
            .get_password()
            .map(ProviderToken::new)
            .map_err(|_| PersistenceFailure)
    }

    fn delete(&self, reference: &SecretReference) -> Result<(), PersistenceFailure> {
        match Self::entry(reference)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(PersistenceFailure),
        }
    }
}
