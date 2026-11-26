//! Route planning state and the shared route sheet.
//!
//! The web client owns this flow in `web/app/nav.js`; this module is the
//! native equivalent. State lives in [`RoutePlan`] on [`super::PinpointAppUI`],
//! while the chosen route is copied to `AppData.last_route` for the map.

use super::theme;
use crate::models::AppData;
use crate::osm::{MapCapabilities, Route, SearchPlace};
use eframe::egui::{self, RichText};
use std::fmt::Write as _;

/// A route endpoint or stop. `name` is display-only; coordinates are the
/// source of truth sent to the backend.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutePoint {
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}

impl RoutePoint {
    pub fn new(lat: f64, lon: f64, name: impl Into<String>) -> Self {
        Self {
            lat,
            lon,
            name: name.into(),
        }
    }

    pub fn from_place(place: &SearchPlace) -> Self {
        Self::new(place.latitude, place.longitude, place.short_name.clone())
    }

    pub fn label(&self) -> String {
        if self.name.trim().is_empty() {
            format!("{:.5}, {:.5}", self.lat, self.lon)
        } else {
            format!("{} ({:.4}, {:.4})", self.name, self.lat, self.lon)
        }
    }
}

/// Everything the route sheet needs. Kept UI-side: no network credentials or
/// friend locations are stored here beyond the explicit route endpoints.
#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub origin: Option<RoutePoint>,
    pub destination: Option<RoutePoint>,
    pub stops: Vec<RoutePoint>,
    pub avoid: Vec<String>,
    pub mode: String,
    pub routes: Vec<Route>,
    pub selected: usize,
    pub open: bool,
    pub following: bool,
    pub next_stop: usize,
    /// Index of the next maneuver in the selected route's `steps`.
    pub next_step: usize,
    /// Metres from the last fix to the next maneuver, when that maneuver has a location.
    pub nav_distance_m: Option<f64>,
    pub capabilities: Option<MapCapabilities>,
    pub error: Option<String>,
    /// Ask the route sheet's close control for focus once after opening.
    pub focus: bool,
}

impl Default for RoutePlan {
    fn default() -> Self {
        Self {
            origin: None,
            destination: None,
            stops: Vec::new(),
            avoid: Vec::new(),
            mode: "driving".to_string(),
            routes: Vec::new(),
            selected: 0,
            open: false,
            following: false,
            next_stop: 0,
            next_step: 0,
            nav_distance_m: None,
            capabilities: None,
            error: None,
            focus: false,
        }
    }
}

impl RoutePlan {
    /// Open the sheet and ask its close control for focus on the next frame.
    pub fn show(&mut self) {
        self.open = true;
        self.focus = true;
    }

    pub fn open_with_destination(&mut self, place: &SearchPlace) {
        self.destination = Some(RoutePoint::from_place(place));
        self.show();
        self.error = None;
        self.following = false;
        self.next_step = 0;
        self.nav_distance_m = None;
    }

    pub fn open_with_origin(&mut self, place: &SearchPlace) {
        self.origin = Some(RoutePoint::from_place(place));
        self.show();
        self.error = None;
        self.following = false;
        self.next_step = 0;
        self.nav_distance_m = None;
    }

    pub fn swap(&mut self) {
        std::mem::swap(&mut self.origin, &mut self.destination);
        self.routes.clear();
        self.selected = 0;
        self.following = false;
        self.next_step = 0;
        self.nav_distance_m = None;
    }

    pub fn clear_route(&mut self) {
        self.routes.clear();
        self.selected = 0;
        self.following = false;
        self.next_stop = 0;
        self.next_step = 0;
        self.nav_distance_m = None;
        self.error = None;
    }

    pub fn has_endpoints(&self) -> bool {
        self.origin.is_some() && self.destination.is_some()
    }

    pub fn via(&self) -> Vec<(f64, f64)> {
        self.stops.iter().map(|p| (p.lat, p.lon)).collect()
    }

    pub fn avoid_csv(&self) -> Vec<String> {
        self.avoid.clone()
    }

    pub fn add_stop(&mut self, point: RoutePoint) {
        if self.stops.len() < 6 {
            self.stops.push(point);
            self.routes.clear();
            self.selected = 0;
        }
    }

    pub fn remove_stop(&mut self, index: usize) {
        if index < self.stops.len() {
            self.stops.remove(index);
            self.routes.clear();
            self.selected = 0;
        }
    }

    pub fn apply_selected(&self, data: &mut AppData) {
        if let Some(route) = self.routes.get(self.selected) {
            data.last_route = Some(route.clone());
        }
    }

