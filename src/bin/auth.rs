//! Auth service binary (pure Rust).
//! Run: `cargo run --bin pinpoint-auth` (BIND_ADDR default 0.0.0.0:8001).
use pinpoint_desktop::server::{
    auth_router, converge, health, security_headers, session_auth, status_routes, AppState,
    ServiceKind,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8001".to_string());
    let state = AppState::from_env_for(ServiceKind::Auth).await;
    let persist_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            persist_state.refresh_from_db().await;
            persist_state.persist().await;
        }
    });
    println!("pinpoint-auth listening on http://{addr}/");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let app_state = state.clone();
    let session_state = state.clone();
    let app = auth_router()
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
