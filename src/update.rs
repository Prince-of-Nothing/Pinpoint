//! GitHub release checker.
//!
//! The app has no auto-updater (Android APKs are installed by the user or
//! Obtainium), so this module answers three questions: is the GitHub release
//! newer, what changed, and which asset should this device download? The check
//! runs at most once a day at launch (opt-out in Profile & settings) or on
//! demand, reads the public GitHub Releases API, and falls back to the Atom
//! feed if the API is unavailable. No token and no account data are used.

#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

/// Public repository whose releases are checked.
pub const GITHUB_REPO: &str = "Prince-of-Nothing/Pinpoint";
pub const RELEASES_URL: &str = "https://github.com/Prince-of-Nothing/Pinpoint/releases";
const RELEASES_ATOM: &str = "https://github.com/Prince-of-Nothing/Pinpoint/releases.atom";
const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/Prince-of-Nothing/Pinpoint/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub title: String,
    pub url: String,
    pub updated: String,
    pub notes: String,
    pub assets: Vec<ReleaseAsset>,
}

impl ReleaseInfo {
    /// Direct APK download for this release. Prefers the universal APK
    /// (`pinpoint-android-<version>.apk`); falls back to the device ABI and
    /// finally the release page if an older release has no assets.
    pub fn download_url(&self) -> Option<&str> {
        let apks: Vec<&ReleaseAsset> = self
            .assets
            .iter()
            .filter(|asset| asset.name.ends_with(".apk"))
            .collect();
        if apks.is_empty() {
            return None;
        }
        let version = self.tag.trim_start_matches(['v', 'V']);
        let universal = format!("pinpoint-android-{version}.apk");
        if let Some(asset) = apks.iter().find(|asset| asset.name == universal) {
            return Some(&asset.url);
        }
        let target = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else if cfg!(target_arch = "arm") {
            "armeabi-v7a"
        } else {
            ""
        };
        if !target.is_empty() {
            if let Some(asset) = apks.iter().find(|asset| asset.name.contains(target)) {
                return Some(&asset.url);
            }
        }
        apks.first().map(|asset| asset.url.as_str())
    }

