//! Places screen (routes / POIs / reverse)
use super::theme;
use super::{PendingAction, PinpointAppUI};
use eframe::egui::{self, RichText};

impl PinpointAppUI {
    pub fn draw_places_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .id_salt("places_screen")
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_places_body(ui, ctx));
    }

    fn draw_places_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Places — route, nearby POIs & address lookup");
        ui.add_space(8.0);

        // Route planner entry point (shared sheet on desktop and mobile).
        theme::card(ui, |ui| {
            theme::card_title(ui, "Route planner");
            ui.label(
                "Search for a place, then use Directions or Set as origin. The route sheet \
                 supports stops, avoid options, alternatives and exports.",
            );
            if theme::primary_button(ui, "Open route planner").clicked() {
                self.route_plan.open = true;
                if self.route_plan.origin.is_none() {
                    let (lat, lon, name) = {
                        let d = self.app.data();
                        let d = d.lock().unwrap();
                        d.my_location
                            .as_ref()
                            .map(|l| (l.latitude, l.longitude, "My location".to_string()))
                            .unwrap_or((
                                d.map_center_lat,
                                d.map_center_lon,
                                "Map centre".to_string(),
                            ))
                    };
                    self.route_plan.origin =
                        Some(crate::ui::route::RoutePoint::new(lat, lon, name));
                }
            }
        });
        ui.add_space(8.0);

        // POI form
        theme::card(ui, |ui| {
            theme::card_title(ui, "Nearby (Overpass)");
            for cat in [
                "food", "fuel", "pharmacy", "hospital", "hotel", "tourism", "water", "parking",
                "park",
            ] {
                if ui.selectable_label(self.poi_category == cat, cat).clicked() {
                    self.poi_category = cat.to_string();
                }
            }
            if theme::primary_button(ui, "Find nearby").clicked() {
                self.spawn_action(
                    PendingAction::Poi {
                        lat: 41.8902,
                        lon: -87.6323,
                        category: self.poi_category.clone(),
                    },
                    ctx,
                );
            }
        });
        ui.add_space(8.0);

        // Reverse form
        theme::card(ui, |ui| {
            theme::card_title(ui, "Reverse geocode");
            ui.add(
                egui::TextEdit::singleline(&mut self.reverse_label)
                    .hint_text("lat,lon e.g. 41.89,-87.63")
                    .desired_width(220.0),
            );
            if theme::primary_button(ui, "Lookup address").clicked() {
                let s = self.reverse_label.clone();
                {
                    let parts: Vec<&str> = s.split(',').collect();
                    if parts.len() == 2 {
                        if let (Ok(lat), Ok(lon)) = (
                            parts[0].trim().parse::<f64>(),
                            parts[1].trim().parse::<f64>(),
                        ) {
                            self.spawn_action(PendingAction::Reverse { lat, lon }, ctx);
                        }
                    }
                }
            }
        });

        // Nearby POI results (name, hours, phone, website, save, OSM link)
        let pois = { self.app.data().lock().unwrap().poi_results.clone() };
        if !pois.is_empty() {
            ui.add_space(8.0);
            theme::card(ui, |ui| {
                ui.label(
                    RichText::new(format!("Nearby places ({})", pois.len()))
                        .size(14.0)
                        .strong(),
                );
                for poi in &pois {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} {}", poi.icon, poi.name));
                        if !poi.opening_hours.is_empty() {
                            ui.label(RichText::new(&poi.opening_hours).size(10.0));
                        }
                        if !poi.phone.is_empty() {
                            ui.label(RichText::new(&poi.phone).size(10.0));
                        }
                        if ui.button("Save").clicked() {
                            let store = crate::bookmarks::BookmarkStore::default_location();
                            store.add(&poi.name, poi.latitude, poi.longitude);
                            self.app.refresh_bookmarks();
                            self.app.set_notice(Some(format!("Saved {}", poi.name)));
                        }
                        if ui.button("OSM").clicked() {
                            let url = format!(
                                "https://www.openstreetmap.org/?mlat={:.4}&mlon={:.4}#map=16/{:.4}/{:.4}",
                                poi.latitude, poi.longitude, poi.latitude, poi.longitude
                            );
                            let _ = webbrowser::open(&url);
                        }
                    });
                    if !poi.website.is_empty() {
                        ui.hyperlink_to(RichText::new(&poi.website).size(10.0), &poi.website);
                    }
                    ui.add_space(2.0);
                }
            });
        }

        // Route results (summary, steps, GPX export)
        let route = { self.app.data().lock().unwrap().last_route.clone() };
        if let Some(route) = route {
            ui.add_space(8.0);
            theme::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "Route — {:.2} km, ~{} min",
                            route.distance_m / 1000.0,
                            (route.duration_s / 60.0) as i64
                        ))
                        .size(14.0)
                        .strong(),
                    );
                    if ui.button("Export GPX").clicked() {
                        self.spawn_action(
                            PendingAction::ExportGpx {
                                name: "Pinpoint route".to_string(),
                            },
                            ctx,
                        );
                    }
                });
                if !route.summary.is_empty() {
                    ui.label(RichText::new(&route.summary).size(11.0));
                }
                for step in &route.steps {
                    ui.label(format!("• {}", step.text));
                }
            });
        }
        self.draw_route_sheet(ui, ctx, false, ctx.screen_rect().top() + 64.0);
    }
}
