use super::*;
use tower::ServiceExt;

#[tokio::test]
async fn middleware_marks_only_missing_sessions_not_provider_errors() {
    let authentication = WebAuthentication::for_tests();
    let session = authentication.issue_test_session();
    let app = Router::new()
        .route("/", get(|| async { StatusCode::UNAUTHORIZED }))
        .layer(axum::middleware::from_fn_with_state(
            authentication.clone(),
            require_authenticated_session,
        ));
    let anonymous = app
        .clone()
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(anonymous.headers()[SESSION_ERROR_HEADER], "unauthenticated");
    let provider_failure = app
        .oneshot(
            Request::get("/")
                .header(axum::http::header::COOKIE, &session.cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(provider_failure.status(), StatusCode::UNAUTHORIZED);
    assert!(
        !provider_failure
            .headers()
            .contains_key(SESSION_ERROR_HEADER)
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        HeaderValue::from_str(&session.cookie).unwrap(),
    );
    assert!(authentication.session_from_headers(&headers).is_some());
}

#[tokio::test]
async fn login_endpoint_returns_retry_after_when_rate_limited() {
    let mut authentication = WebAuthentication::for_tests();
    authentication.oidc = Some(Arc::new(OidcConfiguration {
        provider: serde_json::from_value(serde_json::json!({
            "issuer": "https://identity.example",
            "authorization_endpoint": "https://identity.example/authorize",
            "token_endpoint": "https://identity.example/token",
            "jwks_uri": "https://identity.example/keys",
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"]
        }))
        .unwrap(),
        client_id: "opscope".into(),
        client_secret: "test".into(),
        redirect_url: "https://ops.example/api/auth/callback".into(),
        http_client: reqwest::Client::new(),
    }));
    for _ in 0..10 {
        let response = login(
            Extension(authentication.clone()),
            Query(LoginQuery {
                return_to: Some("/settings".into()),
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    }
    let response = login(
        Extension(authentication),
        Query(LoginQuery {
            return_to: None,
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[axum::http::header::RETRY_AFTER], "60");
    assert_eq!(response.headers()[CACHE_CONTROL], "no-store");
    assert!(!response.headers().contains_key(SET_COOKIE));
}

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
        "/\t/attacker.example",
        "/\0",
        "/\u{7f}",
        "/\u{85}",
        "/.//attacker.example",
        "/%2e//attacker.example",
    ] {
        assert_eq!(safe_return_to(Some(unsafe_path)), "/");
        assert_eq!(return_location(unsafe_path), HeaderValue::from_static("/"));
    }
}

#[test]
fn query_decoding_cannot_hide_control_characters() {
    for query in ["returnTo=/%09/attacker.example", "returnTo=/%00"] {
        let uri = format!("/api/auth/login?{query}").parse().unwrap();
        let Query(query) = Query::<LoginQuery>::try_from_uri(&uri).unwrap();
        assert_eq!(safe_return_to(query.return_to.as_deref()), "/");
    }
    assert_eq!(
        safe_return_to(Some("/a/../settings?tab=sync#notifications")),
        "/settings?tab=sync#notifications"
    );
    assert_eq!(safe_return_to(Some(&format!("/{}", "x".repeat(2048)))), "/");
    assert_eq!(
        redirect_with_cookie("/\0", "test=value".into()).status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}
