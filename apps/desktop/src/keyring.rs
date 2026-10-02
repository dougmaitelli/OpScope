use opscope_core::application::{PersistenceFailure, ProviderToken, SecretReference, SecretStore};

#[derive(Clone, Copy)]
pub(crate) struct KeyringSecretStore;

const SERVICE: &str = "dev.opscope.desktop";
const LEGACY_SERVICE: &str = "dev.opsscope.desktop";

impl KeyringSecretStore {
    fn entry(
        service: &str,
        reference: &SecretReference,
    ) -> Result<keyring::Entry, PersistenceFailure> {
        keyring::Entry::new(service, reference.expose()).map_err(|_| PersistenceFailure)
    }
}

impl SecretStore for KeyringSecretStore {
    fn store(
        &self,
        reference: &SecretReference,
        token: &ProviderToken,
    ) -> Result<(), PersistenceFailure> {
        Self::entry(SERVICE, reference)?
            .set_password(token.expose())
            .map_err(|_| PersistenceFailure)
    }

    fn retrieve(&self, reference: &SecretReference) -> Result<ProviderToken, PersistenceFailure> {
        let entry = Self::entry(SERVICE, reference)?;
        let password = match entry.get_password() {
            Err(keyring::Error::NoEntry) => {
                let password = Self::entry(LEGACY_SERVICE, reference)?
                    .get_password()
                    .map_err(|_| PersistenceFailure)?;
                entry
                    .set_password(&password)
                    .map_err(|_| PersistenceFailure)?;
                Ok(password)
            }
            result => result,
        };
        password
            .map(ProviderToken::new)
            .map_err(|_| PersistenceFailure)
    }

    fn delete(&self, reference: &SecretReference) -> Result<(), PersistenceFailure> {
        for service in [SERVICE, LEGACY_SERVICE] {
            match Self::entry(service, reference)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(_) => return Err(PersistenceFailure),
            }
        }
        Ok(())
    }
}
