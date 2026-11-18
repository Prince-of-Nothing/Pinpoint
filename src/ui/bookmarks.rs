//! Saved places screen
use super::theme;
use super::PinpointAppUI;
use eframe::egui::{self, RichText};

impl PinpointAppUI {
    pub fn draw_bookmarks_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .id_salt("bookmarks_screen")
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_bookmarks_body(ui, ctx));
    }

    fn draw_bookmarks_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Saved places (on-device only)");
        ui.add_space(8.0);
        let store = crate::bookmarks::BookmarkStore::default_location();
        let list = store.list();
        self.app.data().lock().unwrap().bookmarks_count = list.len();
        theme::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} saved", list.len()));
                if !list.is_empty() && ui.button("Copy GeoJSON").clicked() {
                    ctx.copy_text(store.to_geojson());
                    self.app
                        .set_notice(Some("Copied bookmarks GeoJSON to clipboard.".to_string()));
                }
            });
            for b in &list {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} ({:.4}, {:.4})",
                        b.name, b.latitude, b.longitude
                    ));
                    if ui.button("Remove").clicked() {
                        store.remove(&b.id);
                        self.app.refresh_bookmarks();
                    }
                    if ui.button("Map").clicked() {
                        self.focus_map_at(b.latitude, b.longitude);
                    }
                    if ui.button("Share").clicked() {
                        self.share_text(
                            ctx,
                            &format!(
                                "geo:{:.5},{:.5}?q={}",
                                b.latitude,
                                b.longitude,
                                crate::ui::map::encode_component(&b.name)
                            ),
                        );
                    }
                });
            }
            if list.is_empty() {
                ui.label("Nothing saved yet — add a place below.");
            }
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::card_title(ui, "Add place");
            ui.add(
                egui::TextEdit::singleline(&mut self.bookmark_name)
                    .hint_text("Name")
                    .desired_width(200.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.bookmark_lat)
                    .hint_text("Latitude")
                    .desired_width(120.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.bookmark_lon)
                    .hint_text("Longitude")
                    .desired_width(120.0),
            );
            if theme::primary_button(ui, "Save").clicked() {
                if let (Ok(lat), Ok(lon)) = (
                    self.bookmark_lat.trim().parse::<f64>(),
                    self.bookmark_lon.trim().parse::<f64>(),
                ) {
                    let name = if self.bookmark_name.trim().is_empty() {
                        "Saved place".to_string()
                    } else {
                        self.bookmark_name.clone()
                    };
                    store.add(&name, lat, lon);
                    self.app.refresh_bookmarks();
                    self.bookmark_name.clear();
                    self.bookmark_lat.clear();
                    self.bookmark_lon.clear();
                    self.app
                        .set_notice(Some("Place saved on-device.".to_string()));
                } else {
                    self.app
                        .set_error(Some("Enter valid latitude/longitude".to_string()));
                }
                let _ = ctx;
            }
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::card_title(ui, "Import GeoJSON");
            ui.label(
                RichText::new(
                    "Paste a FeatureCollection exported from Pinpoint (Saved → Copy GeoJSON).",
                )
                .size(10.0)
                .color(theme::MUTED),
            );
            ui.add(
                egui::TextEdit::multiline(&mut self.bookmark_import_text)
                    .hint_text("{\"type\":\"FeatureCollection\",...}")
                    .desired_rows(6)
                    .desired_width(ui.available_width()),
            );
            if ui.button("Import GeoJSON").clicked() {
                match store.import_geojson(&self.bookmark_import_text) {
                    Ok(summary) => {
                        self.app.refresh_bookmarks();
                        let message = format!(
                            "Imported {} places ({} duplicates, {} invalid, {} over limit).",
                            summary.imported,
                            summary.duplicates,
                            summary.invalid,
                            summary.over_limit
                        );
                        if summary.imported > 0 {
                            self.bookmark_import_text.clear();
                            self.app.set_notice(Some(message));
                        } else {
                            self.app
                                .set_error(Some(format!("{message} No new places were imported.")));
                        }
                    }
                    Err(error) => self.app.set_error(Some(error)),
                }
            }
        });
    }
}