    pub fn summary(&self) -> Option<String> {
        let route = self.routes.get(self.selected)?;
        Some(format!(
            "{} · {}",
            format_distance(route.distance_m),
            format_duration(route.duration_s)
        ))
    }

    pub fn selected_route(&self) -> Option<&Route> {
        self.routes.get(self.selected)
    }

    /// Human-readable next-step line for the following banner.
    pub(crate) fn navigation_status(&self) -> Option<String> {
        if !self.following {
            return None;
        }
        let route = self.selected_route()?;
        let step = route.steps.get(self.next_step);
        let target = if self.next_stop < self.stops.len() {
            format!("stop {}", self.next_stop + 1)
        } else {
            "destination".to_string()
        };
        Some(match step {
            Some(step) => {
                let distance = self
                    .nav_distance_m
                    .map(|metres| format!(", {}", format_distance(metres)))
                    .unwrap_or_default();
                format!("Next: {} ({target}{distance})", step.text)
            }
            None => format!("Heading to {target}"),
        })
    }

    pub fn mode_label(&self) -> &'static str {
        match self.mode.as_str() {
            "foot" => "Walk",
            "bike" => "Bike",
            _ => "Car",
        }
    }

    pub fn can_use_mode(&self, mode: &str) -> bool {
        self.capabilities
            .as_ref()
            .map(|caps| caps.modes.iter().any(|m| m == mode))
            .unwrap_or(true)
    }

    pub fn can_avoid(&self) -> bool {
        self.capabilities
            .as_ref()
            .map(|caps| caps.avoid)
            .unwrap_or(true)
    }

    pub fn share_text(&self) -> String {
        let Some(route) = self.selected_route() else {
            return "Pinpoint route".to_string();
        };
        let mut out = String::new();
        let _ = writeln!(out, "Pinpoint route ({})", self.mode_label());
        if let Some(origin) = &self.origin {
            let _ = writeln!(out, "A: {}", origin.label());
        }
        for (i, stop) in self.stops.iter().enumerate() {
            let _ = writeln!(out, "{}: {}", i + 1, stop.label());
        }
        if let Some(destination) = &self.destination {
            let _ = writeln!(out, "B: {}", destination.label());
        }
        let _ = writeln!(
            out,
            "{} · {} · {}",
            format_distance(route.distance_m),
            format_duration(route.duration_s),
            if route.summary.is_empty() {
                "Route"
            } else {
                &route.summary
            }
        );
        for step in &route.steps {
            let distance = step
                .distance_m
                .map(format_distance)
                .map(|d| format!(" ({d})"))
                .unwrap_or_default();
            let _ = writeln!(out, "- {}{}", step.text, distance);
        }
        out
    }
}

pub fn format_distance(meters: f64) -> String {
    if meters < 1000.0 {
        format!("{} m", meters.round() as i64)
    } else {
        format!("{:.1} km", meters / 1000.0)
    }
}

/// Great-circle distance in metres.
pub fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6_371_000.0;
    let p1 = lat1.to_radians();
    let p2 = lat2.to_radians();
    let dp = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * a.sqrt().atan2((1.0 - a).sqrt())
}

pub fn format_duration(seconds: f64) -> String {
    let minutes = (seconds / 60.0).round() as i64;
    if minutes < 60 {
        format!("{minutes} min")
    } else {
        format!("{} h {} min", minutes / 60, minutes % 60)
    }
}

/// Google Maps directions URL for the current plan.
pub fn google_route_url(plan: &RoutePlan) -> Option<String> {
    let origin = plan.origin.as_ref()?;
    let destination = plan.destination.as_ref()?;
    let mut url = format!(
        "https://www.google.com/maps/dir/?api=1&origin={:.6},{:.6}&destination={:.6},{:.6}&travelmode={}",
        origin.lat,
        origin.lon,
        destination.lat,
        destination.lon,
        match plan.mode.as_str() {
            "foot" => "walking",
            "bike" => "bicycling",
            _ => "driving",
        }
    );
    if !plan.stops.is_empty() {
        let waypoints = plan
            .stops
            .iter()
            .map(|p| format!("{:.6},{:.6}", p.lat, p.lon))
            .collect::<Vec<_>>()
            .join("|");
        url.push_str("&waypoints=");
        url.push_str(&waypoints);
    }
    Some(url)
}

/// Organic Maps directions URL for the current plan.
pub fn organic_route_url(plan: &RoutePlan) -> Option<String> {
    let origin = plan.origin.as_ref()?;
    let destination = plan.destination.as_ref()?;
    Some(format!(
        "https://omaps.app/dir?from={:.6},{:.6}&to={:.6},{:.6}&mode={}",
        origin.lat, origin.lon, destination.lat, destination.lon, plan.mode
    ))
}

