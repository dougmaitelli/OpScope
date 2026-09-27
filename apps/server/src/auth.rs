mod config;
mod sessions;

use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::{Extension, Query, State};
use axum::http::header::{CACHE_CONTROL, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
pub use config::AuthenticationConfigurationError;
use config::OidcConfiguration;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use sessions::{
    PendingLogin, SESSION_LIFETIME, SessionStore, build_cookie, clear_cookie, cookie_header,
    cookie_value, now, secure_eq,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

pub const AUTH_SESSION_PATH: &str = "/api/auth/session";
pub const AUTH_LOGIN_PATH: &str = "/api/auth/login";
pub const AUTH_CALLBACK_PATH: &str = "/api/auth/callback";
pub const AUTH_LOGOUT_PATH: &str = "/api/auth/logout";
pub const CSRF_HEADER: &str = "x-csrf-token";

const LOGIN_LIFETIME: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedUser {
    pub subject: String,
    pub email: Option<String>,
}

#[derive(Clone)]
pub struct AuthenticatedSession {
    id: [u8; 32],
    pub user: AuthenticatedUser,
    csrf_token: String,
}

#[derive(Clone)]
pub struct WebAuthentication {
    enabled: bool,
    oidc: Option<Arc<OidcConfiguration>>,
    sessions: SessionStore,
    secure_cookies: bool,
    allowed_subjects: Arc<HashSet<String>>,
}

impl WebAuthentication {
    pub async fn from_environment() -> Result<Self, AuthenticationConfigurationError> {
        Ok(match config::from_environment().await? {
            Some(configuration) => Self {
                enabled: true,
                oidc: Some(configuration.oidc),
                sessions: SessionStore::default(),
                secure_cookies: configuration.secure_cookies,
                allowed_subjects: configuration.allowed_subjects,
            },
            None => Self::disabled(),
        })
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn disabled() -> Self {
        Self {
            enabled: false,
            oidc: None,
            sessions: SessionStore::default(),
            secure_cookies: false,
            allowed_subjects: Arc::new(HashSet::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self {
            enabled: true,
            oidc: None,
            sessions: SessionStore::default(),
            secure_cookies: false,
            allowed_subjects: Arc::new(HashSet::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn disabled_for_tests() -> Self {
        Self::disabled()
    }

    #[cfg(test)]
    pub(crate) fn issue_test_session(&self) -> TestSession {
        let user = AuthenticatedUser {
            subject: "test-user".to_owned(),
            email: Some("test@example.com".to_owned()),
        };
        let (token, csrf_token) = self.sessions.create_session(user);
        TestSession {
            cookie: format!("{}={token}", self.session_cookie_name()),
            csrf_token,
        }
    }

    fn session_cookie_name(&self) -> &'static str {
        if self.secure_cookies {
            "__Host-opsscope-session"
        } else {
            "opsscope-session"
        }
    }

    fn login_cookie_name(&self) -> &'static str {
        if self.secure_cookies {
            "__Host-opsscope-oidc"
        } else {
            "opsscope-oidc"
        }
    }

    fn session_from_headers(&self, headers: &HeaderMap) -> Option<AuthenticatedSession> {
        let token = cookie_value(headers, self.session_cookie_name())?;
        self.sessions.session(&token)
    }

    fn is_subject_allowed(&self, subject: &str) -> bool {
        self.allowed_subjects.is_empty() || self.allowed_subjects.contains(subject)
    }
}

#[cfg(test)]
pub(crate) struct TestSession {
    pub(crate) cookie: String,
    pub(crate) csrf_token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthenticationStatus {
    enabled: bool,
    authenticated: bool,
    user: Option<AuthenticatedUser>,
    csrf_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginQuery {
    return_to: Option<String>,
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: String,
    state: String,
}

pub fn routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route(AUTH_SESSION_PATH, get(session_status))
        .route(AUTH_LOGIN_PATH, get(login))
        .route(AUTH_CALLBACK_PATH, get(callback))
}

async fn session_status(
    Extension(authentication): Extension<WebAuthentication>,
    headers: HeaderMap,
) -> Response {
    if !authentication.enabled {
        return no_store(
            Json(AuthenticationStatus {
                enabled: false,
                authenticated: false,
                user: None,
                csrf_token: None,
            })
            .into_response(),
        );
    }
    let session = authentication.session_from_headers(&headers);
    let status = match session {
        Some(session) => AuthenticationStatus {
            enabled: true,
            authenticated: true,
            user: Some(session.user),
            csrf_token: Some(session.csrf_token),
        },
        None => AuthenticationStatus {
            enabled: true,
            authenticated: false,
            user: None,
            csrf_token: None,
        },
    };
    no_store(Json(status).into_response())
}

async fn login(
    Extension(authentication): Extension<WebAuthentication>,
    Query(query): Query<LoginQuery>,
) -> Response {
    let Some(oidc) = &authentication.oidc else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let client = CoreClient::from_provider_metadata(
        oidc.provider.clone(),
        ClientId::new(oidc.client_id.clone()),
        Some(ClientSecret::new(oidc.client_secret.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(oidc.redirect_url.clone()).expect("validated redirect URL"));
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (authorization_url, csrf_state, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_owned()))
        .add_scope(Scope::new("profile".to_owned()))
        .add_scope(Scope::new("email".to_owned()))
        .set_pkce_challenge(pkce_challenge)
        .url();
    let login_token = authentication.sessions.create_login(PendingLogin {
        state: csrf_state.secret().clone(),
        nonce: nonce.secret().clone(),
        pkce_verifier: pkce_verifier.secret().clone(),
        return_to: safe_return_to(query.return_to.as_deref()),
        expires_at: now() + LOGIN_LIFETIME.as_secs(),
    });
    no_store(redirect_with_cookie(
        authorization_url.as_str(),
        build_cookie(
            authentication.login_cookie_name(),
            &login_token,
            LOGIN_LIFETIME.as_secs(),
            authentication.secure_cookies,
            "Lax",
        ),
    ))
}

async fn callback(
    Extension(authentication): Extension<WebAuthentication>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(oidc) = &authentication.oidc else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(login_token) = cookie_value(&headers, authentication.login_cookie_name()) else {
        return authentication_error("OIDC login cookie is missing");
    };
    let Some(pending) = authentication.sessions.take_login(&login_token) else {
        return authentication_error("OIDC login has expired");
    };
    if !secure_eq(&query.state, &pending.state) {
        return authentication_error("OIDC state validation failed");
    }
    let client = CoreClient::from_provider_metadata(
        oidc.provider.clone(),
        ClientId::new(oidc.client_id.clone()),
        Some(ClientSecret::new(oidc.client_secret.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(oidc.redirect_url.clone()).expect("validated redirect URL"));
    let exchange = match client.exchange_code(AuthorizationCode::new(query.code)) {
        Ok(exchange) => exchange,
        Err(_) => return authentication_error("OIDC token endpoint is unavailable"),
    };
    let token_response = match exchange
        .set_pkce_verifier(PkceCodeVerifier::new(pending.pkce_verifier))
        .request_async(&oidc.http_client)
        .await
    {
        Ok(response) => response,
        Err(_) => return authentication_error("OIDC code exchange failed"),
    };
    let Some(id_token) = token_response.id_token() else {
        return authentication_error("OIDC provider returned no ID token");
    };
    let nonce = Nonce::new(pending.nonce);
    let claims = match id_token.claims(&client.id_token_verifier(), &nonce) {
        Ok(claims) => claims,
        Err(_) => return authentication_error("OIDC ID token validation failed"),
    };
    if let Some(expected_hash) = claims.access_token_hash() {
        let verifier = client.id_token_verifier();
        let algorithm = match id_token.signing_alg() {
            Ok(algorithm) => algorithm,
            Err(_) => return authentication_error("OIDC signing algorithm is invalid"),
        };
        let key = match id_token.signing_key(&verifier) {
            Ok(key) => key,
            Err(_) => return authentication_error("OIDC signing key is invalid"),
        };
        let actual_hash =
            match AccessTokenHash::from_token(token_response.access_token(), algorithm, key) {
                Ok(hash) => hash,
                Err(_) => return authentication_error("OIDC access token hash is invalid"),
            };
        if !secure_eq(actual_hash.as_str(), expected_hash.as_str()) {
            return authentication_error("OIDC access token validation failed");
        }
    }
    let subject = claims.subject().as_str();
    if !authentication.is_subject_allowed(subject) {
        return (StatusCode::FORBIDDEN, "OIDC subject is not authorized").into_response();
    }
    let user = AuthenticatedUser {
        subject: subject.to_owned(),
        email: claims.email().map(|email| email.as_str().to_owned()),
    };
    let (session_token, _) = authentication.sessions.create_session(user);
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::SEE_OTHER;
    response.headers_mut().insert(
        LOCATION,
        HeaderValue::from_str(&pending.return_to).expect("validated return path"),
    );
    response.headers_mut().append(
        SET_COOKIE,
        cookie_header(build_cookie(
            authentication.session_cookie_name(),
            &session_token,
            SESSION_LIFETIME.as_secs(),
            authentication.secure_cookies,
            "Strict",
        )),
    );
    response.headers_mut().append(
        SET_COOKIE,
        cookie_header(clear_cookie(
            authentication.login_cookie_name(),
            authentication.secure_cookies,
            "Lax",
        )),
    );
    no_store(response)
}

pub async fn require_authenticated_session(
    State(authentication): State<WebAuthentication>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    if !authentication.enabled {
        return no_store(next.run(request).await);
    }
    let Some(session) = authentication.session_from_headers(request.headers()) else {
        return no_store(StatusCode::UNAUTHORIZED.into_response());
    };
    if is_state_changing(request.method()) {
        let csrf = request
            .headers()
            .get(CSRF_HEADER)
            .and_then(|value| value.to_str().ok());
        if !csrf.is_some_and(|csrf| secure_eq(csrf, &session.csrf_token)) {
            return no_store(StatusCode::FORBIDDEN.into_response());
        }
    }
    request.extensions_mut().insert(session);
    no_store(next.run(request).await)
}

pub async fn logout(
    Extension(authentication): Extension<WebAuthentication>,
    headers: HeaderMap,
) -> Response {
    if let Some(session) = authentication.session_from_headers(&headers) {
        authentication.sessions.delete_session(&session.id);
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        cookie_header(clear_cookie(
            authentication.session_cookie_name(),
            authentication.secure_cookies,
            "Strict",
        )),
    );
    no_store(response)
}

fn safe_return_to(value: Option<&str>) -> String {
    value
        .filter(|value| {
            value.starts_with('/')
                && !value.starts_with("//")
                && !value.contains(['\r', '\n', '\\'])
        })
        .unwrap_or("/")
        .to_owned()
}

fn redirect_with_cookie(location: &str, cookie: String) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::TEMPORARY_REDIRECT;
    response.headers_mut().insert(
        LOCATION,
        HeaderValue::from_str(location).expect("OIDC authorization URL is a valid header"),
    );
    response
        .headers_mut()
        .insert(SET_COOKIE, cookie_header(cookie));
    response
}

fn authentication_error(message: &'static str) -> Response {
    no_store((StatusCode::UNAUTHORIZED, message).into_response())
}

fn is_state_changing(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_paths_cannot_escape_the_application_origin() {
        assert_eq!(
            safe_return_to(Some("/settings?tab=sync")),
            "/settings?tab=sync"
        );
        for unsafe_path in [
            "https://example.com",
            "//example.com",
            "/\\example.com",
            "/\r\nX: y",
        ] {
            assert_eq!(safe_return_to(Some(unsafe_path)), "/");
        }
    }
}
