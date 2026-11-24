//! Pinpoint diagnostics ingestion service (port 8005).
//!
//! Run: `cargo run --bin pinpoint-diag`
//! Env: `BIND_ADDR` (default `0.0.0.0:8005`), `DATABASE_URL` (optional).
//!
//! Privacy: reports carry a category, a message, an app version/platform and
//! (when signed in) a public handle. No IP, coordinates or device identifiers
//! are read or stored. Admin listing/triage runs on the admin service (8006).

use pinpoint_desktop::server::{
    converge, diagnostic_router, health, security_headers, session_auth, status_routes, AppState,
    ServiceKind,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8005".to_string());
    let state = AppState::from_env_for(ServiceKind::Diag).await;
    let persist_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            persist_state.refresh_from_db().await;
            persist_state.persist().await;
        }
    });
    println!("pinpoint-diag listening on http://{addr}/ (reports only; no IP logging)");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let app_state = state.clone();
    let session_state = state.clone();
    let app = diagnostic_router()
        .merge(status_routes())
        .route("/health", axum::routing::get(health))
        .with_state(state)
        .layer(axum::middleware::from_fn_with_state(app_state, converge))
        .layer(axum::middleware::from_fn_with_state(
            session_state,
            session_auth,
        ))
        .layer(axum::middleware::from_fn(security_headers));
    axum::serve(listener, app).await?;
    Ok(())
}
