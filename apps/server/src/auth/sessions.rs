use super::{AuthenticatedSession, AuthenticatedUser};
use axum::http::header::COOKIE;
use axum::http::{HeaderMap, HeaderValue};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) const SESSION_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);
const MAX_PENDING_LOGINS: usize = 64;
const LOGIN_RATE_LIMIT: usize = 10;
const LOGIN_RATE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone, Default)]
pub(super) struct SessionStore {
    state: Arc<Mutex<SessionState>>,
}

#[derive(Default)]
struct SessionState {
    login_attempts: VecDeque<Instant>,
    sessions: HashMap<[u8; 32], StoredSession>,
    logins: HashMap<[u8; 32], PendingLogin>,
}

struct StoredSession {
    user: AuthenticatedUser,
    csrf_token: String,
    expires_at: u64,
}

pub(super) struct PendingLogin {
    pub(super) state: String,
    pub(super) nonce: String,
    pub(super) pkce_verifier: String,
    pub(super) return_to: String,
    pub(super) expires_at: u64,
}

impl SessionStore {
    pub(super) fn create_login(&self, pending: PendingLogin) -> Result<String, ()> {
        self.create_login_at(pending, Instant::now())
    }

    fn create_login_at(&self, pending: PendingLogin, instant: Instant) -> Result<String, ()> {
        let mut state = self.lock();
        while state
            .login_attempts
            .front()
            .is_some_and(|attempt| instant.duration_since(*attempt) >= LOGIN_RATE_WINDOW)
        {
            state.login_attempts.pop_front();
        }
        if state.login_attempts.len() >= LOGIN_RATE_LIMIT {
            return Err(());
        }
        state.login_attempts.push_back(instant);
        state.remove_expired();
        if state.logins.len() >= MAX_PENDING_LOGINS {
            return Err(());
        }
        let token = random_token();
        let id = token_hash(&token);
        state.logins.insert(id, pending);
        Ok(token)
    }

    pub(super) fn take_login(&self, token: &str) -> Option<PendingLogin> {
        let mut state = self.lock();
        state
            .logins
            .remove(&token_hash(token))
            .filter(|pending| pending.expires_at > now())
    }

    pub(super) fn create_session(&self, user: AuthenticatedUser) -> (String, String) {
        let token = random_token();
        let csrf_token = random_token();
        let id = token_hash(&token);
        let mut state = self.lock();
        state.remove_expired();
        state.sessions.insert(
            id,
            StoredSession {
                user,
                csrf_token: csrf_token.clone(),
                expires_at: now() + SESSION_LIFETIME.as_secs(),
            },
        );
        (token, csrf_token)
    }

    pub(super) fn session(&self, token: &str) -> Option<AuthenticatedSession> {
        let id = token_hash(token);
        let mut state = self.lock();
        if state
            .sessions
            .get(&id)
            .is_some_and(|session| session.expires_at <= now())
        {
            state.sessions.remove(&id);
            return None;
        }
        let session = state.sessions.get(&id)?;
        Some(AuthenticatedSession {
            id,
            user: session.user.clone(),
            csrf_token: session.csrf_token.clone(),
        })
    }

    pub(super) fn delete_session(&self, id: &[u8; 32]) {
        self.lock().sessions.remove(id);
    }

    fn lock(&self) -> MutexGuard<'_, SessionState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl SessionState {
    fn remove_expired(&mut self) {
        let now = now();
        self.sessions.retain(|_, session| session.expires_at > now);
        self.logins.retain(|_, login| login.expires_at > now);
    }
}

pub(super) fn secure_eq(left: &str, right: &str) -> bool {
    token_hash(left) == token_hash(right)
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|header| header.to_str().ok())
        .flat_map(|header| header.split(';'))
        .filter_map(|cookie| cookie.trim().split_once('='))
        .find_map(|(cookie_name, value)| (cookie_name == name).then(|| value.to_owned()))
}

pub(super) fn build_cookie(
    name: &str,
    value: &str,
    max_age: u64,
    secure: bool,
    same_site: &str,
) -> String {
    format!(
        "{name}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite={same_site}{}",
        if secure { "; Secure" } else { "" }
    )
}

pub(super) fn clear_cookie(name: &str, secure: bool, same_site: &str) -> String {
    build_cookie(name, "", 0, secure, same_site)
}

pub(super) fn cookie_header(value: String) -> HeaderValue {
    HeaderValue::from_str(&value).expect("cookie contains only validated values")
}

fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).expect("operating system randomness is unavailable");
    URL_SAFE_NO_PAD.encode(bytes)
}

fn token_hash(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

#[cfg(test)]
mod tests;
