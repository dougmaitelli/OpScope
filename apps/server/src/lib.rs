//! Development-only HTTP composition. Authentication is added before exposure.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use ciwatcher_core::application::{ListMonitors, MonitorSource};
use ciwatcher_core::contracts::{
    HEALTH_HTTP_PATH, HealthResponse, LIST_MONITORS_HTTP_PATH, ListMonitorsResponse,
};

#[derive(Clone, Debug)]
struct AppState<S> {
    list_monitors: ListMonitors<S>,
}

pub fn router<S>(source: S) -> Router
where
    S: MonitorSource + Clone + 'static,
{
    let state = AppState {
        list_monitors: ListMonitors::new(source),
    };

    Router::new()
        .route(HEALTH_HTTP_PATH, get(health))
        .route(LIST_MONITORS_HTTP_PATH, get(list_monitors::<S>))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse::ready())
}

async fn list_monitors<S>(
    State(state): State<AppState<S>>,
) -> Result<Json<ListMonitorsResponse>, StatusCode>
where
    S: MonitorSource,
{
    state
        .list_monitors
        .execute()
        .await
        .map(ListMonitorsResponse::from_domain)
        .map(Json)
        .map_err(|_| StatusCode::BAD_GATEWAY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use ciwatcher_core::integrations::fake::FakeMonitorSource;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn both_routes_use_the_shared_contract() -> Result<(), Box<dyn std::error::Error>> {
        let app = router(FakeMonitorSource);

        let health = app
            .clone()
            .oneshot(Request::get(HEALTH_HTTP_PATH).body(Body::empty())?)
            .await?;
        assert_eq!(health.status(), StatusCode::OK);

        let monitors = app
            .oneshot(Request::get(LIST_MONITORS_HTTP_PATH).body(Body::empty())?)
            .await?;
        assert_eq!(monitors.status(), StatusCode::OK);
        let body = monitors.into_body().collect().await?.to_bytes();
        let decoded: ListMonitorsResponse = serde_json::from_slice(&body)?;
        assert_eq!(decoded.monitors.len(), 1);
        Ok(())
    }
}
