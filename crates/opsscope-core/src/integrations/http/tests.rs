use super::*;
use crate::integrations::test_support::{Exchange, MockApi};

#[tokio::test]
async fn network_errors_are_classified_and_logs_are_bounded() {
    for (status, expected) in [
        (401, Failure::InvalidCredentials),
        (403, Failure::PermissionDenied),
        (429, Failure::RateLimited),
        (503, Failure::ProviderUnavailable),
    ] {
        let mut exchange = Exchange::json(
            "/resource",
            "",
            serde_json::json!({"message":"provider-specific secret"}),
        );
        exchange.status = status;
        let api = MockApi::start(vec![exchange]);
        assert_eq!(
            json::<serde_json::Value>(client().unwrap().get(format!("{}/resource", api.url))).await,
            Err(expected)
        );
        api.finish();
    }
    let api = MockApi::start(vec![Exchange {
        request_body: None,
        path: "/logs".into(),
        authorization: String::new(),
        status: 200,
        headers: String::new(),
        body: "123456789".into(),
    }]);
    assert_eq!(
        log_text(client().unwrap().get(format!("{}/logs", api.url)), 4)
            .await
            .unwrap(),
        ("1234".into(), true)
    );
    api.finish();
}

#[tokio::test]
async fn transport_does_not_follow_provider_redirects_with_credentials() {
    let mut response = Exchange::json(
        "/user",
        "authorization: Bearer test-token",
        serde_json::json!(null),
    );
    response.status = 302;
    response
        .headers
        .push_str("Location: https://attacker.invalid/\r\n");
    let api = MockApi::start(vec![response]);
    let result = json::<serde_json::Value>(
        client()
            .unwrap()
            .get(format!("{}/user", api.url))
            .bearer_auth("test-token"),
    )
    .await;
    assert_eq!(result, Err(Failure::UnexpectedResponse));
    api.finish();
}

#[test]
fn servers_require_secure_origins_without_embedded_credentials() {
    for value in [
        "http://example.com",
        "https://user:secret@example.com",
        "https://example.com/path",
        "https://example.com?token=secret",
        "https://example.com/#fragment",
        "https://exam\tple.com",
    ] {
        let config = [("serverUrl".into(), value.into())].into_iter().collect();
        assert_eq!(
            configure_server(&config, "https://gitlab.com"),
            Err(Failure::InvalidConfiguration)
        );
    }
    let config = [("serverUrl".into(), "https://GITLAB.example.com:443/".into())]
        .into_iter()
        .collect();
    assert_eq!(
        configure_server(&config, "").unwrap().unique_key,
        "https://gitlab.example.com"
    );
}

#[test]
fn route_identifiers_are_encoded_as_individual_segments() {
    let url = endpoint(
        "https://example.com/api/v4/",
        &["projects", "group/sub/repo?x=#y"],
    )
    .unwrap();
    assert_eq!(
        url.as_str(),
        "https://example.com/api/v4/projects/group%2Fsub%2Frepo%3Fx=%23y"
    );
    assert!(endpoint("https://example.com/api/", &[".."]).is_err());
}

#[test]
fn status_errors_are_classified_without_provider_response_bodies() {
    assert_eq!(
        status_failure(StatusCode::UNAUTHORIZED),
        Failure::InvalidCredentials
    );
    assert_eq!(
        status_failure(StatusCode::FORBIDDEN),
        Failure::PermissionDenied
    );
    assert_eq!(
        status_failure(StatusCode::TOO_MANY_REQUESTS),
        Failure::RateLimited
    );
    assert_eq!(
        status_failure(StatusCode::BAD_GATEWAY),
        Failure::ProviderUnavailable
    );
}
