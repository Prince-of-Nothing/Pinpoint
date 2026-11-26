//! "Sharing with you" strip on the native map: one chip per friend whose
//! location you can see right now, freshest first. A live location (updated in
//! the last 15 minutes) gets a sunflower ring. Tapping a chip flies the map to
//! that friend. The model is plain data so it can be tested without a frame;
//! the drawing helper only calls it.

use super::theme;
use crate::models::FriendLocation;
use chrono::{DateTime, Utc};
use eframe::egui::{self, RichText};

/// A location counts as live for this many minutes after its last update.
pub(crate) const LIVE_WINDOW_MINUTES: i64 = 15;

/// Zoom used when the map flies to a friend, the same as opening a place.
pub(crate) const FLY_ZOOM: u8 = 15;

/// Distance from the bottom of the phone screen to the strip: the tab bar
/// plus an 8 pt gap.
pub(crate) const ABOVE_TAB_BAR: f32 = super::phone_tabs::BAR_HEIGHT + 8.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StripEntry {
    pub user_id: i64,
    pub name: String,
    pub initials: String,
    pub live: bool,
    pub age: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// Where the map should go when a chip is tapped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlyTo {
    pub latitude: f64,
    pub longitude: f64,
    pub zoom: u8,
}

/// The strip's entries for the given friend locations, freshest first. A
/// location whose time cannot be read is kept, not live, and sorts last.
pub(crate) fn strip_entries(locations: &[FriendLocation], now: DateTime<Utc>) -> Vec<StripEntry> {
    let mut rows: Vec<(Option<DateTime<Utc>>, StripEntry)> = locations
        .iter()
        .map(|loc| {
            let name = if loc.user_name.trim().is_empty() {
                "Friend".to_string()
            } else {
                loc.user_name.clone()
            };
            let entry = StripEntry {
                user_id: loc.user_id,
                initials: initials(&name),
                name,
                live: is_live(&loc.updated_at, now),
                age: compact_age(&loc.updated_at, now),
                latitude: loc.latitude,
                longitude: loc.longitude,
            };
            (parse_time(&loc.updated_at), entry)
        })
        .collect();
    // `None` sorts before `Some`, so descending order puts unreadable times last.
    rows.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    rows.into_iter().map(|(_, entry)| entry).collect()
}

