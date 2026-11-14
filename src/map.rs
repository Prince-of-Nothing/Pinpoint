//! Interactive Slippy map canvas for friend locations, offline tile packs,
//! and ODbL-compliant OpenStreetMap hand-off.

use crate::models::{AppData, FriendLocation};
use crate::tilepacks::{cached_tile_path, latlon_to_world_px, world_px_to_latlon, TILE_SIZE};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, Ui, Vec2};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Canonical OpenStreetMap copyright / ODbL attribution URL.
pub const OSM_COPYRIGHT_URL: &str = "https://www.openstreetmap.org/copyright";

/// Default Moldova-first map center (Chișinău).
pub const DEFAULT_CENTER_LAT: f64 = 47.0105;
pub const DEFAULT_CENTER_LON: f64 = 28.8638;
pub const DEFAULT_ZOOM: u8 = 13;

/// Ensure `state` has a valid Slippy viewport (`z` in `2..=18`, center initialized).
pub fn ensure_viewport(state: &mut AppData) {
    if state.map_zoom == 0 {
        state.map_zoom = DEFAULT_ZOOM;
        if let Some(first) = state.locations.first() {
            state.map_center_lat = first.latitude;
            state.map_center_lon = first.longitude;
        } else {
            state.map_center_lat = DEFAULT_CENTER_LAT;
            state.map_center_lon = DEFAULT_CENTER_LON;
        }
    }
    state.map_zoom = state.map_zoom.clamp(2, 18);
    state.map_center_lat = state.map_center_lat.clamp(-85.0, 85.0);
    state.map_center_lon = state.map_center_lon.clamp(-180.0, 180.0);
}

/// Project `(lat, lon)` onto `map_rect` given `(center_lat, center_lon, zoom)`.
pub fn project_to_rect(
    lat: f64,
    lon: f64,
    center_lat: f64,
    center_lon: f64,
    zoom: u8,
    map_rect: Rect,
) -> Pos2 {
    let (cx, cy) = latlon_to_world_px(center_lat, center_lon, zoom);
    let (px, py) = latlon_to_world_px(lat, lon, zoom);
    let center = map_rect.center();
    Pos2::new(center.x + (px - cx) as f32, center.y + (py - cy) as f32)
}

/// Convert a screen position inside the map canvas back to `(lat, lon)`.
/// Used for tap-to-inspect and tap-to-set-destination.
pub fn screen_to_latlon(
    map_rect: Rect,
    center_lat: f64,
    center_lon: f64,
    zoom: u8,
    pos: Pos2,
) -> (f64, f64) {
    let (cx, cy) = latlon_to_world_px(center_lat, center_lon, zoom);
    let wx = cx + (pos.x - map_rect.center().x) as f64;
    let wy = cy + (pos.y - map_rect.center().y) as f64;
    world_px_to_latlon(wx, wy, zoom)
}

