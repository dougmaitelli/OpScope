use super::*;
use axum::{Json, Router, routing::post};
use serde_json::{Value, json};
use tokio::net::TcpListener;

#[test]
fn requires_an_http_endpoint() {
    assert!(AppriseNotificationSink::new("https://apprise.test/notify/opscope").is_ok());
    assert!(AppriseNotificationSink::new("ftp://apprise.test/notify").is_err());
    assert!(AppriseNotificationSink::new("not a url").is_err());
}

#[test]
fn serializes_the_apprise_failure_contract_without_tags() {
    let payload = AppriseNotification {
        title: "Workflow failed",
        body: "Build",
        kind: "failure",
        format: "text",
        tag: &[],
    };
    assert_eq!(
        serde_json::to_value(payload).expect("payload serializes"),
        json!({
            "title": "Workflow failed",
            "body": "Build",
            "type": "failure",
            "format": "text"
        })
    );
}

#[test]
fn parses_comma_separated_tags_and_ignores_empty_entries() {
    let sink = AppriseNotificationSink::new("https://apprise.test/notify/opscope")
        .expect("valid endpoint")
        .with_tags(" opscope, , alerts,, ");
    assert_eq!(sink.tags, ["opscope", "alerts"]);
}

#[test]
fn empty_or_whitespace_tags_leave_routing_unfiltered() {
    for tags in ["", "   ", " , , "] {
        let sink = AppriseNotificationSink::new("https://apprise.test/notify/opscope")
            .expect("valid endpoint")
            .with_tags(tags);
        assert!(sink.tags.is_empty());
    }
}

async fn captured_payload(severity: NotificationSeverity) -> Value {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("address");
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let router = Router::new().route(
        "/notify/opscope",
        post(move |Json(payload): Json<Value>| {
            let sender = sender.clone();
            async move {
                sender.send(payload).await.expect("capture payload");
                axum::http::StatusCode::OK
            }
        }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let sink = AppriseNotificationSink::new(&format!("http://{address}/notify/opscope"))
        .expect("valid endpoint")
        .with_tags(" opscope, alerts, ");
    let result = sink
        .send(&Notification {
            repository: opscope_core::domain::Repository {
                id: "repo".into(),
                owner: "owner".into(),
                name: "project".into(),
                description: None,
                visibility: opscope_core::domain::RepositoryVisibility::Private,
                web_url: "https://example.test/owner/project".into(),
            },
            events: opscope_core::application::NotificationEvents::WorkflowFailures(vec![
                opscope_core::application::WorkflowFailureEvent {
                    workflow_id: "build".into(),
                    name: "Build".into(),
                    run_id: Some("run".into()),
                    attempt: Some(1),
                },
            ]),
            severity,
        })
        .await;
    server.abort();
    result.expect("notification sent");
    receiver.try_recv().expect("captured request")
}

#[tokio::test]
async fn sends_routing_tags_in_the_notification_request() {
    let payload = captured_payload(NotificationSeverity::Failure).await;
    assert_eq!(
        payload,
        json!({
            "title": "OpScope - ❌ Workflow failed in owner/project",
            "body": "Build",
            "type": "failure",
            "format": "text",
            "tag": ["opscope", "alerts"]
        })
    );
}

#[tokio::test]
async fn work_item_notifications_are_informational_and_keep_routing_tags() {
    let payload = captured_payload(NotificationSeverity::Info).await;
    assert_eq!(payload["type"], "info");
    assert_eq!(payload["tag"], json!(["opscope", "alerts"]));
}