/// Magic Earth directions URL for the current plan.
pub fn magic_route_url(plan: &RoutePlan) -> Option<String> {
    let origin = plan.origin.as_ref()?;
    let destination = plan.destination.as_ref()?;
    Some(format!(
        "magicearth://?route&from={:.6},{:.6}&to={:.6},{:.6}&mode={}",
        origin.lat, origin.lon, destination.lat, destination.lon, plan.mode
    ))
}

/// Consume the route sheet's one-shot focus request.
pub(crate) fn take_route_focus(plan: &mut RoutePlan) -> bool {
    std::mem::take(&mut plan.focus)
}

/// The first instruction is usually the departure point; show the next turn.
fn initial_navigation_step(route: &Route) -> usize {
    if route.steps.len() > 1 {
        1
    } else {
        0
    }
}

/// Advance through maneuvers the user has reached, so the banner follows turns.
fn next_navigation_step(current: usize, route: &Route, lat: f64, lon: f64) -> usize {
    let mut step = current.min(route.steps.len());
    while let Some(candidate) = route.steps.get(step) {
        let Some((step_lat, step_lon)) = candidate.location else {
            break;
        };
        if haversine_m(lat, lon, step_lat, step_lon) > 40.0 {
            break;
        }
        step += 1;
    }
    step.min(route.steps.len())
}

/// Metres from the fix to the maneuver at `step`, or `None` when the maneuver
/// is finished or has no location.
fn next_navigation_distance(route: &Route, step: usize, lat: f64, lon: f64) -> Option<f64> {
    let (step_lat, step_lon) = route.steps.get(step)?.location?;
    Some(haversine_m(lat, lon, step_lat, step_lon))
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RouteSheetOutcome {
    pub close: bool,
    pub clear: bool,
    pub plan: bool,
    pub navigate: bool,
    pub copy_share: bool,
    pub copy_kml: bool,
    pub copy_geojson: bool,
    pub external: Option<String>,
    pub add_stop: bool,
    pub remove_stop: Option<usize>,
    pub close_id: Option<egui::Id>,
    pub plan_id: Option<egui::Id>,
    pub navigate_id: Option<egui::Id>,
    pub share_id: Option<egui::Id>,
    pub kml_id: Option<egui::Id>,
    pub geojson_id: Option<egui::Id>,
    pub clear_id: Option<egui::Id>,
}

#[derive(Clone, Copy)]
pub(crate) struct RouteSheetData<'a> {
    pub map_center: (f64, f64),
    pub my_location: Option<(f64, f64)>,
    pub selected_place: Option<&'a SearchPlace>,
    pub navigation_status: Option<&'a str>,
}

