//! Gateway trust boundary: the bundled nginx gateway must overwrite any
//! client-supplied `X-Forwarded-For`; otherwise the per-client throttles can be
//! bypassed by rotating a spoofed header.
//!
//! nginx does not inherit server-level `proxy_set_header` directives into a
//! location that defines any of its own, so every proxied location must stay
//! free of them and inherit the complete set from the server block.

const HTTP: &str = include_str!("../docker/nginx-gateway.conf");
const HTTPS: &str = include_str!("../docker/nginx-gateway-https.conf");

const HEADERS: [&str; 4] = [
    "proxy_set_header Host $host;",
    "proxy_set_header X-Real-IP $remote_addr;",
    "proxy_set_header X-Forwarded-For $remote_addr;",
    "proxy_set_header X-Forwarded-Proto $pinpoint_proto;",
];

#[test]
fn gateway_overwrites_forwarded_for() {
    for (name, config) in [("http", HTTP), ("https", HTTPS)] {
        assert!(
            !config.contains("$proxy_add_x_forwarded_for"),
            "{name}: appending lets a client spoof the first hop"
        );
        let first_location = config
            .find("\n    location ")
            .unwrap_or_else(|| panic!("{name}: no location block"));
        let server_head = &config[..first_location];
        for header in HEADERS {
            assert_eq!(
                config.matches(header).count(),
                1,
                "{name}: {header} must appear exactly once"
            );
            assert!(
                server_head.contains(header),
                "{name}: {header} must be set at server level"
            );
        }
        assert!(
            !config[first_location..].contains("proxy_set_header"),
            "{name}: a location-level proxy_set_header would drop the inherited set"
        );
    }
}