fn parse_time(iso: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

/// True while the location is at most 15 minutes old. A time that cannot be
/// read is never live.
pub(crate) fn is_live(updated_at: &str, now: DateTime<Utc>) -> bool {
    parse_time(updated_at).is_some_and(|at| (now - at).num_seconds() <= LIVE_WINDOW_MINUTES * 60)
}

/// Short age for a chip: "now", "5 min", "3 h" or "2 d". Empty when the time
/// cannot be read.
pub(crate) fn compact_age(updated_at: &str, now: DateTime<Utc>) -> String {
    let Some(at) = parse_time(updated_at) else {
        return String::new();
    };
    let seconds = (now - at).num_seconds().max(0);
    let minutes = (seconds + 30) / 60;
    if minutes < 1 {
        "now".to_string()
    } else if minutes < 60 {
        format!("{minutes} min")
    } else {
        let hours = (minutes + 30) / 60;
        if hours < 24 {
            format!("{hours} h")
        } else {
            format!("{} d", (hours + 12) / 24)
        }
    }
}

/// Initials from the first letter of the first and last words, split on
/// spaces and `.`, `_`, `-`. `?` when the name has no letters.
pub(crate) fn initials(name: &str) -> String {
    let words: Vec<&str> = name
        .trim_start_matches('@')
        .split(|c: char| c.is_whitespace() || matches!(c, '.' | '_' | '-'))
        .filter(|word| word.chars().any(char::is_alphabetic))
        .collect();
    let Some(first) = words.first() else {
        return "?".to_string();
    };
    let last = if words.len() > 1 {
        words.last().copied()
    } else {
        None
    };
    let mut out = String::new();
    out.extend(
        first
            .chars()
            .next()
            .into_iter()
            .flat_map(char::to_uppercase),
    );
    if let Some(last) = last {
        out.extend(last.chars().next().into_iter().flat_map(char::to_uppercase));
    }
    out
}

/// Where the map goes for a tapped chip.
pub(crate) fn fly_to(entry: &StripEntry) -> FlyTo {
    FlyTo {
        latitude: entry.latitude,
        longitude: entry.longitude,
        zoom: FLY_ZOOM,
    }
}

/// Draws the strip bottom-left, `bottom_inset` points above the bottom of the
/// screen. Returns where the map should fly when a chip is tapped.
pub(crate) fn draw_live_strip(
    ctx: &egui::Context,
    entries: &[StripEntry],
    bottom_inset: f32,
) -> Option<FlyTo> {
    if entries.is_empty() {
        return None;
    }
    let width = (ctx.screen_rect().width() - 24.0).max(120.0);
    let mut chosen: Option<FlyTo> = None;
    egui::Area::new(egui::Id::new("mobile_live_strip"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -bottom_inset))
        .show(ctx, |ui| {
            ui.set_max_width(width);
            egui::Frame::new()
                .fill(theme::PANEL.gamma_multiply(0.98))
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                .corner_radius(egui::CornerRadius::same(16))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    theme::caption(ui, "Sharing with you");
                    egui::ScrollArea::horizontal()
                        .id_salt("mobile_live_strip_list")
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for entry in entries {
                                    let (ring_width, ring) = if entry.live {
                                        (2.0_f32, theme::DARK.live)
                                    } else {
                                        (1.0_f32, theme::BORDER)
                                    };
                                    let text = format!(
                                        "{}  {} · {}",
                                        entry.initials, entry.name, entry.age
                                    );
                                    let chip = egui::Button::new(
                                        RichText::new(text).size(13.0).color(theme::TEXT),
                                    )
                                    .fill(theme::PANEL_2)
                                    .stroke(egui::Stroke::new(ring_width, ring))
                                    .corner_radius(egui::CornerRadius::same(14))
                                    .min_size(egui::vec2(0.0, 36.0));
                                    if ui.add(chip).clicked() {
                                        chosen = Some(fly_to(entry));
                                    }
                                }
                            });
                        });
                });
        });
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-26T22:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn friend(user_id: i64, name: &str, updated_at: &str) -> FriendLocation {
        FriendLocation {
            user_id,
            user_name: name.to_string(),
            latitude: 47.01,
            longitude: 28.86,
            precision_level: "exact".to_string(),
            updated_at: updated_at.to_string(),
            accuracy: None,
        }
    }

    #[test]
    fn freshest_location_comes_first() {
        let locations = [
            friend(1, "Ana", "2026-11-26T21:00:00Z"),
            friend(2, "Ben", "2026-11-26T21:50:00Z"),
            friend(3, "Cy", "2026-11-26T20:00:00Z"),
        ];
        let order: Vec<i64> = strip_entries(&locations, now())
            .iter()
            .map(|entry| entry.user_id)
            .collect();
        assert_eq!(order, [2, 1, 3]);
    }

    #[test]
    fn a_location_is_live_for_fifteen_minutes_exactly() {
        assert!(is_live("2026-11-26T21:45:00Z", now()), "15 minutes old");
        assert!(!is_live("2026-11-26T21:44:00Z", now()), "16 minutes old");
    }

    #[test]
    fn an_unreadable_time_is_not_live_and_sorts_last() {
        let locations = [
            friend(1, "Ana", "not a time"),
            friend(2, "Ben", "2026-11-26T21:59:00Z"),
        ];
        let entries = strip_entries(&locations, now());
        assert_eq!(entries[0].user_id, 2);
        assert!(!entries[1].live);
        assert_eq!(entries[1].age, "");
    }

    #[test]
    fn age_reads_like_the_web_strip() {
        assert_eq!(compact_age("2026-11-26T22:00:00Z", now()), "now");
        assert_eq!(compact_age("2026-11-26T21:55:00Z", now()), "5 min");
        assert_eq!(compact_age("2026-11-26T19:00:00Z", now()), "3 h");
        assert_eq!(compact_age("2026-11-24T22:00:00Z", now()), "2 d");
    }

    #[test]
    fn initials_come_from_the_first_and_last_word() {
        assert_eq!(initials("Ada Lovelace"), "AL");
        assert_eq!(initials("madonna"), "M");
        assert_eq!(initials("dana.k-r"), "DR");
        assert_eq!(initials("  "), "?");
    }

    #[test]
    fn a_blank_name_shows_as_friend() {
        let entries = strip_entries(&[friend(7, "  ", "2026-11-26T21:59:00Z")], now());
        assert_eq!(entries[0].name, "Friend");
        assert_eq!(entries[0].initials, "F");
    }

    #[test]
    fn tapping_a_chip_flies_to_that_friend_at_the_open_place_zoom() {
        let entries = strip_entries(&[friend(4, "Ana", "2026-11-26T21:59:00Z")], now());
        assert_eq!(
            fly_to(&entries[0]),
            FlyTo {
                latitude: 47.01,
                longitude: 28.86,
                zoom: 15,
            }
        );
    }
}
