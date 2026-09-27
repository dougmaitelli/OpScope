use async_trait::async_trait;
use semver::Version;
use serde::Deserialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const LATEST_RELEASE_API_URL: &str =
    "https://api.github.com/repos/dougmaitelli/OpsScope/releases/latest";
const UPDATE_CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const UPDATE_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseUpdate {
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub update_available: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateCheckFailure {
    ProviderUnavailable,
    InvalidRelease,
}

impl Display for UpdateCheckFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ProviderUnavailable => "release provider unavailable",
            Self::InvalidRelease => "release provider returned an invalid version",
        })
    }
}

impl Error for UpdateCheckFailure {}

#[derive(Clone, Debug)]
struct LatestRelease {
    version: Version,
    url: String,
}

#[async_trait]
trait ReleaseProvider: Send + Sync {
    async fn latest_release(&self) -> Result<LatestRelease, UpdateCheckFailure>;
}

#[derive(Clone)]
pub struct CheckForUpdates {
    current_version: Version,
    provider: Arc<dyn ReleaseProvider>,
    cache: Arc<Mutex<Option<CachedRelease>>>,
    cache_ttl: Duration,
}

#[derive(Clone)]
struct CachedRelease {
    release: LatestRelease,
    checked_at: Instant,
}

impl CheckForUpdates {
    #[must_use]
    pub fn github() -> Self {
        Self::new(
            Version::parse(env!("CARGO_PKG_VERSION"))
                .expect("package version must be valid semver"),
            Arc::new(GithubReleaseProvider::default()),
            UPDATE_CACHE_TTL,
        )
    }

    fn new(
        current_version: Version,
        provider: Arc<dyn ReleaseProvider>,
        cache_ttl: Duration,
    ) -> Self {
        Self {
            current_version,
            provider,
            cache: Arc::new(Mutex::new(None)),
            cache_ttl,
        }
    }

    pub async fn execute(&self) -> Result<ReleaseUpdate, UpdateCheckFailure> {
        let cached = self.cache.lock().ok().and_then(|cache| {
            cache
                .as_ref()
                .filter(|cached| cached.checked_at.elapsed() < self.cache_ttl)
                .cloned()
        });
        let release = match cached {
            Some(cached) => cached.release,
            None => {
                let release = self.provider.latest_release().await?;
                if let Ok(mut cache) = self.cache.lock() {
                    *cache = Some(CachedRelease {
                        release: release.clone(),
                        checked_at: Instant::now(),
                    });
                }
                release
            }
        };
        Ok(ReleaseUpdate {
            current_version: self.current_version.to_string(),
            latest_version: release.version.to_string(),
            release_url: release.url,
            update_available: release.version > self.current_version,
        })
    }
}

#[derive(Default)]
struct GithubReleaseProvider {
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    html_url: String,
}

#[async_trait]
impl ReleaseProvider for GithubReleaseProvider {
    async fn latest_release(&self) -> Result<LatestRelease, UpdateCheckFailure> {
        let response = self
            .client
            .get(LATEST_RELEASE_API_URL)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header(reqwest::header::USER_AGENT, "OpsScope update checker")
            .timeout(UPDATE_REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|_| UpdateCheckFailure::ProviderUnavailable)?
            .error_for_status()
            .map_err(|_| UpdateCheckFailure::ProviderUnavailable)?
            .json::<GithubReleaseResponse>()
            .await
            .map_err(|_| UpdateCheckFailure::ProviderUnavailable)?;
        let version = Version::parse(response.tag_name.trim_start_matches('v'))
            .map_err(|_| UpdateCheckFailure::InvalidRelease)?;
        Ok(LatestRelease {
            version,
            url: response.html_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FakeReleaseProvider {
        calls: AtomicUsize,
        release: LatestRelease,
    }

    #[async_trait]
    impl ReleaseProvider for FakeReleaseProvider {
        async fn latest_release(&self) -> Result<LatestRelease, UpdateCheckFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.release.clone())
        }
    }

    #[tokio::test]
    async fn reports_newer_semantic_versions_and_caches_successes() {
        let provider = Arc::new(FakeReleaseProvider {
            calls: AtomicUsize::new(0),
            release: LatestRelease {
                version: Version::new(1, 3, 0),
                url: "https://example.com/releases/v1.3.0".to_owned(),
            },
        });
        let checker = CheckForUpdates::new(
            Version::new(1, 2, 0),
            provider.clone(),
            Duration::from_secs(60),
        );

        let first = checker.execute().await.expect("update check succeeds");
        let second = checker.execute().await.expect("cached check succeeds");

        assert!(first.update_available);
        assert_eq!(first.latest_version, "1.3.0");
        assert_eq!(second, first);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn does_not_offer_an_older_stable_release_to_a_prerelease_build() {
        let provider = Arc::new(FakeReleaseProvider {
            calls: AtomicUsize::new(0),
            release: LatestRelease {
                version: Version::new(1, 9, 0),
                url: "https://example.com/releases/v1.9.0".to_owned(),
            },
        });
        let checker = CheckForUpdates::new(
            Version::parse("2.0.0-beta.1").expect("valid version"),
            provider,
            Duration::from_secs(60),
        );

        assert!(
            !checker
                .execute()
                .await
                .expect("update check succeeds")
                .update_available
        );
    }
}
