//! Pinpoint API server (pure Rust).
//!
//! Run: `cargo run --bin pinpoint-server`
//! Env: `BIND_ADDR` (default `127.0.0.1:8000`), `DATABASE_URL`
//! (optional; when set, the state snapshot is persisted to Postgres).

use pinpoint_desktop::server::{router, AppState};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8000".to_string());
    let state = AppState::from_env().await;
    if state.pg.is_some() {
        println!("Postgres persistence enabled (DATABASE_URL set)");
    } else {
        println!("Running in-memory mode (no DATABASE_URL; data is not persisted)");
    }
    state.spawn_location_pruner();

    // Background snapshot-persist so in-memory mutations survive restarts
    // via Postgres without modifying every handler.
    let persist_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            persist_state.refresh_from_db().await;
            persist_state.persist().await;
        }
    });

    let app = router(state);

    println!("Pinpoint API server listening on http://{addr}/");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
