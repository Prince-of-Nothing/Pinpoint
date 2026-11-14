//! GitHub release checker.
//!
//! The app has no auto-updater (Android APKs are installed by the user or
//! Obtainium), so this module answers one question: is the GitHub release
//! newer than the running `CARGO_PKG_VERSION`? The check is user-initiated
//! and reads the public Atom feed, so it needs no token and sends no account
//! data. It is intentionally a direct GitHub request; the UI says so.

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

/// Public repository whose releases are checked.
pub const GITHUB_REPO: &str = "Prince-of-Nothing/Pinpoint";
pub const RELEASES_URL: &str = "https://github.com/Prince-of-Nothing/Pinpoint/releases";
const RELEASES_ATOM: &str = "https://github.com/Prince-of-Nothing/Pinpoint/releases.atom";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub title: String,
    pub url: String,
    pub updated: String,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateStatus {
    #[default]
    Unknown,
    Checking,
    UpToDate {
        current: String,
    },
    Available {
        current: String,
        latest: ReleaseInfo,
    },
    NoReleases,
    Error(String),
}

impl UpdateStatus {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available { .. })
    }
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Compare dotted numeric versions after stripping a leading `v` and any
/// pre-release/build suffix. Returns true when `latest` is strictly newer.
pub fn is_newer(current: &str, latest: &str) -> bool {
    parse_version(latest) > parse_version(current)
}

fn parse_version(value: &str) -> (u64, u64, u64) {
    let core = value
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()
        .unwrap_or("");
    let mut parts = core.split('.').map(|part| {
        part.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(0)
    });
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

/// Fetch the newest published GitHub release. `Ok(None)` means the repo has
/// no releases yet (a valid state for a young project).
#[cfg(not(target_arch = "wasm32"))]
pub async fn check_latest() -> anyhow::Result<Option<ReleaseInfo>> {
    let client = reqwest::Client::builder()
        // This environment's IPv6 path is broken; the tile proxy uses the
        // same IPv4 binding for the same reason.
        .local_address(Some(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)))
        .timeout(Duration::from_secs(15))
        .build()?;
    let response = client
        .get(RELEASES_ATOM)
        .header(
            reqwest::header::USER_AGENT,
            format!("Pinpoint/{} (+{RELEASES_URL})", current_version()),
        )
        .header(reqwest::header::ACCEPT, "application/atom+xml")
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!(
            "GitHub releases returned HTTP {}",
            response.status().as_u16()
        );
    }
    let body = response.text().await?;
    Ok(parse_feed(&body))
}

/// WASM stubs do not make external update requests; the native UI owns this.
#[cfg(target_arch = "wasm32")]
pub async fn check_latest() -> anyhow::Result<Option<ReleaseInfo>> {
    anyhow::bail!("update checks are unavailable in the wasm stub")
}

/// Parse the first entry of GitHub's releases Atom feed. Kept pure so the
/// fixture tests do not need a network.
pub fn parse_feed(xml: &str) -> Option<ReleaseInfo> {
    let start = xml.find("<entry>")?;
    let end = xml[start..].find("</entry>")? + start;
    let entry = &xml[start..end];

    let title = tag_text(entry, "title").unwrap_or_default();
    let updated = tag_text(entry, "updated").unwrap_or_default();

    let url = entry
        .find("/releases/tag/")
        .map(|index| {
            let rest = &entry[index..];
            let href_end = rest.find('"').unwrap_or(rest.len());
            format!("https://github.com{}", &rest[..href_end])
        })
        .unwrap_or_else(|| RELEASES_URL.to_string());

    let tag = url
        .rsplit('/')
        .next()
        .map(str::to_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| title.clone());

    let content = tag_text(entry, "content").unwrap_or_default();
    let notes = strip_html(&unescape_xml(&content));
    let notes: String = notes.chars().take(1200).collect();

    if tag.is_empty() && title.is_empty() {
        return None;
    }
    Some(ReleaseInfo {
        tag: if tag.is_empty() { title.clone() } else { tag },
        title,
        url,
        updated,
        notes,
    })
}

fn tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = xml.find(&open)?;
    let after_open = xml[start..].find('>')? + start + 1;
    let close = format!("</{tag}>");
    let end = xml[after_open..].find(&close)? + after_open;
    Some(unescape_xml(xml[after_open..end].trim()))
}

fn unescape_xml(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Turn release-note HTML into compact plain text for egui.
fn strip_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_tag = false;
    for c in value.chars() {
        match c {
            '<' => {
                in_tag = true;
                if !out.ends_with('\n') {
                    out.push(' ');
                }
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <title>v1.2.2 — route polish</title>
    <link rel="alternate" type="text/html" href="https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v1.2.2"/>
    <updated>2026-11-14T10:40:15Z</updated>
    <content type="html">&lt;p&gt;Route fixes &amp;amp; faster tiles.&lt;/p&gt;</content>
  </entry>
</feed>"#;

    #[test]
    fn compares_dotted_versions() {
        assert!(is_newer("1.2.1", "v1.2.2"));
        assert!(is_newer("1.2.9", "1.2.10"));
        assert!(!is_newer("1.2.1", "v1.2.1"));
        assert!(!is_newer("1.3.0", "1.2.9"));
        assert!(is_newer("1.2.1", "1.2.2-beta"));
    }

    #[test]
    fn parses_first_release_from_atom() {
        let release = parse_feed(FEED).expect("entry");
        assert_eq!(release.tag, "v1.2.2");
        assert_eq!(release.title, "v1.2.2 — route polish");
        assert!(release.url.ends_with("/releases/tag/v1.2.2"));
        assert!(release.notes.contains("Route fixes"));
        assert!(!release.notes.contains('<'));
    }

    #[test]
    fn empty_feed_has_no_release() {
        assert!(parse_feed("<feed><title>Release notes</title></feed>").is_none());
    }

    #[test]
    fn strips_release_note_html() {
        assert_eq!(
            strip_html("<p>Hello <strong>world</strong></p>"),
            "Hello world"
        );
    }
}