/// Try to load or retrieve a cached tile texture from disk (`tiles/{z}/{x}/{y}.png`).
fn load_tile_texture(
    ctx: &egui::Context,
    z: u8,
    x: u32,
    y: u32,
    dark: bool,
) -> Option<TextureHandle> {
    let id = egui::Id::new(("pinpoint_tile_tex", z, x, y, dark));
    if let Some(handle) = ctx.data_mut(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(handle);
    }
    let path = cached_tile_path(z, x, y)?;
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    // Dark mode matches the web's `.tiles-dark`: invert, rotate hue by 180°
    // (restoring road/water/park hues), dim and soften contrast. Light mode
    // keeps the clean OSM raster (Organic/Magic-Earth-like).
    let img = if dark {
        let mut img = img;
        for pixel in img.pixels_mut() {
            let [r, g, b, a] = pixel.0;
            pixel.0 = [255 - r, 255 - g, 255 - b, a];
        }
        let img = image::imageops::huerotate(&img, 180);
        let img = image::imageops::brighten(&img, -20);
        image::imageops::contrast(&img, -5.0)
    } else {
        img
    };
    let size = [img.width() as usize, img.height() as usize];
    let color_img = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    let handle = ctx.load_texture(
        format!("tile_{z}_{x}_{y}_{}", if dark { "dark" } else { "light" }),
        color_img,
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    Some(handle)
}

/// Fetch one missing tile through the Pinpoint backend, then repaint.
/// Deduplicates in-flight requests across frames.
fn prefetch_tile(ctx: &egui::Context, backend_url: &str, z: u8, x: u32, y: u32) {
    static PENDING: OnceLock<Mutex<HashSet<(u8, u32, u32)>>> = OnceLock::new();
    let pending = PENDING.get_or_init(|| Mutex::new(HashSet::new()));
    {
        let mut set = pending.lock().unwrap();
        if !set.insert((z, x, y)) {
            return;
        }
    }
    let base = backend_url.trim_end_matches('/').to_string();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        if let Ok(rt) = rt {
            let _ = rt.block_on(crate::tilepacks::download_pack(
                &[(z as u32, x, y)],
                None,
                Some(&base),
            ));
        }
        let pending = PENDING.get_or_init(|| Mutex::new(HashSet::new()));
        pending.lock().unwrap().remove(&(z, x, y));
        ctx.request_repaint();
    });
}

/// Render an interactive Slippy tile canvas showing friend locations and
/// cached tiles at the default 320 pt height.
pub fn render_map(ui: &mut Ui, state: &mut AppData) {
    render_map_sized(ui, state, 320.0);
}

/// Same as [`render_map`] but with an explicit canvas height (mobile layout)
/// and optional live tile prefetch through the Pinpoint backend.
pub fn render_map_sized(ui: &mut Ui, state: &mut AppData, height: f32) {
    render_map_with(ui, state, height, None, false, false);
}

