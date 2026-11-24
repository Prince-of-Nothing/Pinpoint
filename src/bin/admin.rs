//! Pinpoint dedicated admin API service (port 8006).
//!
//! Run: `cargo run --bin pinpoint-admin`
//! Env: `BIND_ADDR` (default `0.0.0.0:8006`), `DATABASE_URL` (optional).
//!
//! Mounts the same role-gated `/api/v1/admin/*` router as the combined server,
//! plus report triage and the append-only audit trail. Every handler calls
//! `require_admin`; this binary does not relax those gates.
//!
//! Bootstrap an existing account once:
//!
//! ```text
//! DATABASE_URL=postgres://... cargo run --bin pinpoint-admin -- \
//!   --promote-email owner@example.com
//! ```
//!
//! The promotion path only updates an existing active row. It never creates a
//! user, so it cannot reintroduce a demo or seeded account.

use pinpoint_desktop::server::{
    admin_router, converge, health, security_headers, session_auth, status_routes, AppState,
    ServiceKind,
};
use std::time::Duration;

const USAGE: &str = "\
Usage:
  pinpoint-admin
  pinpoint-admin --promote-email <email>
  pinpoint-admin --help

Without arguments the role-gated admin API starts on BIND_ADDR (default
0.0.0.0:8006). --promote-email updates one existing active account to the
admin role and exits; DATABASE_URL is required.";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Serve,
    Promote(String),
    Help,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    match parse_args(std::env::args().skip(1))? {
        Command::Serve => serve().await,
        Command::Promote(email) => promote(&email).await,
        Command::Help => {
            println!("{USAGE}");
            Ok(())
        }
    }
}

async fn serve() -> anyhow::Result<()> {
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
        ))
        .layer(axum::middleware::from_fn(security_headers));
    axum::serve(listener, app).await?;
    Ok(())
}

async fn promote(email: &str) -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL is required to promote an account"))?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    let result = sqlx::query(
        "UPDATE pinpoint_users SET role = 'admin' \
         WHERE lower(email) = lower($1) AND is_active = TRUE",
    )
    .bind(email)
    .execute(&pool)
    .await?;
    if result.rows_affected() == 0 {
        anyhow::bail!(
            "no active account found for {email}; register the account first, then retry"
        );
    }
    println!("promoted {email} to admin");
    Ok(())
}

fn parse_args<I>(args: I) -> anyhow::Result<Command>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Command::Serve);
    };
    match first.as_str() {
        "--promote-email" => {
            let email = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("--promote-email requires an email address"))?;
            if args.next().is_some() {
                anyhow::bail!("unexpected arguments after the email address");
            }
            let email = email.trim();
            if email.is_empty() || !email.contains('@') {
                anyhow::bail!("--promote-email expects a valid email address");
            }
            Ok(Command::Promote(email.to_string()))
        }
        "--help" | "-h" => Ok(Command::Help),
        other => anyhow::bail!("unknown argument `{other}`\n\n{USAGE}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn no_arguments_starts_the_service() {
        assert_eq!(parse_args(args(&[])).unwrap(), Command::Serve);
    }

    #[test]
    fn promote_email_is_parsed_and_trimmed() {
        assert_eq!(
            parse_args(args(&["--promote-email", " owner@example.com "])).unwrap(),
            Command::Promote("owner@example.com".to_string())
        );
    }

    #[test]
    fn help_does_not_start_the_service() {
        assert_eq!(parse_args(args(&["--help"])).unwrap(), Command::Help);
        assert_eq!(parse_args(args(&["-h"])).unwrap(), Command::Help);
    }

    #[test]
    fn missing_or_invalid_email_is_rejected() {
        assert!(parse_args(args(&["--promote-email"])).is_err());
        assert!(parse_args(args(&["--promote-email", "not-an-email"])).is_err());
        assert!(parse_args(args(&["--promote-email", "a@b", "extra"])).is_err());
    }

    #[test]
    fn unknown_arguments_are_rejected() {
        assert!(parse_args(args(&["--promote"])).is_err());
    }
}
