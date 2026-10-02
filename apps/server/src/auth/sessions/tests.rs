use super::*;

fn pending() -> PendingLogin {
    PendingLogin {
        state: "state".into(),
        nonce: "nonce".into(),
        pkce_verifier: "verifier".into(),
        return_to: "/".into(),
        expires_at: now() + 600,
    }
}

#[test]
fn login_rate_limit_is_bounded_and_recovers_after_window() {
    let store = SessionStore::default();
    let instant = Instant::now();
    for _ in 0..LOGIN_RATE_LIMIT {
        assert!(store.create_login_at(pending(), instant).is_ok());
    }
    for _ in 0..100 {
        assert!(store.create_login_at(pending(), instant).is_err());
    }
    assert_eq!(store.lock().logins.len(), LOGIN_RATE_LIMIT);
    assert_eq!(store.lock().login_attempts.len(), LOGIN_RATE_LIMIT);
    assert!(
        store
            .create_login_at(pending(), instant + LOGIN_RATE_WINDOW)
            .is_ok()
    );
}

#[test]
fn pending_capacity_preserves_existing_logins_and_reclaims_expired_entries() {
    let store = SessionStore::default();
    let instant = Instant::now();
    let mut first = None;
    for index in 0..MAX_PENDING_LOGINS {
        let time = instant + LOGIN_RATE_WINDOW * (index / LOGIN_RATE_LIMIT) as u32;
        let token = store.create_login_at(pending(), time).unwrap();
        if first.is_none() {
            first = Some(token);
        }
    }
    let later = instant + LOGIN_RATE_WINDOW * 20;
    assert!(store.create_login_at(pending(), later).is_err());
    assert_eq!(store.lock().logins.len(), MAX_PENDING_LOGINS);
    assert!(store.take_login(first.as_deref().unwrap()).is_some());
    assert!(store.take_login(first.as_deref().unwrap()).is_none());
    assert!(store.create_login_at(pending(), later).is_ok());
    for pending in store.lock().logins.values_mut() {
        pending.expires_at = 0;
    }
    assert!(store.create_login_at(pending(), later).is_ok());
    assert_eq!(store.lock().logins.len(), 1);
}

#[test]
fn session_and_callback_lookup_only_expire_the_requested_entry() {
    let store = SessionStore::default();
    let expired = store.create_login(pending()).unwrap();
    store
        .lock()
        .logins
        .get_mut(&token_hash(&expired))
        .unwrap()
        .expires_at = 0;
    let (session, _) = store.create_session(AuthenticatedUser {
        subject: "user".into(),
        email: None,
    });
    let pending_token = store.create_login(pending()).unwrap();
    assert!(store.session(&session).is_some());
    assert!(store.take_login(&pending_token).is_some());
    assert!(store.take_login(&expired).is_none());
    store
        .lock()
        .sessions
        .get_mut(&token_hash(&session))
        .unwrap()
        .expires_at = 0;
    assert!(store.session(&session).is_none());
}

#[test]
fn session_tokens_are_stored_by_hash_and_expire_on_delete() {
    let store = SessionStore::default();
    let (token, _) = store.create_session(AuthenticatedUser {
        subject: "subject".to_owned(),
        email: None,
    });
    let session = store.session(&token).expect("session exists");
    assert!(!store.lock().sessions.contains_key(token.as_bytes()));
    store.delete_session(&session.id);
    assert!(store.session(&token).is_none());
}

#[test]
fn production_cookie_has_host_prefix_and_security_attributes() {
    let cookie = build_cookie("__Host-opscope-session", "token", 60, true, "Strict");
    assert!(cookie.starts_with("__Host-opscope-session=token; Path=/;"));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    assert!(cookie.ends_with("; Secure"));
    assert!(!cookie.contains("Domain="));
}
