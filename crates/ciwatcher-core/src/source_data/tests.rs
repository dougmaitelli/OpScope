use super::*;
use crate::application::{
    CredentialField, ProviderToken, SecretReference, SourceModule, StoredConnection,
    ValidatedAccount,
};
use crate::domain::{RepositoryVisibility, WorkflowState};
use crate::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone)]
struct CountingSourceModule {
    repository_requests: Arc<AtomicUsize>,
    workflow_requests: Arc<AtomicUsize>,
}

#[async_trait]
impl SourceModule for CountingSourceModule {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "example".to_owned(),
            name: "Example".to_owned(),
            description: "Example source".to_owned(),
            abbreviation: "EX".to_owned(),
            credential: CredentialField {
                label: "Token".to_owned(),
                placeholder: "token".to_owned(),
                help: "Test token".to_owned(),
            },
        }
    }

    async fn validate(
        &self,
        _token: &ProviderToken,
    ) -> Result<ValidatedAccount, ConnectionValidationFailure> {
        unreachable!("validation is not part of source data loading")
    }

    async fn list_repositories(
        &self,
        _token: &ProviderToken,
    ) -> Result<Vec<Repository>, ConnectionValidationFailure> {
        self.repository_requests.fetch_add(1, Ordering::SeqCst);
        Ok(vec![test_repository()])
    }

    async fn list_workflows(
        &self,
        _token: &ProviderToken,
        _repository: &Repository,
    ) -> Result<Vec<Workflow>, ConnectionValidationFailure> {
        self.workflow_requests.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Workflow {
            id: "workflow-1".to_owned(),
            name: "Build".to_owned(),
            path: ".ci/build.yml".to_owned(),
            state: WorkflowState::Active,
            web_url: "https://example.com/workflows/1".to_owned(),
        }])
    }
}

fn test_repository() -> Repository {
    Repository {
        id: "repository-1".to_owned(),
        owner: "owner".to_owned(),
        name: "project".to_owned(),
        description: None,
        visibility: RepositoryVisibility::Private,
        web_url: "https://example.com/owner/project".to_owned(),
    }
}

struct TestDependencies {
    registry: SourceRegistry,
    database: SqliteDatabase,
    secrets: EncryptedSecretStore,
    repository_requests: Arc<AtomicUsize>,
    workflow_requests: Arc<AtomicUsize>,
}

fn dependencies() -> Result<TestDependencies, PersistenceFailure> {
    let repository_requests = Arc::new(AtomicUsize::new(0));
    let workflow_requests = Arc::new(AtomicUsize::new(0));
    let registry = SourceRegistry::new(vec![Arc::new(CountingSourceModule {
        repository_requests: repository_requests.clone(),
        workflow_requests: workflow_requests.clone(),
    })]);
    let database = SqliteDatabase::in_memory()?;
    let reference = SecretReference::for_source("example", "42");
    database.save(&StoredConnection {
        source_id: "example".to_owned(),
        account: ValidatedAccount {
            external_id: "42".to_owned(),
            name: "Example Account".to_owned(),
            handle: None,
            profile_url: None,
        },
        secret_reference: reference.clone(),
    })?;
    let secrets = EncryptedSecretStore::new(database.clone(), ServerMasterKey::generate()?);
    secrets.store(&reference, &ProviderToken::new("secret".to_owned()))?;
    Ok(TestDependencies {
        registry,
        database,
        secrets,
        repository_requests,
        workflow_requests,
    })
}

#[tokio::test]
async fn cached_source_data_hides_cache_hits_from_its_caller()
-> Result<(), Box<dyn std::error::Error>> {
    let dependencies = dependencies()?;
    let source_data = ReadThroughSourceData::cached(
        dependencies.registry,
        Arc::new(dependencies.database.clone()),
        Arc::new(dependencies.secrets),
        Arc::new(dependencies.database),
        Duration::MAX,
    );

    let first_repositories = source_data
        .repositories("example")
        .await?
        .expect("connected source");
    let second_repositories = source_data
        .repositories("example")
        .await?
        .expect("connected source");
    assert_eq!(first_repositories, second_repositories);
    assert_eq!(dependencies.repository_requests.load(Ordering::SeqCst), 1);

    let repository = &first_repositories[0];
    let first_workflows = source_data.workflows("example", repository).await?;
    let second_workflows = source_data.workflows("example", repository).await?;
    assert_eq!(first_workflows, second_workflows);
    assert_eq!(dependencies.workflow_requests.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn uncached_source_data_uses_the_same_interface() -> Result<(), Box<dyn std::error::Error>> {
    let dependencies = dependencies()?;
    let source_data = ReadThroughSourceData::uncached(
        dependencies.registry,
        Arc::new(dependencies.database),
        Arc::new(dependencies.secrets),
    );

    source_data.repositories("example").await?;
    source_data.repositories("example").await?;
    assert_eq!(dependencies.repository_requests.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn zero_max_age_refreshes_provider_data() -> Result<(), Box<dyn std::error::Error>> {
    let dependencies = dependencies()?;
    let source_data = ReadThroughSourceData::cached(
        dependencies.registry,
        Arc::new(dependencies.database.clone()),
        Arc::new(dependencies.secrets),
        Arc::new(dependencies.database),
        Duration::ZERO,
    );

    source_data.repositories("example").await?;
    source_data.repositories("example").await?;
    assert_eq!(dependencies.repository_requests.load(Ordering::SeqCst), 2);
    Ok(())
}
