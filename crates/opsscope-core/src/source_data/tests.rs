use super::*;
use crate::application::{
    ConfiguredSource, ConnectionConfiguration, CredentialField, ProviderToken, SecretReference,
    SourceCapability, SourceDescriptor, SourceModule, StoredConnection, ValidatedAccount,
    WorkflowRunLogsFailure,
};
use crate::domain::{
    RepositoryVisibility, RunLifecycle, RunOutcome, WorkflowRun, WorkflowRunLogs, WorkflowState,
};
use crate::persistence::{EncryptedSecretStore, ServerMasterKey, SqliteDatabase};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Clone)]
struct CountingSourceModule {
    repository_requests: Arc<AtomicUsize>,
    workflow_requests: Arc<AtomicUsize>,
    workflow_run_requests: Arc<AtomicUsize>,
    fail_workflow_runs: Arc<AtomicBool>,
}

#[async_trait]
impl SourceModule for CountingSourceModule {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "example".to_owned(),
            name: "Example".to_owned(),
            description: "Example source".to_owned(),
            abbreviation: "EX".to_owned(),
            capabilities: vec![SourceCapability::Workflows],
            credential: CredentialField {
                label: "Token".to_owned(),
                placeholder: "token".to_owned(),
                help: "Test token".to_owned(),
            },
            connection_fields: Vec::new(),
        }
    }

    fn configure(
        &self,
        configuration: &ConnectionConfiguration,
    ) -> Result<ConfiguredSource, ConnectionValidationFailure> {
        Ok(ConfiguredSource {
            unique_key: "example".to_owned(),
            label: "Example".to_owned(),
            configuration: configuration.clone(),
        })
    }

    async fn validate(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
    ) -> Result<ValidatedAccount, ConnectionValidationFailure> {
        unreachable!("validation is not part of source data loading")
    }

    async fn list_repositories(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
    ) -> Result<Vec<Repository>, ConnectionValidationFailure> {
        self.repository_requests.fetch_add(1, Ordering::SeqCst);
        Ok(vec![test_repository()])
    }

    async fn list_workflows(
        &self,
        _configuration: &ConnectionConfiguration,
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

    async fn list_workflow_runs(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
        _repository: &Repository,
    ) -> Result<Vec<WorkflowRun>, ConnectionValidationFailure> {
        self.workflow_run_requests.fetch_add(1, Ordering::SeqCst);
        if self.fail_workflow_runs.load(Ordering::SeqCst) {
            Err(ConnectionValidationFailure::ProviderUnavailable)
        } else {
            Ok(vec![test_run()])
        }
    }

    async fn workflow_run_logs(
        &self,
        _configuration: &ConnectionConfiguration,
        _token: &ProviderToken,
        _repository: &Repository,
        _run: &WorkflowRun,
    ) -> Result<WorkflowRunLogs, WorkflowRunLogsFailure> {
        Ok(WorkflowRunLogs {
            files: Vec::new(),
            truncated: false,
        })
    }
}

