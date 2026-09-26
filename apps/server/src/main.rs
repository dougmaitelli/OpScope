use ciwatcher_core::integrations::fake::FakeMonitorSource;
use std::error::Error;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // This unauthenticated skeleton is deliberately loopback-only.
    let listener = TcpListener::bind("127.0.0.1:4317").await?;
    println!("CI Watcher development server listening on http://127.0.0.1:4317");
    axum::serve(listener, ciwatcher_server::router(FakeMonitorSource))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("failed to install shutdown signal handler: {error}");
    }
}
