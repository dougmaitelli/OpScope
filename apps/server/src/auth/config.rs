use super::AUTH_CALLBACK_PATH;
use openidconnect::IssuerUrl;
use openidconnect::core::CoreProviderMetadata;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::time::Duration;

pub(super) struct AuthenticationConfiguration {
    pub(super) oidc: Arc<OidcConfiguration>,
    pub(super) secure_cookies: bool,
    pub(super) allowed_subjects: Arc<HashSet<String>>,
}

pub(super) struct OidcConfiguration {
    pub(super) provider: CoreProviderMetadata,
    pub(super) client_id: String,
    pub(super) client_secret: String,
    pub(super) redirect_url: String,
    pub(super) http_client: openidconnect::reqwest::Client,
}

#[derive(Debug)]
pub struct AuthenticationConfigurationError(&'static str);

impl Display for AuthenticationConfigurationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for AuthenticationConfigurationError {}

pub(super) async fn from_environment()
-> Result<AuthenticationConfiguration, AuthenticationConfigurationError> {
    let issuer = validate_issuer_url(&required_environment("CIWATCHER_OIDC_ISSUER")?)?;
    let client_id = required_environment("CIWATCHER_OIDC_CLIENT_ID")?;
    let client_secret = required_environment("CIWATCHER_OIDC_CLIENT_SECRET")?;
    let public_url = validate_public_url(&required_environment("CIWATCHER_PUBLIC_URL")?)?;
    let secure_cookies = public_url.starts_with("https://");
    let redirect_url = format!("{public_url}{AUTH_CALLBACK_PATH}");
    let http_client = openidconnect::reqwest::ClientBuilder::new()
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| AuthenticationConfigurationError("failed to build OIDC HTTP client"))?;
    let provider = CoreProviderMetadata::discover_async(
        IssuerUrl::new(issuer)
            .map_err(|_| AuthenticationConfigurationError("invalid OIDC issuer URL"))?,
        &http_client,
    )
    .await
    .map_err(|_| AuthenticationConfigurationError("OIDC discovery failed"))?;
    let allowed_subjects = env::var("CIWATCHER_OIDC_ALLOWED_SUBJECTS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
        .map(str::to_owned)
        .collect();

    Ok(AuthenticationConfiguration {
        oidc: Arc::new(OidcConfiguration {
            provider,
            client_id,
            client_secret,
            redirect_url,
            http_client,
        }),
        secure_cookies,
        allowed_subjects: Arc::new(allowed_subjects),
    })
}

fn required_environment(name: &'static str) -> Result<String, AuthenticationConfigurationError> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(AuthenticationConfigurationError(match name {
            "CIWATCHER_OIDC_ISSUER" => "CIWATCHER_OIDC_ISSUER is required",
            "CIWATCHER_OIDC_CLIENT_ID" => "CIWATCHER_OIDC_CLIENT_ID is required",
            "CIWATCHER_OIDC_CLIENT_SECRET" => "CIWATCHER_OIDC_CLIENT_SECRET is required",
            "CIWATCHER_PUBLIC_URL" => "CIWATCHER_PUBLIC_URL is required",
            _ => "required authentication environment variable is missing",
        }))
}

fn validate_public_url(value: &str) -> Result<String, AuthenticationConfigurationError> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| AuthenticationConfigurationError("CIWATCHER_PUBLIC_URL is invalid"))?;
    if !uses_secure_transport(&url) {
        return Err(AuthenticationConfigurationError(
            "CIWATCHER_PUBLIC_URL must use HTTPS except on loopback",
        ));
    }
    if url.cannot_be_a_base()
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(AuthenticationConfigurationError(
            "CIWATCHER_PUBLIC_URL must contain only an origin",
        ));
    }
    Ok(value.trim_end_matches('/').to_owned())
}

fn validate_issuer_url(value: &str) -> Result<String, AuthenticationConfigurationError> {
    let url = reqwest::Url::parse(value)
        .map_err(|_| AuthenticationConfigurationError("CIWATCHER_OIDC_ISSUER is invalid"))?;
    if !uses_secure_transport(&url) {
        return Err(AuthenticationConfigurationError(
            "CIWATCHER_OIDC_ISSUER must use HTTPS except on loopback",
        ));
    }
    if url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AuthenticationConfigurationError(
            "CIWATCHER_OIDC_ISSUER must not contain credentials, a query, or a fragment",
        ));
    }
    Ok(value.trim_end_matches('/').to_owned())
}

fn uses_secure_transport(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        || url.scheme() == "http"
            && url.host_str().is_some_and(|host| {
                host == "localhost"
                    || host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_url_requires_https_except_for_loopback() {
        assert_eq!(
            validate_public_url("https://ci.example.com/").expect("HTTPS is accepted"),
            "https://ci.example.com"
        );
        assert!(validate_public_url("http://127.0.0.1:4317").is_ok());
        assert!(validate_public_url("http://ci.example.com").is_err());
        assert!(validate_public_url("https://ci.example.com/path").is_err());
    }

    #[test]
    fn issuer_requires_a_secure_transport_but_may_have_a_tenant_path() {
        assert_eq!(
            validate_issuer_url("https://id.example.com/tenant/").expect("issuer is valid"),
            "https://id.example.com/tenant"
        );
        assert!(validate_issuer_url("http://localhost:8080/tenant").is_ok());
        assert!(validate_issuer_url("http://id.example.com/tenant").is_err());
        assert!(validate_issuer_url("https://id.example.com/tenant?key=value").is_err());
    }
}
