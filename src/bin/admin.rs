//! Pinpoint dedicated admin API service (port 8006).
//!
//! Run: `cargo run --bin pinpoint-admin`
//! Env: `BIND_ADDR` (default `0.0.0.0:8006`), `DATABASE_URL` (optional).
//!
//! Mounts the same role-gated `/api/v1/admin/*` router as the combined server,
//! plus report triage and the append-only audit trail. Every handler calls
//! `require_admin`; this binary does not relax those gates.

use pinpoint_desktop::server::{
    admin_router, converge, health, session_auth, status_routes, AppState, ServiceKind,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8006".to_string());
    let state = AppState::from_env_for(ServiceKind::Admin).await;
    let persist_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            persist_state.refresh_from_db().await;
            persist_state.persist().await;
        }
    });
    println!("pinpoint-admin listening on http://{addr}/ (role-gated admin API)");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let app_state = state.clone();
    let session_state = state.clone();
    let app = admin_router()
        .merge(status_routes())
        .route("/health", axum::routing::get(health))
        .with_state(state)
        .layer(axum::middleware::from_fn_with_state(app_state, converge))
        .layer(axum::middleware::from_fn_with_state(
            session_state,
            session_auth,
        ));
    axum::serve(listener, app).await?;
    Ok(())
}