impl super::PinpointAppUI {
    /// Poll the platform fix once per second while navigation is active and
    /// advance the next-stop counter when close enough. The env/file provider
    /// is the current Android/desktop fallback; a real LocationManager bridge
    /// can replace it without touching this loop.
    pub(crate) fn tick_navigation(&mut self) {
        if !self.route_plan.following {
            self.route_plan.nav_distance_m = None;
            return;
        }
        let now = std::time::Instant::now();
        if self
            .route_nav_last_fix
            .map(|last| now.duration_since(last).as_secs_f64() < 1.0)
            .unwrap_or(false)
        {
            return;
        }
        self.route_nav_last_fix = Some(now);
        let Some((lat, lon, accuracy)) = self.app.locate() else {
            if !self.route_nav_gps_warned {
                self.route_nav_gps_warned = true;
                self.app.set_error(Some(
                    "Navigation is waiting for a GPS fix (no fix available).".to_string(),
                ));
            }
            return;
        };
        self.route_nav_gps_warned = false;
        let data = self.app.data();
        let mut d = data.lock().unwrap();
        let (user_id, user_name) = d
            .user
            .as_ref()
            .map(|u| (u.id, u.full_name.clone()))
            .unwrap_or((0, "Me".to_string()));
        d.my_location = Some(crate::models::FriendLocation {
            user_id,
            user_name,
            latitude: lat,
            longitude: lon,
            precision_level: "exact".to_string(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            accuracy: Some(accuracy),
        });
        d.map_center_lat = lat;
        d.map_center_lon = lon;
        d.map_zoom = d.map_zoom.max(15);
        drop(d);

        let next = self
            .route_plan
            .selected_route()
            .map(|route| next_navigation_step(self.route_plan.next_step, route, lat, lon));
        if let Some(next) = next {
            self.route_plan.next_step = next;
        }
        self.route_plan.nav_distance_m = self
            .route_plan
            .selected_route()
            .and_then(|route| next_navigation_distance(route, self.route_plan.next_step, lat, lon));

        if self.route_plan.next_stop < self.route_plan.stops.len() {
            if let Some(stop) = self.route_plan.stops.get(self.route_plan.next_stop) {
                if haversine_m(lat, lon, stop.lat, stop.lon) < 60.0 {
                    self.route_plan.next_stop += 1;
                }
            }
        }
    }

    /// Consume one map tap: set B when the route sheet is open, otherwise
    /// reverse-geocode it into the place sheet.
    pub(crate) fn handle_map_click(&mut self, ctx: &egui::Context) {
        let bookmark = { self.app.data().lock().unwrap().map_bookmark_click.take() };
        if let Some((name, latitude, longitude)) = bookmark {
            let place = crate::osm::SearchPlace {
                display_name: name.clone(),
                short_name: name,
                latitude,
                longitude,
                category: "saved".to_string(),
                place_type: "bookmark".to_string(),
                distance_m: None,
            };
            self.app.data().lock().unwrap().selected_place = Some(place.clone());
            self.mobile_selected_place = Some(place);
            self.place_focus = true;
            self.center_map_at(latitude, longitude);
            tracing::info!("bookmark tapped");
            return;
        }
        let click = { self.app.data().lock().unwrap().map_click.take() };
        if let Some((lat, lon)) = click {
            if self.route_plan.open {
                self.route_plan.destination = Some(RoutePoint::new(lat, lon, "Map tap"));
                self.route_plan.error = None;
            } else {
                self.spawn_action(super::PendingAction::Reverse { lat, lon }, ctx);
            }
        }
    }

    /// Copy freshly returned backend alternatives into the route sheet.
    pub(crate) fn sync_route_results(&mut self) {
        let (seq, routes, error) = {
            let data = self.app.data();
            let d = data.lock().unwrap();
            (
                d.route_result_seq,
                d.route_results.clone(),
                d.route_error.clone(),
            )
        };
        if seq == self.route_result_seq {
            return;
        }
        self.route_result_seq = seq;
        self.route_plan.routes = routes;
        self.route_plan.selected = 0;
        self.route_plan.following = false;
        self.route_plan.next_step = self
            .route_plan
            .selected_route()
            .map(initial_navigation_step)
            .unwrap_or(0);
        self.route_plan.nav_distance_m = None;
        self.route_plan.error = error;
        let app_data = self.app.data();
        let mut data = app_data.lock().unwrap();
        self.route_plan.apply_selected(&mut data);
        self.route_plan.show();
    }

    /// Draw the route planner as a floating sheet. `top` is the map's top
    /// edge; desktop uses a left panel, compact uses a bottom sheet.
    pub(crate) fn draw_route_sheet(
        &mut self,
        _ui: &mut egui::Ui,
        ctx: &egui::Context,
        compact: bool,
        top: f32,
    ) {
        if !self.route_plan.open {
            return;
        }
        let blocked = self.crash_dialog.is_some()
            || self.update_dialog_open
            || self.share_sheet.is_some()
            || self.mobile_publish_open
            || self.mobile_selected_place.is_some()
            || self.recovery_code_visible();
        if super::escape_dismisses(ctx.input(|i| i.key_pressed(egui::Key::Escape)), blocked) {
            self.route_plan.open = false;
            self.route_plan.focus = false;
            return;
        }
        let (map_center, my_location) = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            (
                (data.map_center_lat, data.map_center_lon),
                data.my_location
                    .as_ref()
                    .map(|loc| (loc.latitude, loc.longitude)),
            )
        };
        let selected_place = self.selected_place();
        let navigation_status = self.route_plan.navigation_status();
        let mut plan = self.route_plan.clone();
        let outcome = Self::draw_route_sheet_view(
            ctx,
            &mut plan,
            compact,
            top,
            RouteSheetData {
                map_center,
                my_location,
                selected_place: selected_place.as_ref(),
                navigation_status: navigation_status.as_deref(),
            },
        );
        if outcome.close {
            plan.open = false;
            plan.focus = false;
            plan.following = false;
        }
        if outcome.clear {
            plan.clear_route();
            self.app.data().lock().unwrap().last_route = None;
        }
        if outcome.remove_stop.is_some() || outcome.add_stop {
            if let Some(index) = outcome.remove_stop {
                plan.remove_stop(index);
            }
            if outcome.add_stop {
                if let Some(place) = self.app.data().lock().unwrap().selected_place.clone() {
                    plan.add_stop(RoutePoint::from_place(&place));
                } else {
                    plan.error = Some("Select a place first to add it as a stop.".into());
                }
            }
            plan.error = None;
        }
        if outcome.plan {
            self.request_route(&plan, ctx);
        }
        if outcome.navigate {
            plan.following = true;
            plan.next_stop = 0;
            self.app.set_notice(Some(
                "Route preview started. Live GPS follows when a fix is available.".to_string(),
            ));
        }
        if outcome.copy_share {
            ctx.copy_text(plan.share_text());
            self.app
                .set_notice(Some("Route summary copied.".to_string()));
        }
        if outcome.copy_kml || outcome.copy_geojson {
            if let Some(route) = plan.selected_route() {
                let text = if outcome.copy_kml {
                    crate::osm::route_to_kml(route, "Pinpoint route")
                } else {
                    crate::osm::route_to_geojson(route, "Pinpoint route")
                };
                ctx.copy_text(text);
                self.app.set_notice(Some(if outcome.copy_kml {
                    "KML copied to clipboard.".to_string()
                } else {
                    "GeoJSON copied to clipboard.".to_string()
                }));
            }
        }
        if let Some(url) = outcome.external {
            let _ = webbrowser::open(&url);
        }
        self.route_plan = plan;
        let app_data = self.app.data();
        let mut data = app_data.lock().unwrap();
        self.route_plan.apply_selected(&mut data);
    }