pub fn render_map_with(
    ui: &mut Ui,
    state: &mut AppData,
    height: f32,
    backend_url: Option<&str>,
    online: bool,
    minimal: bool,
) {
    ensure_viewport(state);
    let compact = ui.available_width() < 520.0;

    // Collect all locations to display (friends + own location when available).
    let mut all_locations: Vec<FriendLocation> = state.locations.clone();
    if let Some(me) = &state.my_location {
        let mut own = me.clone();
        own.user_name = format!("Me ({})", own.user_name);
        all_locations.push(own);
    }

    let frame = if minimal {
        egui::Frame::NONE
    } else {
        egui::Frame::group(ui.style())
    };
    frame.show(ui, |ui| {
        if !minimal {
            ui.horizontal_wrapped(|ui| {
                if !compact {
                    ui.label(RichText::new("Interactive Slippy Map").size(16.0).strong());
                    ui.add_space(8.0);
                }
                if ui.small_button("−").clicked() && state.map_zoom > 2 {
                    state.map_zoom -= 1;
                }
                ui.label(
                    RichText::new(format!("z{}", state.map_zoom))
                        .monospace()
                        .size(12.0),
                );
                if ui.small_button("+").clicked() && state.map_zoom < 18 {
                    state.map_zoom += 1;
                }
                if !compact && ui.small_button("Chișinău").clicked() {
                    state.map_center_lat = DEFAULT_CENTER_LAT;
                    state.map_center_lon = DEFAULT_CENTER_LON;
                    state.map_zoom = DEFAULT_ZOOM;
                }
                if !all_locations.is_empty() && ui.small_button("Fit pins").clicked() {
                    let sum_lat: f64 = all_locations.iter().map(|l| l.latitude).sum();
                    let sum_lon: f64 = all_locations.iter().map(|l| l.longitude).sum();
                    let n = all_locations.len() as f64;
                    state.map_center_lat = sum_lat / n;
                    state.map_center_lon = sum_lon / n;
                }
                if !compact {
                    ui.label(
                        RichText::new(format!(
                            "center {:.4}, {:.4}",
                            state.map_center_lat, state.map_center_lon
                        ))
                        .size(11.0)
                        .color(Color32::from_rgb(148, 163, 184)),
                    );
                }
            });
            ui.add_space(6.0);
        }

        // Allocate interactive canvas
        let avail_width = if minimal {
            ui.available_width()
        } else {
            (ui.available_width() - 8.0).max(240.0)
        };
        let canvas_size = Vec2::new(avail_width, height.max(220.0));
        let (map_rect, response) = ui.allocate_exact_size(canvas_size, Sense::click_and_drag());

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                state.map_click = Some(screen_to_latlon(
                    map_rect,
                    state.map_center_lat,
                    state.map_center_lon,
                    state.map_zoom,
                    pos,
                ));
            }
        }

        // Drag-to-pan in Web Mercator pixel space
        if response.dragged() {
            let delta = response.drag_delta();
            let (cx, cy) =
                latlon_to_world_px(state.map_center_lat, state.map_center_lon, state.map_zoom);
            let (new_lat, new_lon) =
                world_px_to_latlon(cx - delta.x as f64, cy - delta.y as f64, state.map_zoom);
            state.map_center_lat = new_lat;
            state.map_center_lon = new_lon;
        }

        // Scroll wheel zoom when hovered
        if response.hovered() {
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll_y > 12.0 && state.map_zoom < 18 {
                state.map_zoom += 1;
            } else if scroll_y < -12.0 && state.map_zoom > 2 {
                state.map_zoom -= 1;
            }
        }

        let painter = ui.painter_at(map_rect);
        painter.rect_filled(map_rect, 4.0, Color32::from_rgb(15, 23, 42));

        // Draw visible 256×256 Slippy tiles
        let z = state.map_zoom;
        let label_size = state.map_label_size.unwrap_or(12.0).clamp(10.0, 18.0);
        let icon_size = state.map_icon_size.unwrap_or(26.0).clamp(18.0, 40.0);
        let pin_radius = icon_size * 0.27;
        let max_tile = (1u32 << z).saturating_sub(1);
        let (cx, cy) = latlon_to_world_px(state.map_center_lat, state.map_center_lon, z);
        let top_left_world_x = cx - (map_rect.width() as f64) / 2.0;
        let top_left_world_y = cy - (map_rect.height() as f64) / 2.0;
        let start_tx = (top_left_world_x / TILE_SIZE).floor() as i64;
        let start_ty = (top_left_world_y / TILE_SIZE).floor() as i64;
        let end_tx = ((top_left_world_x + map_rect.width() as f64) / TILE_SIZE).floor() as i64;
        let end_ty = ((top_left_world_y + map_rect.height() as f64) / TILE_SIZE).floor() as i64;

        for ty in start_ty..=end_ty {
            for tx in start_tx..=end_tx {
                if tx < 0 || ty < 0 || tx > max_tile as i64 || ty > max_tile as i64 {
                    continue;
                }
                let utx = tx as u32;
                let uty = ty as u32;
                let screen_x = map_rect.left() + (tx as f64 * TILE_SIZE - top_left_world_x) as f32;
                let screen_y = map_rect.top() + (ty as f64 * TILE_SIZE - top_left_world_y) as f32;
                let tile_rect = Rect::from_min_size(
                    Pos2::new(screen_x, screen_y),
                    Vec2::splat(TILE_SIZE as f32),
                );

                if let Some(tex) = load_tile_texture(ui.ctx(), z, utx, uty, state.map_dark) {
                    painter.image(
                        tex.id(),
                        tile_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    if online {
                        if let Some(base) = backend_url {
                            prefetch_tile(ui.ctx(), base, z, utx, uty);
                        }
                    }
                    painter.rect_stroke(
                        tile_rect,
                        0.0_f32,
                        Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59)),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        tile_rect.min + Vec2::new(6.0, 6.0),
                        egui::Align2::LEFT_TOP,
                        format!("{z}/{utx}/{uty}"),
                        egui::FontId::monospace(9.0),
                        Color32::from_rgb(71, 85, 105),
                    );
                }
            }
        }

        // Route line (A → stops → B), drawn under the pins.
        if let Some(route) = &state.last_route {
            let points: Vec<Pos2> = route
                .points
                .iter()
                .map(|(lat, lon)| {
                    project_to_rect(
                        *lat,
                        *lon,
                        state.map_center_lat,
                        state.map_center_lon,
                        z,
                        map_rect,
                    )
                })
                .collect();
            if points.len() > 1 {
                painter.add(egui::Shape::line(
                    points.clone(),
                    Stroke::new(8.0_f32, Color32::from_rgb(2, 6, 23)),
                ));
                painter.add(egui::Shape::line(
                    points,
                    Stroke::new(4.0_f32, Color32::from_rgb(52, 211, 153)),
                ));
            }
            for (index, (lat, lon)) in route.points.iter().enumerate() {
                let is_endpoint = index == 0 || index + 1 == route.points.len();
                if !is_endpoint {
                    continue;
                }
                let p = project_to_rect(
                    *lat,
                    *lon,
                    state.map_center_lat,
                    state.map_center_lon,
                    z,
                    map_rect,
                );
                if !map_rect.expand(20.0).contains(p) {
                    continue;
                }
                let label = if index == 0 { "A" } else { "B" };
                let color = if index == 0 {
                    Color32::from_rgb(52, 211, 153)
                } else {
                    Color32::from_rgb(248, 113, 113)
                };
                painter.circle_filled(p, 12.0, color);
                painter.circle_stroke(p, 12.0, Stroke::new(2.0_f32, Color32::BLACK));
                painter.text(
                    p,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(5, 46, 27),
                );
            }
        }

        // Selected search/reverse-geocode place (web `.marker.center`).
        if let Some(place) = &state.selected_place {
            let p = project_to_rect(
                place.latitude,
                place.longitude,
                state.map_center_lat,
                state.map_center_lon,
                z,
                map_rect,
            );
            if map_rect.expand(40.0).contains(p) {
                let accent = Color32::from_rgb(52, 211, 153);
                painter.circle_stroke(p, icon_size * 0.54, Stroke::new(2.5_f32, accent));
                painter.circle_filled(p, pin_radius * 0.7, accent);
                painter.text(
                    p + Vec2::new(0.0, -(icon_size * 0.7)),
                    egui::Align2::CENTER_BOTTOM,
                    &place.short_name,
                    egui::FontId::proportional(label_size),
                    accent,
                );
            }
        }

        // Draw pins for each location
        for loc in &all_locations {
            let pin_pos = project_to_rect(
                loc.latitude,
                loc.longitude,
                state.map_center_lat,
                state.map_center_lon,
                z,
                map_rect,
            );
            if !map_rect.expand(24.0).contains(pin_pos) {
                continue;
            }
            let (pin_color, ring_radius) = match loc.precision_level.as_str() {
                "exact" => (Color32::from_rgb(52, 211, 153), pin_radius + 5.0),
                "approximate" => (Color32::from_rgb(250, 204, 21), pin_radius + 15.0),
                "city_only" => (Color32::from_rgb(248, 113, 113), pin_radius + 27.0),
                _ => (Color32::from_rgb(148, 163, 184), pin_radius + 7.0),
            };
            painter.circle_stroke(
                pin_pos,
                ring_radius,
                Stroke::new(1.5_f32, pin_color.gamma_multiply(0.55)),
            );
            painter.circle_filled(pin_pos, pin_radius, pin_color);
            painter.circle_stroke(pin_pos, pin_radius, Stroke::new(2.0_f32, Color32::BLACK));

            painter.text(
                pin_pos + Vec2::new(0.0, -(icon_size * 0.46)),
                egui::Align2::CENTER_BOTTOM,
                &loc.user_name,
                egui::FontId::proportional(label_size),
                Color32::WHITE,
            );
        }

        painter.rect_stroke(
            map_rect,
            4.0_f32,
            Stroke::new(1.0_f32, Color32::from_rgb(51, 65, 85)),
            egui::StrokeKind::Inside,
        );

        ui.add_space(6.0);

        if !minimal {
            // Legend
            ui.horizontal_wrapped(|ui| {
                if !compact {
                    ui.label(RichText::new("Legend:").size(12.0).strong());
                }
                ui.label(
                    RichText::new("● Exact")
                        .size(12.0)
                        .color(Color32::from_rgb(52, 211, 153)),
                );
                ui.label(
                    RichText::new("● Approximate")
                        .size(12.0)
                        .color(Color32::from_rgb(250, 204, 21)),
                );
                ui.label(
                    RichText::new("● City-only")
                        .size(12.0)
                        .color(Color32::from_rgb(248, 113, 113)),
                );
                if !compact {
                    ui.label(
                        RichText::new("(drag map to pan, scroll/+/− to zoom)")
                            .size(11.0)
                            .color(Color32::from_rgb(148, 163, 184)),
                    );
                }
            });

            // Map provider buttons (OSM data — ODbL; see docs/sdlc/07-openstreetmap-integration.md)
            if let Some(first_loc) = all_locations.first() {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("OpenStreetMap").clicked() {
                        open_map_in_browser(ui.ctx(), first_loc, "osm");
                    }
                    if !compact && ui.button("Directions (OSM)").clicked() {
                        open_map_in_browser(ui.ctx(), first_loc, "directions");
                    }
                    if ui.button("Organic Maps").clicked() {
                        open_map_in_browser(ui.ctx(), first_loc, "organic");
                    }
                    if ui.button("Google Maps").clicked() {
                        open_map_in_browser(ui.ctx(), first_loc, "google");
                    }
                });
            }
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Map data © OpenStreetMap contributors (ODbL)")
                    .size(10.0)
                    .color(Color32::from_rgb(148, 163, 184)),
            );
            ui.hyperlink_to(
                RichText::new(OSM_COPYRIGHT_URL).size(10.0),
                OSM_COPYRIGHT_URL,
            );
        });
    });

    ui.add_space(10.0);

    if !minimal {
        if all_locations.is_empty() {
            ui.group(|ui| {
                ui.label(RichText::new("No friend locations shared yet").size(14.0));
            });
        } else if compact {
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| draw_location_list(ui, &all_locations));
        } else {
            draw_location_list(ui, &all_locations);
        }
    }
}

