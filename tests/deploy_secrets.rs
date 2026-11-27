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
