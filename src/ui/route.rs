//! Route planning state and the shared route sheet.
//!
//! The web client owns this flow in `web/app/nav.js`; this module is the
//! native equivalent. State lives in [`RoutePlan`] on [`super::PinpointAppUI`],
//! while the chosen route is copied to `AppData.last_route` for the map.

use crate::models::AppData;
use crate::osm::{MapCapabilities, Route, SearchPlace};
use eframe::egui::{self, Color32, RichText};
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
    pub capabilities: Option<MapCapabilities>,
    pub error: Option<String>,
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
            capabilities: None,
            error: None,
        }
    }
}

impl RoutePlan {
    pub fn open_with_destination(&mut self, place: &SearchPlace) {
        self.destination = Some(RoutePoint::from_place(place));
        self.open = true;
        self.error = None;
        self.following = false;
    }

    pub fn open_with_origin(&mut self, place: &SearchPlace) {
        self.origin = Some(RoutePoint::from_place(place));
        self.open = true;
        self.error = None;
        self.following = false;
    }

    pub fn swap(&mut self) {
        std::mem::swap(&mut self.origin, &mut self.destination);
        self.routes.clear();
        self.selected = 0;
        self.following = false;
    }

    pub fn clear_route(&mut self) {
        self.routes.clear();
        self.selected = 0;
        self.following = false;
        self.next_stop = 0;
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

impl super::PinpointAppUI {
    /// Poll the platform fix once per second while navigation is active and
    /// advance the next-stop counter when close enough. The env/file provider
    /// is the current Android/desktop fallback; a real LocationManager bridge
    /// can replace it without touching this loop.
    pub(crate) fn tick_navigation(&mut self) {
        if !self.route_plan.following {
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

        if self.route_plan.next_stop < self.route_plan.stops.len() {
            if let Some(stop) = self.route_plan.stops.get(self.route_plan.next_stop) {
                if haversine_m(lat, lon, stop.lat, stop.lon) < 60.0 {
                    self.route_plan.next_stop += 1;
                }
            }
        }
    }

    /// Human-readable next-step line for the following banner.
    pub(crate) fn navigation_status(&self) -> Option<String> {
        if !self.route_plan.following {
            return None;
        }
        let route = self.route_plan.selected_route()?;
        let step = route.steps.get(self.route_plan.next_stop);
        let target = if self.route_plan.next_stop < self.route_plan.stops.len() {
            format!("stop {}", self.route_plan.next_stop + 1)
        } else {
            "destination".to_string()
        };
        Some(match step {
            Some(step) => format!("Next: {} ({})", step.text, target),
            None => format!("Heading to {target}"),
        })
    }

    /// Consume one map tap: set B when the route sheet is open, otherwise
    /// reverse-geocode it into the place sheet.
    pub(crate) fn handle_map_click(&mut self, ctx: &egui::Context) {
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
        self.route_plan.error = error;
        let app_data = self.app.data();
        let mut data = app_data.lock().unwrap();
        self.route_plan.apply_selected(&mut data);
        self.route_plan.open = true;
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
        let width = ctx.screen_rect().width();
        let panel = Color32::from_rgba_unmultiplied(15, 23, 42, 248);
        let border = Color32::from_rgb(31, 41, 55);
        let muted = Color32::from_rgb(156, 163, 175);
        let accent = Color32::from_rgb(52, 211, 153);
        let mut plan = self.route_plan.clone();
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
                            if ui.button("×").clicked() {
                                close = true;
                            }
                        });
                    });
                    egui::ScrollArea::vertical()
                        .max_height(if compact {
                            (ctx.screen_rect().height() - top - 130.0).max(240.0)
                        } else {
                            520.0
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
                                if ui.small_button("Use map centre").clicked() {
                                    let (lat, lon) = {
                                        let d = self.app.data();
                                        let d = d.lock().unwrap();
                                        (d.map_center_lat, d.map_center_lon)
                                    };
                                    plan.origin = Some(RoutePoint::new(lat, lon, "Map centre"));
                                }
                                if ui.small_button("Use my location").clicked() {
                                    let (lat, lon) = {
                                        let d = self.app.data();
                                        let d = d.lock().unwrap();
                                        d.my_location
                                            .as_ref()
                                            .map(|l| (l.latitude, l.longitude))
                                            .unwrap_or((47.0105, 28.8638))
                                    };
                                    plan.origin = Some(RoutePoint::new(lat, lon, "My location"));
                                }
                                if ui.small_button("Swap").clicked() {
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
                            if let Some(place) = self
                                .selected_place()
                                .or_else(|| self.app.data().lock().unwrap().selected_place.clone())
                            {
                                if ui.small_button("Use selected place as B").clicked() {
                                    plan.destination = Some(RoutePoint::from_place(&place));
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
                                    if ui.small_button("×").clicked() {
                                        remove_stop = Some(index);
                                    }
                                });
                            }
                            if ui.small_button("+ Add selected place as stop").clicked() {
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
                                if ui.button("Route").clicked() {
                                    plan_requested = true;
                                }
                            });

                            if let Some(error) = &plan.error {
                                ui.colored_label(Color32::from_rgb(248, 113, 113), error);
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
                                    if let Some(status) = self.navigation_status() {
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
                                    } else if ui.button("Navigate").clicked() {
                                        navigate = true;
                                    }
                                    if ui.button("Share").clicked() {
                                        copy_share = true;
                                    }
                                    if ui.small_button("GPX").clicked() {
                                        copy_share = true;
                                    }
                                    if ui.small_button("KML").clicked() {
                                        copy_kml = true;
                                    }
                                    if ui.small_button("GeoJSON").clicked() {
                                        copy_geojson = true;
                                    }
                                    if ui.small_button("Clear").clicked() {
                                        clear = true;
                                    }
                                });
                                ui.horizontal_wrapped(|ui| {
                                    if ui.small_button("Google Maps").clicked() {
                                        external = google_route_url(&plan);
                                    }
                                    if ui.small_button("Organic Maps").clicked() {
                                        external = organic_route_url(&plan);
                                    }
                                    if ui.small_button("Magic Earth").clicked() {
                                        external = magic_route_url(&plan);
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

        if close {
            plan.open = false;
            plan.following = false;
        }
        if clear {
            plan.clear_route();
            self.app.data().lock().unwrap().last_route = None;
        }
        if remove_stop.is_some() || add_stop {
            if let Some(index) = remove_stop {
                plan.remove_stop(index);
            }
            if add_stop {
                if let Some(place) = self.app.data().lock().unwrap().selected_place.clone() {
                    plan.add_stop(RoutePoint::from_place(&place));
                } else {
                    plan.error = Some("Select a place first to add it as a stop.".into());
                }
            }
            plan.error = None;
        }
        if plan_requested {
            self.request_route(&plan, ctx);
        }
        if navigate {
            plan.following = true;
            plan.next_stop = 0;
            self.app.set_notice(Some(
                "Route preview started. Live GPS follows when a fix is available.".to_string(),
            ));
        }
        if copy_share {
            ctx.copy_text(plan.share_text());
            self.app
                .set_notice(Some("Route summary copied.".to_string()));
        }
        if copy_kml || copy_geojson {
            if let Some(route) = plan.selected_route() {
                let text = if copy_kml {
                    crate::osm::route_to_kml(route, "Pinpoint route")
                } else {
                    crate::osm::route_to_geojson(route, "Pinpoint route")
                };
                ctx.copy_text(text);
                self.app.set_notice(Some(if copy_kml {
                    "KML copied to clipboard.".to_string()
                } else {
                    "GeoJSON copied to clipboard.".to_string()
                }));
            }
        }
        if let Some(url) = external {
            let _ = webbrowser::open(&url);
        }
        self.route_plan = plan;
        let app_data = self.app.data();
        let mut data = app_data.lock().unwrap();
        self.route_plan.apply_selected(&mut data);
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
}