/// Open a map in the default browser for the given location and provider.
/// All URLs are OSM-data based (ODbL): OSM standard, OSM directions, and
/// Organic Maps (https://organicmaps.app). Reference:
/// https://wiki.openstreetmap.org/wiki/Main_Page — Magic Earth
/// (https://wiki.openstreetmap.org/wiki/Magic_Earth) is an OSM-data
/// consumer too; we link to OSM-native targets instead of proprietary
/// deep links so the desktop client stays OSM-only by default.
fn open_map_in_browser(ctx: &egui::Context, loc: &FriendLocation, provider: &str) {
    let url = match provider {
        "osm" => format!(
            "https://www.openstreetmap.org/#map=10/{:.4}/{:.4}",
            loc.latitude, loc.longitude
        ),
        "directions" => format!(
            "https://www.openstreetmap.org/directions?to={:.4}%2C{:.4}",
            loc.latitude, loc.longitude
        ),
        "organic" => {
            // Organic Maps web export format (OSM data, offline-first).
            format!(
                "https://organicmaps.app/?ll={:.4}%2C{:.4}&z=14",
                loc.latitude, loc.longitude
            )
        }
        "google" => format!(
            "https://www.google.com/maps/dir/?api=1&destination={:.4},{:.4}",
            loc.latitude, loc.longitude
        ),
        _ => return,
    };

    if let Ok(path) = which::which("xdg-open") {
        let _ = std::process::Command::new(path).arg(&url).spawn();
    } else if let Ok(path) = which::which("open") {
        let _ = std::process::Command::new(path).arg(&url).spawn();
    } else {
        ctx.open_url(egui::OpenUrl::new_tab(url));
    }
}