    pub fn apk_name(&self) -> Option<&str> {
        let version = self.tag.trim_start_matches(['v', 'V']);
        let universal = format!("pinpoint-android-{version}.apk");
        self.assets
            .iter()
            .find(|asset| asset.name == universal)
            .map(|asset| asset.name.as_str())
            .or_else(|| {
                self.assets
                    .iter()
                    .find(|asset| asset.name.ends_with(".apk"))
                    .map(|asset| asset.name.as_str())
            })
    }
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

/// Release to show in the in-app banner, if any. Dismissal is per release
/// tag, so a newer release re-enables the banner.
pub fn banner_release<'a>(
    status: &'a UpdateStatus,
    dismissed_tag: &str,
) -> Option<&'a ReleaseInfo> {
    match status {
        UpdateStatus::Available { latest, .. }
            if !dismissed_tag.trim().eq_ignore_ascii_case(latest.tag.trim()) =>
        {
            Some(latest)
        }
        _ => None,
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
/// no releases yet (a valid state for a young project). The API is preferred
/// because it carries assets and the full release body; the Atom feed is a
/// fallback when the API is rate-limited or unavailable.
#[cfg(not(target_arch = "wasm32"))]
pub async fn check_latest() -> anyhow::Result<Option<ReleaseInfo>> {
    let client = reqwest::Client::builder()
        // This environment's IPv6 path is broken; the tile proxy uses the
        // same IPv4 binding for the same reason.
        .local_address(Some(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)))
        .timeout(Duration::from_secs(15))
        .build()?;
    match fetch_api(&client).await {
        Ok(release) => Ok(release),
        Err(api_error) => match fetch_atom(&client).await {
            Ok(release) => Ok(release),
            Err(_) => Err(api_error),
        },
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_api(client: &reqwest::Client) -> anyhow::Result<Option<ReleaseInfo>> {
    let response = client
        .get(LATEST_RELEASE_API)
        .header(
            reqwest::header::USER_AGENT,
            format!("Pinpoint/{} (+{RELEASES_URL})", current_version()),
        )
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success() {
        anyhow::bail!(
            "GitHub releases returned HTTP {}",
            response.status().as_u16()
        );
    }
    let value: serde_json::Value = response.json().await?;
    Ok(parse_release_json(&value))
}

#[cfg(not(target_arch = "wasm32"))]
async fn fetch_atom(client: &reqwest::Client) -> anyhow::Result<Option<ReleaseInfo>> {
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

/// Parse one GitHub Releases API object. Kept pure so tests do not need a
/// network. Returns `None` when the payload has no tag.
pub fn parse_release_json(value: &serde_json::Value) -> Option<ReleaseInfo> {
    let tag = value.get("tag_name")?.as_str()?.trim();
    if tag.is_empty() {
        return None;
    }
    let title = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(tag)
        .to_string();
    let url = value
        .get("html_url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{RELEASES_URL}/tag/{tag}"));
    let updated = value
        .get("published_at")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let notes = value
        .get("body")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let assets = value
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|asset| {
                    let name = asset.get("name")?.as_str()?.to_string();
                    let url = asset.get("browser_download_url")?.as_str()?.to_string();
                    let size = asset
                        .get("size")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    Some(ReleaseAsset { name, url, size })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ReleaseInfo {
        tag: tag.to_string(),
        title,
        url,
        updated,
        notes,
        assets,
    })
}

/// Turn the release Markdown into readable egui text without a parser
/// dependency. Keeps line structure and bullets, drops emphasis markers.
pub fn notes_plain(markdown: &str) -> String {
    let mut out = String::new();
    for line in markdown.replace("\r\n", "\n").lines() {
        let trimmed = line.trim_start();
        let mut text = if let Some(rest) = trimmed.strip_prefix("### ") {
            rest.to_string()
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            rest.to_string()
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            rest.to_string()
        } else if let Some(rest) = trimmed.strip_prefix("- ") {
            format!("• {rest}")
        } else if let Some(rest) = trimmed.strip_prefix("* ") {
            format!("• {rest}")
        } else {
            line.to_string()
        };
        for token in ["**", "__", "`"] {
            text = text.replace(token, "");
        }
        // [label](url) -> label (url)
        while let Some(start) = text.find('[') {
            let Some(mid) = text[start..].find("](") else {
                break;
            };
            let mid = start + mid;
            let Some(end) = text[mid + 2..].find(')') else {
                break;
            };
            let end = mid + 2 + end;
            let label = &text[start + 1..mid];
            let url = &text[mid + 2..end];
            text.replace_range(start..=end, &format!("{label} ({url})"));
        }
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Parse the first entry of GitHub's releases Atom feed. Kept pure so the
/// fallback tests do not need a network.
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
    let notes: String = notes.chars().take(4000).collect();

    if tag.is_empty() && title.is_empty() {
        return None;
    }
    Some(ReleaseInfo {
        tag: if tag.is_empty() { title.clone() } else { tag },
        title,
        url,
        updated,
        notes,
        assets: Vec::new(),
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

    fn api_release() -> serde_json::Value {
        serde_json::json!({
            "tag_name": "v1.3.15",
            "name": "Pinpoint 1.3.15",
            "html_url": "https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v1.3.15",
            "published_at": "2026-11-16T10:00:00Z",
            "body": "## Fixes\n- **Direct APK** link\n- Show `release notes`\n",
            "assets": [
                {"name": "pinpoint-android-1.3.15.apk", "browser_download_url": "https://github.com/Prince-of-Nothing/Pinpoint/releases/download/v1.3.15/pinpoint-android-1.3.15.apk", "size": 11_000_000},
                {"name": "pinpoint-android-1.3.15.apk.idsig", "browser_download_url": "https://example.invalid/idsig", "size": 100}
            ]
        })
    }

    #[test]
    fn compares_dotted_versions() {
        assert!(is_newer("1.2.1", "v1.2.2"));
        assert!(is_newer("1.2.9", "1.2.10"));
        assert!(!is_newer("1.2.1", "v1.2.1"));
        assert!(!is_newer("1.3.0", "1.2.9"));
        assert!(is_newer("1.2.1", "1.2.2-beta"));
    }

    #[test]
    fn parses_release_api_with_direct_apk_and_notes() {
        let release = parse_release_json(&api_release()).expect("release");
        assert_eq!(release.tag, "v1.3.15");
        assert_eq!(release.title, "Pinpoint 1.3.15");
        assert!(release.notes.contains("Direct APK"));
        assert_eq!(
            release.download_url(),
            Some("https://github.com/Prince-of-Nothing/Pinpoint/releases/download/v1.3.15/pinpoint-android-1.3.15.apk")
        );
        assert_eq!(release.apk_name(), Some("pinpoint-android-1.3.15.apk"));
        assert!(notes_plain(&release.notes).contains("• Direct APK link"));
        assert!(notes_plain(&release.notes).contains("release notes"));
    }

    #[test]
    fn missing_assets_falls_back_to_release_page() {
        let mut value = api_release();
        value["assets"] = serde_json::json!([]);
        let release = parse_release_json(&value).expect("release");
        assert!(release.download_url().is_none());
        assert_eq!(
            release.url,
            "https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v1.3.15"
        );
    }

    #[test]
    fn parses_atom_fallback_without_assets() {
        let release = parse_feed(FEED).expect("entry");
        assert_eq!(release.tag, "v1.2.2");
        assert!(release.notes.contains("Route fixes"));
        assert!(release.download_url().is_none());
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

    #[test]
    fn banner_shows_only_for_a_newer_undismissed_release() {
        let latest = parse_release_json(&api_release()).expect("release");
        let available = UpdateStatus::Available {
            current: "1.3.10".to_string(),
            latest: latest.clone(),
        };
        assert_eq!(
            banner_release(&available, "").map(|r| r.tag.as_str()),
            Some("v1.3.15")
        );
        assert!(banner_release(&available, "v1.3.15").is_none());
        assert!(banner_release(&available, " V1.3.15 ").is_none());
        assert!(banner_release(
            &UpdateStatus::UpToDate {
                current: "1.3.15".to_string()
            },
            ""
        )
        .is_none());
        assert!(banner_release(&UpdateStatus::Checking, "").is_none());
    }
}