fn test_run() -> WorkflowRun {
    WorkflowRun {
        id: "run-1".to_owned(),
        workflow_id: "workflow-1".to_owned(),
        run_number: 12,
        attempt: 1,
        title: "Build main".to_owned(),
        lifecycle: RunLifecycle::Completed,
        outcome: RunOutcome::Success,
        branch: Some("main".to_owned()),
        commit_sha: "abcdef123456".to_owned(),
        actor: Some("octocat".to_owned()),
        trigger: "push".to_owned(),
        created_at: "2026-09-26T18:00:00Z".to_owned(),
        started_at: Some("2026-09-26T18:00:02Z".to_owned()),
        updated_at: "2026-09-26T18:03:00Z".to_owned(),
        web_url: "https://example.com/runs/1".to_owned(),
        provider_status: "completed".to_owned(),
        provider_conclusion: Some("success".to_owned()),
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
    workflow_run_requests: Arc<AtomicUsize>,
    fail_workflow_runs: Arc<AtomicBool>,
}

fn dependencies() -> Result<TestDependencies, PersistenceFailure> {
    let repository_requests = Arc::new(AtomicUsize::new(0));
    let workflow_requests = Arc::new(AtomicUsize::new(0));
    let workflow_run_requests = Arc::new(AtomicUsize::new(0));
    let fail_workflow_runs = Arc::new(AtomicBool::new(false));
    let registry = SourceRegistry::new(vec![Arc::new(CountingSourceModule {
        repository_requests: repository_requests.clone(),
        workflow_requests: workflow_requests.clone(),
        workflow_run_requests: workflow_run_requests.clone(),
        fail_workflow_runs: fail_workflow_runs.clone(),
    })]);
    let database = SqliteDatabase::in_memory()?;
    let reference = SecretReference::for_connection("example", "test");
    database.save(&StoredConnection {
        id: "example".to_owned(),
        source_id: "example".to_owned(),
        unique_key: "example".to_owned(),
        label: "Example".to_owned(),
        configuration: ConnectionConfiguration::new(),
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
        workflow_run_requests,
        fail_workflow_runs,
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
        SourceDataCachePolicy::uniform(Duration::MAX),
    );

    let first_repositories = source_data
        .repositories("example", RefreshMode::IfStale)
        .await?
        .expect("connected source");
    let second_repositories = source_data
        .repositories("example", RefreshMode::IfStale)
        .await?
        .expect("connected source");
    assert_eq!(first_repositories, second_repositories);
    assert_eq!(dependencies.repository_requests.load(Ordering::SeqCst), 1);

    let repository = &first_repositories[0];
    let first_workflows = source_data
        .workflows("example", repository, RefreshMode::IfStale)
        .await?;
    let second_workflows = source_data
        .workflows("example", repository, RefreshMode::IfStale)
        .await?;
    assert_eq!(first_workflows, second_workflows);
    assert_eq!(dependencies.workflow_requests.load(Ordering::SeqCst), 1);

    let first_runs = source_data
        .workflow_runs("example", repository, RefreshMode::IfStale)
        .await?;
    let second_runs = source_data
        .workflow_runs("example", repository, RefreshMode::IfStale)
        .await?;
    assert_eq!(first_runs.runs, second_runs.runs);
    assert!(!second_runs.stale);
    assert_eq!(dependencies.workflow_run_requests.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn cache_policy_applies_a_separate_ttl_to_each_resource_kind()
-> Result<(), Box<dyn std::error::Error>> {
    let dependencies = dependencies()?;
    let source_data = ReadThroughSourceData::cached(
        dependencies.registry,
        Arc::new(dependencies.database.clone()),
        Arc::new(dependencies.secrets),
        Arc::new(dependencies.database),
        SourceDataCachePolicy {
            repositories: Duration::MAX,
            workflows: Duration::ZERO,
            workflow_runs: Duration::ZERO,
            change_requests: Duration::ZERO,
            change_request_details: Duration::ZERO,
        },
    );

    let repositories = source_data
        .repositories("example", RefreshMode::IfStale)
        .await?
        .expect("connected source");
    source_data
        .repositories("example", RefreshMode::IfStale)
        .await?;
    assert_eq!(dependencies.repository_requests.load(Ordering::SeqCst), 1);

    let repository = &repositories[0];
    source_data
        .workflows("example", repository, RefreshMode::IfStale)
        .await?;
    source_data
        .workflows("example", repository, RefreshMode::IfStale)
        .await?;
    assert_eq!(dependencies.workflow_requests.load(Ordering::SeqCst), 2);

    source_data
        .workflow_runs("example", repository, RefreshMode::IfStale)
        .await?;
    source_data
        .workflow_runs("example", repository, RefreshMode::IfStale)
        .await?;
    assert_eq!(dependencies.workflow_run_requests.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn forced_refresh_bypasses_fresh_cache_entries() -> Result<(), Box<dyn std::error::Error>> {
    let dependencies = dependencies()?;
    let source_data = ReadThroughSourceData::cached(
        dependencies.registry,
        Arc::new(dependencies.database.clone()),
        Arc::new(dependencies.secrets),
        Arc::new(dependencies.database),
        SourceDataCachePolicy::uniform(Duration::MAX),
    );

    let repositories = source_data
        .repositories("example", RefreshMode::IfStale)
        .await?
        .expect("connected source");
    source_data
        .repositories("example", RefreshMode::Force)
        .await?;
    assert_eq!(dependencies.repository_requests.load(Ordering::SeqCst), 2);

    let repository = &repositories[0];
    source_data
        .workflows("example", repository, RefreshMode::IfStale)
        .await?;
    source_data
        .workflows("example", repository, RefreshMode::Force)
        .await?;
    assert_eq!(dependencies.workflow_requests.load(Ordering::SeqCst), 2);

    source_data
        .workflow_runs("example", repository, RefreshMode::IfStale)
        .await?;
    source_data
        .workflow_runs("example", repository, RefreshMode::Force)
        .await?;
    assert_eq!(dependencies.workflow_run_requests.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn cache_first_returns_expired_data_without_waiting_for_the_provider()
-> Result<(), Box<dyn std::error::Error>> {
    let dependencies = dependencies()?;
    let source_data = ReadThroughSourceData::cached(
        dependencies.registry,
        Arc::new(dependencies.database.clone()),
        Arc::new(dependencies.secrets),
        Arc::new(dependencies.database),
        SourceDataCachePolicy::uniform(Duration::ZERO),
    );

    let repositories = source_data
        .repositories("example", RefreshMode::IfStale)
        .await?
        .expect("connected source");
    source_data
        .repositories("example", RefreshMode::CacheFirst)
        .await?;
    assert_eq!(dependencies.repository_requests.load(Ordering::SeqCst), 1);

    let repository = &repositories[0];
    source_data
        .workflows("example", repository, RefreshMode::IfStale)
        .await?;
    source_data
        .workflows("example", repository, RefreshMode::CacheFirst)
        .await?;
    assert_eq!(dependencies.workflow_requests.load(Ordering::SeqCst), 1);

    source_data
        .workflow_runs("example", repository, RefreshMode::IfStale)
        .await?;
    let cached_runs = source_data
        .workflow_runs("example", repository, RefreshMode::CacheFirst)
        .await?;
    assert!(cached_runs.stale);
    assert_eq!(dependencies.workflow_run_requests.load(Ordering::SeqCst), 1);
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

    source_data
        .repositories("example", RefreshMode::IfStale)
        .await?;
    source_data
        .repositories("example", RefreshMode::IfStale)
        .await?;
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
        SourceDataCachePolicy::uniform(Duration::ZERO),
    );

    source_data
        .repositories("example", RefreshMode::IfStale)
        .await?;
    source_data
        .repositories("example", RefreshMode::IfStale)
        .await?;
    assert_eq!(dependencies.repository_requests.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn provider_failure_returns_the_last_successful_run_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let dependencies = dependencies()?;
    let source_data = ReadThroughSourceData::cached(
        dependencies.registry,
        Arc::new(dependencies.database.clone()),
        Arc::new(dependencies.secrets),
        Arc::new(dependencies.database),
        SourceDataCachePolicy::uniform(Duration::MAX),
    );
    let repository = test_repository();

    let fresh = source_data
        .workflow_runs("example", &repository, RefreshMode::IfStale)
        .await?;
    dependencies
        .fail_workflow_runs
        .store(true, Ordering::SeqCst);
    let stale = source_data
        .workflow_runs("example", &repository, RefreshMode::Force)
        .await?;
    let cached_failure = source_data
        .workflow_runs("example", &repository, RefreshMode::IfStale)
        .await?;

    assert_eq!(stale.runs, fresh.runs);
    assert!(stale.stale);
    assert_eq!(cached_failure, stale);
    assert_eq!(
        stale.error,
        Some(ConnectionValidationFailure::ProviderUnavailable)
    );
    assert_eq!(dependencies.workflow_run_requests.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn source_modules_can_omit_change_request_monitoring()
-> Result<(), Box<dyn std::error::Error>> {
    let dependencies = dependencies()?;
    let module = dependencies
        .registry
        .get("example")
        .expect("test source module");
    let descriptor = module.descriptor();
    let token = ProviderToken::new("secret".to_owned());

    assert!(descriptor.supports(SourceCapability::Workflows));
    assert!(!descriptor.supports(SourceCapability::ChangeRequests));
    assert_eq!(
        module
            .list_change_requests(&ConnectionConfiguration::new(), &token, &test_repository(),)
            .await?,
        None
    );
    assert_eq!(
        module
            .change_request_details(
                &ConnectionConfiguration::new(),
                &token,
                &test_repository(),
                42,
            )
            .await?,
        None
    );
    Ok(())
}
