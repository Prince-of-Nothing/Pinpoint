//! Compose exposure: only the edge may publish a host port. The gateway binds to
//! loopback and the optional `public` profile adds Caddy. Postgres, diagnostics
//! and admin stay on the internal network, so nothing on the host reaches them
//! around the gateway's TLS, headers and rate limits. Native development opts
//! Postgres back in through `docker-compose.dev-db.yml`.

const COMPOSE: &str = include_str!("../docker-compose.yml");

#[test]
fn only_the_gateway_and_caddy_publish_host_ports() {
    let mut service = String::new();
    let mut publishing = Vec::new();
    for line in COMPOSE.lines() {
        // Service names sit two spaces in, directly under `services:`.
        if let Some(name) = line
            .strip_prefix("  ")
            .and_then(|rest| rest.strip_suffix(':'))
        {
            if !name.starts_with(' ') && !name.starts_with('#') {
                service = name.to_string();
            }
        }
        if line == "    ports:" {
            publishing.push(service.clone());
        }
    }
    publishing.sort();
    assert_eq!(publishing, ["caddy", "gateway"]);
}

#[test]
fn database_and_service_passwords_have_no_default() {
    for needle in ["PWD:-", "PASSWORD:-"] {
        assert!(
            !COMPOSE.contains(needle),
            "docker-compose.yml must not fall back to a default for {needle}"
        );
    }
}