fn draw_location_list(ui: &mut Ui, locations: &[FriendLocation]) {
    ui.group(|ui| {
        ui.label(RichText::new(format!("Locations: {}", locations.len())).size(14.0));

        for loc in locations {
            let precision_color = match loc.precision_level.as_str() {
                "exact" => Color32::from_rgb(250, 204, 21),
                "approximate" => Color32::from_rgb(239, 68, 68),
                _ => Color32::from_rgb(148, 163, 184),
            };

            ui.horizontal(|ui| {
                ui.label(RichText::new(&loc.user_name).size(13.0));
                ui.add_space(50.0);
                ui.label(
                    RichText::new(format!("{:.4}, {:.4}", loc.latitude, loc.longitude)).size(12.0),
                );
                ui.add_space(20.0);
                ui.label(
                    RichText::new(&loc.precision_level)
                        .size(10.0)
                        .color(precision_color),
                );
            });
            // Distance from first demo point (proxy for "near me" when no GPS).
            let anchor = (47.0105, 28.8638);
            let d = crate::models::haversine_m(anchor.0, anchor.1, loc.latitude, loc.longitude);
            let b = crate::models::bearing_deg(anchor.0, anchor.1, loc.latitude, loc.longitude);
            // Freshness badge (honest: stale/dead never shown as live).
            let now = chrono::Utc::now().timestamp();
            let fresh = crate::models::freshness_state(&loc.updated_at, now);
            let fresh_color = match fresh {
                "live" => Color32::from_rgb(52, 211, 153),
                "fresh" => Color32::from_rgb(250, 204, 21),
                "stale" => Color32::from_rgb(249, 115, 22),
                "dead" => Color32::from_rgb(239, 68, 68),
                _ => Color32::from_rgb(148, 163, 184),
            };
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{} {} · updated {}",
                        crate::models::format_distance(d, true),
                        crate::models::compass_label(b),
                        loc.updated_at
                    ))
                    .size(10.0)
                    .color(Color32::from_rgb(148, 163, 184)),
                );
                ui.colored_label(fresh_color, RichText::new(format!("● {fresh}")).size(10.0));
                if ui.small_button("★ Save").clicked() {
                    let store = crate::bookmarks::BookmarkStore::default_location();
                    store.add(&loc.user_name, loc.latitude, loc.longitude);
                }
                if ui.small_button("Address").clicked() {
                    // Handled by Places screen reverse lookup; here just notice.
                    ui.ctx().request_repaint();
                }
            });
            ui.add_space(5.0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_viewport_initializes_defaults() {
        let mut data = AppData::default();
        ensure_viewport(&mut data);
        assert_eq!(data.map_zoom, DEFAULT_ZOOM);
        assert!((data.map_center_lat - DEFAULT_CENTER_LAT).abs() < 1e-6);
        assert!((data.map_center_lon - DEFAULT_CENTER_LON).abs() < 1e-6);
    }

    #[test]
    fn project_center_maps_to_rect_center() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(400.0, 300.0));
        let p = project_to_rect(47.0105, 28.8638, 47.0105, 28.8638, 13, rect);
        assert!((p.x - rect.center().x).abs() < 1e-3);
        assert!((p.y - rect.center().y).abs() < 1e-3);
    }

    #[test]
    fn copyright_url_is_odbl_compliant() {
        assert!(OSM_COPYRIGHT_URL.contains("openstreetmap.org/copyright"));
    }

    #[test]
    fn screen_to_latlon_round_trips_projected_points() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(400.0, 300.0));
        let center = (47.0105, 28.8638);
        let point = (47.0205, 28.8738);
        let p = project_to_rect(point.0, point.1, center.0, center.1, 13, rect);
        let back = screen_to_latlon(rect, center.0, center.1, 13, p);
        assert!((back.0 - point.0).abs() < 1e-6);
        assert!((back.1 - point.1).abs() < 1e-6);
    }
}
