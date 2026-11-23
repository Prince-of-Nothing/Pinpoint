//! Map service binary (pure Rust).
//! Run: `cargo run --bin pinpoint-map` (BIND_ADDR default 0.0.0.0:8004).
use pinpoint_desktop::server::{map_rate_limit, map_router, status_routes, AppState, ServiceKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8004".to_string());
    println!("pinpoint-map listening on http://{addr}/");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let state = AppState::for_owner(ServiceKind::MapOnly);
    let rate_state = state.clone();
    axum::serve(
        listener,
        map_router().merge(status_routes()).with_state(state).layer(
            axum::middleware::from_fn_with_state(rate_state, map_rate_limit),
        ),
    )
    .await?;
    Ok(())
}