    pub(crate) fn draw_route_sheet_view(
        ctx: &egui::Context,
        plan: &mut RoutePlan,
        compact: bool,
        top: f32,
        data: RouteSheetData<'_>,
    ) -> RouteSheetOutcome {
        let RouteSheetData {
            map_center,
            my_location,
            selected_place,
            navigation_status,
        } = data;
        let mut close_id = None;
        let width = ctx.screen_rect().width();
        let panel = theme::PANEL.gamma_multiply(0.98);
        let border = theme::BORDER;
        let muted = theme::MUTED;
        let accent = theme::ACCENT;
        let focus_close = take_route_focus(plan);
        let mut plan_requested = false;
        let mut close = false;
        let mut clear = false;
        let mut navigate = false;
        let mut copy_share = false;
        let mut copy_kml = false;
        let mut copy_geojson = false;
        let mut external: Option<String> = None;
        let mut add_stop = false;
        let mut remove_stop: Option<usize> = None;
        let mut plan_id = None;
        let mut navigate_id = None;
        let mut share_id = None;
        let mut kml_id = None;
        let mut geojson_id = None;
        let mut clear_id = None;

        let area = if compact {
            egui::Area::new(egui::Id::new("route_sheet"))
                .order(egui::Order::Foreground)
                .anchor(
                    egui::Align2::CENTER_BOTTOM,
                    egui::vec2(0.0, if compact { -66.0 } else { -8.0 }),
                )
        } else {
            egui::Area::new(egui::Id::new("route_sheet"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(12.0, top + 12.0))
        };
        area.show(ctx, |ui| {
            ui.set_width(if compact { width - 24.0 } else { 360.0 });
            egui::Frame::new()
                .fill(panel)
                .stroke(egui::Stroke::new(1.0_f32, border))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Route").size(18.0).strong());
                        ui.label(RichText::new(plan.mode_label()).size(12.0).color(accent));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let close_button = ui.button("×");
                            if close_button.clicked() {
                                close = true;
                            }
                            close_id = Some(close_button.id);
                            ui.memory_mut(|memory| {
                                memory.interested_in_focus(close_button.id, close_button.layer_id);
                            });
                            if focus_close {
                                close_button.request_focus();
                            }
                        });
                    });
                    let screenshot_scroll = if std::env::var("PINPOINT_SCREENSHOT_ROUTE").is_ok() {
                        330.0
                    } else {
                        0.0
                    };
                    egui::ScrollArea::vertical()
                        .vertical_scroll_offset(screenshot_scroll)
                        .max_height(if compact {
                            (ctx.screen_rect().height() - top - 130.0).max(240.0)
                        } else {
                            (ctx.screen_rect().height() - top - 40.0).max(240.0)
                        })
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            let origin_label = plan
                                .origin
                                .as_ref()
                                .map(RoutePoint::label)
                                .unwrap_or_else(|| "Set origin (map centre or search)".into());
                            ui.label(RichText::new("A").strong().color(accent));
                            ui.label(RichText::new(origin_label).size(12.0));
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Use map centre").clicked() {
                                    plan.origin = Some(RoutePoint::new(
                                        map_center.0,
                                        map_center.1,
                                        "Map centre",
                                    ));
                                }
                                if ui.button("Use my location").clicked() {
                                    let (lat, lon) = my_location.unwrap_or((47.0105, 28.8638));
                                    plan.origin = Some(RoutePoint::new(lat, lon, "My location"));
                                }
                                if ui.button("Swap").clicked() {
                                    plan.swap();
                                }
                            });
                            ui.separator();

                            let destination_label = plan
                                .destination
                                .as_ref()
                                .map(RoutePoint::label)
                                .unwrap_or_else(|| "Set destination (search or map tap)".into());
                            ui.label(RichText::new("B").strong().color(accent));
                            ui.label(RichText::new(destination_label).size(12.0));
                            if let Some(place) = selected_place {
                                if ui.button("Use selected place as B").clicked() {
                                    plan.destination = Some(RoutePoint::from_place(place));
                                }
                            }
                            ui.separator();

                            ui.label(RichText::new("Stops").strong());
                            for (index, stop) in plan.stops.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{}. {}", index + 1, stop.label()))
                                            .size(12.0),
                                    );
                                    if ui.button("×").clicked() {
                                        remove_stop = Some(index);
                                    }
                                });
                            }
                            if ui.button("+ Add selected place as stop").clicked() {
                                add_stop = true;
                            }
                            ui.separator();

                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("Avoid").strong());
                                for value in ["motorway", "toll", "ferry"] {
                                    let mut checked = plan.avoid.iter().any(|v| v == value);
                                    let response = ui.add_enabled(
                                        plan.can_avoid(),
                                        egui::Checkbox::new(&mut checked, value),
                                    );
                                    if !plan.can_avoid() {
                                        response.on_hover_text(
                                            "Avoid options need a self-hosted OSRM profile.",
                                        );
                                    }
                                    if checked && !plan.avoid.iter().any(|v| v == value) {
                                        plan.avoid.push(value.to_string());
                                    }
                                    if !checked {
                                        plan.avoid.retain(|v| v != value);
                                    }
                                }
                            });
                            ui.horizontal_wrapped(|ui| {
                                for (mode, label) in
                                    [("driving", "Car"), ("foot", "Walk"), ("bike", "Bike")]
                                {
                                    let enabled = plan.can_use_mode(mode);
                                    if ui
                                        .add_enabled(
                                            enabled,
                                            egui::Button::selectable(plan.mode == mode, label),
                                        )
                                        .clicked()
                                    {
                                        plan.mode = mode.to_string();
                                    }
                                }
                                let plan_button = ui.button("Route");
                                if plan_button.clicked() {
                                    plan_requested = true;
                                }
                                plan_id = Some(plan_button.id);
                            });

                            if let Some(error) = &plan.error {
                                ui.colored_label(theme::DANGER, error);
                            }
                            if !plan.routes.is_empty() {
                                ui.separator();
                                ui.horizontal_wrapped(|ui| {
                                    for (index, route) in plan.routes.iter().enumerate() {
                                        let label = format!(
                                            "{} · {}",
                                            format_distance(route.distance_m),
                                            format_duration(route.duration_s)
                                        );
                                        if ui
                                            .selectable_label(plan.selected == index, label)
                                            .clicked()
                                        {
                                            plan.selected = index;
                                            plan.next_step = plan
                                                .routes
                                                .get(index)
                                                .map(initial_navigation_step)
                                                .unwrap_or(0);
                                            plan.nav_distance_m = None;
                                        }
                                    }
                                });
                                if let Some(route) = plan.routes.get(plan.selected) {
                                    if !route.summary.is_empty() {
                                        ui.label(
                                            RichText::new(&route.summary).size(12.0).color(muted),
                                        );
                                    }
                                    if let Some(summary) = plan.summary() {
                                        ui.label(RichText::new(summary).size(14.0).strong());
                                    }
                                    if let Some(status) = navigation_status {
                                        ui.colored_label(accent, RichText::new(status).size(12.0));
                                    }
                                    for step in &route.steps {
                                        let distance = step
                                            .distance_m
                                            .map(format_distance)
                                            .map(|d| format!(" ({d})"))
                                            .unwrap_or_default();
                                        ui.label(
                                            RichText::new(format!("• {}{}", step.text, distance))
                                                .size(11.0),
                                        );
                                    }
                                }
                                ui.horizontal_wrapped(|ui| {
                                    if plan.following {
                                        if ui.button("Exit navigation").clicked() {
                                            plan.following = false;
                                        }
                                    } else {
                                        let navigate_button = ui.button("Navigate");
                                        if navigate_button.clicked() {
                                            navigate = true;
                                        }
                                        navigate_id = Some(navigate_button.id);
                                    }
                                    let share_button = ui.button("Share");
                                    if share_button.clicked() {
                                        copy_share = true;
                                    }
                                    share_id = Some(share_button.id);
                                    if ui.button("GPX").clicked() {
                                        copy_share = true;
                                    }
                                    let kml_button = ui.button("KML");
                                    if kml_button.clicked() {
                                        copy_kml = true;
                                    }
                                    kml_id = Some(kml_button.id);
                                    let geojson_button = ui.button("GeoJSON");
                                    if geojson_button.clicked() {
                                        copy_geojson = true;
                                    }
                                    geojson_id = Some(geojson_button.id);
                                    let clear_button = ui.button("Clear");
                                    if clear_button.clicked() {
                                        clear = true;
                                    }
                                    clear_id = Some(clear_button.id);
                                });
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("Google Maps").clicked() {
                                        external = google_route_url(plan);
                                    }
                                    if ui.button("Organic Maps").clicked() {
                                        external = organic_route_url(plan);
                                    }
                                    if ui.button("Magic Earth").clicked() {
                                        external = magic_route_url(plan);
                                    }
                                });
                            }
                            ui.separator();
                            ui.label(
                                RichText::new("© OpenStreetMap contributors")
                                    .size(10.0)
                                    .color(muted),
                            );
                        });
                });
        });

        RouteSheetOutcome {
            close,
            clear,
            plan: plan_requested,
            navigate,
            copy_share,
            copy_kml,
            copy_geojson,
            external,
            add_stop,
            remove_stop,
            close_id,
            plan_id,
            navigate_id,
            share_id,
            kml_id,
            geojson_id,
            clear_id,
        }
    }
    /// Spawn a backend route request for the current plan.
    pub(crate) fn request_route(&mut self, plan: &RoutePlan, ctx: &egui::Context) {
        if !plan.has_endpoints() {
            self.route_plan.error = Some("Set both A and B before routing.".into());
            return;
        }
        let (Some(origin), Some(destination)) = (&plan.origin, &plan.destination) else {
            self.route_plan.error = Some("Set both A and B before routing.".into());
            return;
        };
        self.spawn_action(
            super::PendingAction::PlanRoute {
                from: (origin.lat, origin.lon),
                to: (destination.lat, destination.lon),
                mode: plan.mode.clone(),
                via: plan.via(),
                avoid: plan.avoid_csv(),
            },
            ctx,
        );
    }

    pub(crate) fn selected_place(&self) -> Option<SearchPlace> {
        self.app.data().lock().unwrap().selected_place.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(distance: f64) -> Route {
        Route {
            points: vec![(47.0, 28.8), (47.1, 28.9)],
            distance_m: distance,
            duration_s: 600.0,
            steps: vec![],
            summary: "Main".into(),
            legs: vec![],
        }
    }

    #[test]
    fn swap_exchanges_endpoints_and_clears_routes() {
        let mut plan = RoutePlan {
            origin: Some(RoutePoint::new(1.0, 2.0, "A")),
            destination: Some(RoutePoint::new(3.0, 4.0, "B")),
            routes: vec![route(1000.0)],
            ..Default::default()
        };
        plan.swap();
        assert_eq!(plan.origin.as_ref().unwrap().name, "B");
        assert_eq!(plan.destination.as_ref().unwrap().name, "A");
        assert!(plan.routes.is_empty());
    }

    #[test]
    fn apply_selected_copies_route_to_app_data() {
        let mut plan = RoutePlan::default();
        plan.routes.push(route(1200.0));
        let mut data = AppData::default();
        plan.apply_selected(&mut data);
        assert_eq!(data.last_route.unwrap().distance_m, 1200.0);
    }

    #[test]
    fn summary_formats_distance_and_duration() {
        let mut plan = RoutePlan::default();
        plan.routes.push(route(1200.0));
        assert_eq!(plan.summary().unwrap(), "1.2 km · 10 min");
    }

    #[test]
    fn clear_removes_routes_but_keeps_endpoints() {
        let mut plan = RoutePlan {
            origin: Some(RoutePoint::new(1.0, 2.0, "A")),
            destination: Some(RoutePoint::new(3.0, 4.0, "B")),
            routes: vec![route(1000.0)],
            ..Default::default()
        };
        plan.clear_route();
        assert!(plan.routes.is_empty());
        assert!(plan.has_endpoints());
    }

    fn place(name: &str) -> SearchPlace {
        SearchPlace {
            display_name: name.into(),
            short_name: name.into(),
            latitude: 47.0,
            longitude: 28.8,
            category: "test".into(),
            place_type: "test".into(),
            distance_m: None,
        }
    }

    #[test]
    fn a_new_route_sheet_requests_focus_once() {
        let mut plan = RoutePlan::default();
        plan.open_with_destination(&place("B"));
        assert!(take_route_focus(&mut plan));
        assert!(!take_route_focus(&mut plan));
    }

    #[test]
    fn navigation_banner_advances_through_maneuvers() {
        let route = Route {
            points: vec![(47.0, 28.8), (47.02, 28.82)],
            distance_m: 1000.0,
            duration_s: 600.0,
            steps: vec![
                crate::osm::RouteStep {
                    text: "Depart".into(),
                    distance_m: Some(100.0),
                    duration_s: Some(10.0),
                    location: Some((47.0, 28.8)),
                },
                crate::osm::RouteStep {
                    text: "Turn right".into(),
                    distance_m: Some(200.0),
                    duration_s: Some(30.0),
                    location: Some((47.01, 28.81)),
                },
                crate::osm::RouteStep {
                    text: "Arrive".into(),
                    distance_m: None,
                    duration_s: None,
                    location: Some((47.02, 28.82)),
                },
            ],
            summary: "Main".into(),
            legs: vec![],
        };
        let mut plan = RoutePlan {
            routes: vec![route.clone()],
            following: true,
            ..Default::default()
        };
        plan.next_step = initial_navigation_step(&route);
        assert_eq!(plan.next_step, 1, "the departure step is skipped");
        assert_eq!(
            plan.navigation_status().as_deref(),
            Some("Next: Turn right (destination)")
        );

        plan.next_step = next_navigation_step(plan.next_step, &route, 47.01, 28.81);
        assert_eq!(plan.next_step, 2);
        assert_eq!(
            plan.navigation_status().as_deref(),
            Some("Next: Arrive (destination)")
        );

        plan.next_step = next_navigation_step(plan.next_step, &route, 47.02, 28.82);
        assert_eq!(plan.next_step, 3);
        assert_eq!(
            plan.navigation_status().as_deref(),
            Some("Heading to destination")
        );
    }

    fn departure_and_turn(turn_location: Option<(f64, f64)>) -> Route {
        Route {
            points: vec![(47.0, 28.8), (47.02, 28.82)],
            distance_m: 1000.0,
            duration_s: 600.0,
            steps: vec![
                crate::osm::RouteStep {
                    text: "Depart".into(),
                    distance_m: None,
                    duration_s: None,
                    location: Some((47.0, 28.8)),
                },
                crate::osm::RouteStep {
                    text: "Turn right".into(),
                    distance_m: Some(120.0),
                    duration_s: None,
                    location: turn_location,
                },
            ],
            summary: "Main".into(),
            legs: vec![],
        }
    }

    #[test]
    fn navigation_banner_shows_the_distance_to_a_located_maneuver() {
        let route = departure_and_turn(Some((47.01, 28.81)));
        // Ten thousandths of a degree of latitude is about 1.1 km.
        let metres = next_navigation_distance(&route, 1, 47.0, 28.81).expect("located step");
        assert!((metres - 1112.0).abs() < 5.0, "distance was {metres}");

        let mut plan = RoutePlan {
            routes: vec![route.clone()],
            following: true,
            next_step: 1,
            nav_distance_m: Some(250.0),
            ..Default::default()
        };
        assert_eq!(
            plan.navigation_status().as_deref(),
            Some("Next: Turn right (destination, 250 m)")
        );

        plan.nav_distance_m = None;
        assert_eq!(
            plan.navigation_status().as_deref(),
            Some("Next: Turn right (destination)")
        );
    }

    #[test]
    fn navigation_banner_has_no_distance_without_a_maneuver_location() {
        let route = departure_and_turn(None);
        assert_eq!(next_navigation_distance(&route, 1, 47.0, 28.81), None);
        // Past the last maneuver there is no next step to measure.
        assert_eq!(next_navigation_distance(&route, 2, 47.0, 28.81), None);

        let plan = RoutePlan {
            routes: vec![route.clone()],
            following: true,
            next_step: 1,
            nav_distance_m: next_navigation_distance(&route, 1, 47.0, 28.81),
            ..Default::default()
        };
        assert_eq!(
            plan.navigation_status().as_deref(),
            Some("Next: Turn right (destination)")
        );
    }

    #[test]
    fn sheet_open_with_sets_the_selected_endpoint() {
        let mut origin_plan = RoutePlan::default();
        origin_plan.open_with_origin(&place("A"));
        assert_eq!(origin_plan.origin, Some(RoutePoint::new(47.0, 28.8, "A")));
        assert!(origin_plan.open && origin_plan.focus);
        assert!(origin_plan.error.is_none());
        assert!(!origin_plan.following);

        let mut destination_plan = RoutePlan::default();
        destination_plan.open_with_destination(&place("B"));
        assert_eq!(
            destination_plan.destination,
            Some(RoutePoint::new(47.0, 28.8, "B"))
        );
        assert!(destination_plan.open && destination_plan.focus);
    }
}
