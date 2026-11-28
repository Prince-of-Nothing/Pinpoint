//! Regression tests for the deployment files: no known database password may
//! be a silent default, and Postgres must not be published to every interface.
//! Passwords are required from the environment (`.env`), and the database port
//! is bound to loopback only.

use std::fs;
use std::path::Path;

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Development passwords that ship in the public repository. None may appear
/// in a deployment file, not even as a fallback.
const KNOWN_DEFAULTS: &[&str] = &[
    "pinpoint-local-dev",
    "pinpoint-auth-dev",
    "pinpoint-users-dev",
    "pinpoint-locations-dev",
    "pinpoint-messages-dev",
    "pinpoint-diag-dev",
    "pinpoint-repl-dev",
];

const DEPLOY_FILES: &[&str] = &[
    "docker-compose.yml",
    "docker-compose.ha.yml",
    "db/init/01-roles.sh",
];

#[test]
fn deployment_files_contain_no_known_password() {
    for rel in DEPLOY_FILES {
        let text = read(rel);
        for secret in KNOWN_DEFAULTS {
            assert!(
                !text.contains(secret),
                "{rel} still contains a known development password"
            );
        }
    }
}

#[test]
fn password_variables_have_no_fallback_value() {
    for rel in DEPLOY_FILES {
        for line in read(rel).lines() {
            let is_secret =
                line.contains("PASSWORD") || line.contains("_PWD") || line.contains("PGPASSWORD");
            assert!(
                !(is_secret && line.contains(":-")),
                "{rel} gives a password variable a fallback value: {line}"
            );
        }
    }
}

#[test]
fn postgres_superuser_password_comes_from_the_environment() {
    let text = read("docker-compose.yml");
    assert!(
        text.contains("POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:?"),
        "docker-compose.yml must require POSTGRES_PASSWORD from the environment"
    );
}

#[test]
fn postgres_is_bound_to_loopback_only() {
    let text = read("docker-compose.yml");
    for line in text.lines() {
        assert!(
            !line.contains("5432:5432") || line.contains("127.0.0.1:5432:5432"),
            "Postgres is published to all interfaces: {line}"
        );
    }
}

#[test]
fn operator_ports_are_bound_to_loopback_only() {
    // Diagnostics and the admin API are reached through the gateway, so the host
    // should never expose them directly on every interface.
    let text = read("docker-compose.yml");
    for port in ["8005:8005", "8006:8006"] {
        for line in text.lines().filter(|l| l.contains(port)) {
            assert!(
                line.contains(&format!("127.0.0.1:{port}")),
                "port {port} is published to all interfaces: {line}"
            );
        }
    }
}

#[test]
fn gateway_admin_route_is_limited_to_local_networks() {
    // The admin API is role-gated, but the public gateway must still refuse it
    // to clients outside the host and the Docker network.
    let text = read("docker/nginx-gateway.conf");
    let start = text
        .find("location /api/v1/admin/ {")
        .expect("admin location in the gateway");
    let rest = &text[start..];
    let block = &rest[..rest.find('}').expect("end of admin location")];
    for allowed in ["allow 127.0.0.1;", "allow 172.16.0.0/12;"] {
        assert!(
            block.contains(allowed),
            "admin route is missing `{allowed}`"
        );
    }
    assert!(
        block.contains("deny all;"),
        "admin route must deny clients that are not allowed"
    );
    assert!(
        !block.contains("allow all;") && !block.contains("0.0.0.0/0"),
        "admin route must not be open to every client"
    );
}

#[test]
fn backups_are_encrypted_and_need_a_passphrase() {
    let text = read("docker-compose.yml");
    assert!(
        text.contains("openssl enc -aes-256-cbc -pbkdf2"),
        "db-backup must encrypt dumps"
    );
    assert!(
        text.contains("PINPOINT_BACKUP_PASSPHRASE: ${PINPOINT_BACKUP_PASSPHRASE:?"),
        "the backup passphrase must be required from the environment"
    );
    assert!(
        !text.contains("/backups/pinpoint-$$ts.sql"),
        "db-backup must not write plaintext .sql dumps"
    );
}

#[test]
fn service_roles_cannot_read_credential_columns_of_users() {
    // diag, locations and messages never check passwords or MFA, so they get only
    // the non-credential columns of pinpoint_users, never the whole table.
    let text = read("db/02-grants.sql");
    assert!(
        text.contains("REVOKE SELECT ON pinpoint_users FROM"),
        "grants must revoke the table-level SELECT for existing deployments"
    );
    let secrets = [
        "password_hash",
        "totp_secret",
        "recovery_code_hash",
        "identity_subject",
    ];
    for role in ["pinpoint_diag", "pinpoint_locations", "pinpoint_messages"] {
        let grants: Vec<&str> = text
            .split(';')
            .filter(|stmt| {
                stmt.contains("GRANT")
                    && stmt.contains(&format!("TO {role}"))
                    && stmt.contains("pinpoint_users")
            })
            .collect();
        assert!(
            !grants.is_empty(),
            "{role} should get a column-level grant on pinpoint_users"
        );
        for stmt in grants {
            assert!(
                stmt.contains("SELECT ("),
                "{role} must be granted named columns only: {stmt}"
            );
            for secret in secrets {
                assert!(
                    !stmt.contains(secret),
                    "{role} is granted a credential column"
                );
            }
        }
    }
}

#[test]
fn place_notes_are_granted_to_the_services_that_use_them() {
    let text = read("db/02-grants.sql");
    assert!(
        text.contains("GRANT ALL ON pinpoint_place_notes TO pinpoint_locations"),
        "the locations role needs its place-note table"
    );
    assert!(
        text.contains("GRANT DELETE ON pinpoint_place_notes TO pinpoint_users"),
        "account deletion removes place notes through the users role"
    );
}
