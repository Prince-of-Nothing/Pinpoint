//! Map screen
use super::{PendingAction, PinpointAppUI};
use crate::i18n::{self, key};
use crate::map;
use crate::models::FriendLocation;
use eframe::egui::{self, Color32, Direction, RichText};

impl PinpointAppUI {
    pub fn draw_map_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if Self::is_compact(ctx) {
            self.draw_mobile_map_screen(ui, ctx);
            return;
        }
        let lang = self.lang;
        ui.horizontal(|ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(Direction::LeftToRight),
                |ui| {
                    ui.heading(i18n::t(lang, key::FRIEND_LOCATIONS));
                },
            );
        });
        ui.add_space(8.0);

        {
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            let unknown = "Unknown".to_string();
            let user_email = data.user.as_ref().map(|u| &u.email).unwrap_or(&unknown);
            ui.label(format!("Signed in as: {}", user_email));
        }

        // Quick location publish (persistent inputs so typing survives frames)
        ui.group(|ui| {
            ui.label(
                RichText::new(i18n::t(lang, key::PUBLISH_MY_LOCATION))
                    .size(12.0)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.publish_lat)
                        .hint_text(i18n::t(lang, key::LATITUDE))
                        .desired_width(100.0),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.publish_lon)
                        .hint_text(i18n::t(lang, key::LONGITUDE))
                        .desired_width(100.0),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button(i18n::t(lang, key::PUBLISH)).clicked() {
                    match (
                        self.publish_lat.trim().parse::<f64>(),
                        self.publish_lon.trim().parse::<f64>(),
                    ) {
                        (Ok(lat), Ok(lon))
                            if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) =>
                        {
                            self.spawn_action(
                                PendingAction::PublishLocation {
                                    lat,
                                    lon,
                                    accuracy: 10.0,
                                },
                                ctx,
                            );
                        }
                        _ => {
                            self.app.set_error(Some(
                                "Enter a valid latitude (-90..90) and longitude (-180..180)"
                                    .to_string(),
                            ));
                        }
                    }
                }
                if ui.button("📍 Use GPS").clicked() {
                    self.request_gps_publish(ctx);
                }
            });
        });

        ui.add_space(8.0);

        // Friendships with inline accept/reject for incoming requests
        ui.group(|ui| {
            ui.label(
                RichText::new(i18n::t(lang, key::FRIENDS))
                    .size(14.0)
                    .strong(),
            );
            ui.add_space(4.0);

            let binding = self.app.data();
            let data = binding.lock().unwrap();
            if data.friendships.is_empty() {
                ui.label(i18n::t(lang, key::NO_FRIENDS));
            } else {
                for friendship in &data.friendships {
                    let color = match friendship.status.as_str() {
                        "pending" => Color32::YELLOW,
                        "accepted" => Color32::GREEN,
                        _ => Color32::GRAY,
                    };
                    let label = format!(
                        "{} → {}",
                        friendship.requester_name, friendship.receiver_name
                    );
                    let id = friendship.id;
                    let is_incoming = data
                        .user
                        .as_ref()
                        .map(|u| u.id == friendship.receiver_id)
                        .unwrap_or(false);
                    ui.horizontal(|ui| {
                        ui.colored_label(color, &label);
                        if friendship.status == "pending" && is_incoming {
                            if ui.button(i18n::t(lang, key::ACCEPT)).clicked() {
                                self.spawn_action(
                                    PendingAction::RespondFriendship {
                                        id,
                                        action: "accept".to_string(),
                                    },
                                    ctx,
                                );
                            }
                            if ui.button(i18n::t(lang, key::REJECT)).clicked() {
                                self.spawn_action(
                                    PendingAction::RespondFriendship {
                                        id,
                                        action: "reject".to_string(),
                                    },
                                    ctx,
                                );
                            }
                        } else if friendship.status == "pending" {
                            ui.small(i18n::t(lang, key::AWAITING_RESPONSE));
                        }
                    });
                }
            }
        });

        ui.add_space(8.0);

        // Location shares
        ui.group(|ui| {
            ui.label(
                RichText::new(i18n::t(lang, key::LOCATION_SHARING))
                    .size(14.0)
                    .strong(),
            );
            ui.add_space(4.0);

            let binding = self.app.data();
            let data = binding.lock().unwrap();
            if data.shares.is_empty() {
                ui.label(i18n::t(lang, key::NO_SHARING));
            } else {
                for share in &data.shares {
                    let precision_color = match share.precision_level.as_str() {
                        "exact" => Color32::GREEN,
                        "approximate" => Color32::YELLOW,
                        "city_only" => Color32::RED,
                        _ => Color32::GRAY,
                    };
                    let is_active = share.is_active;
                    let share_id = share.id;
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            precision_color,
                            format!("{} - {}", share.direction, share.precision_level),
                        );
                        let label = if is_active {
                            i18n::t(lang, key::PAUSE)
                        } else {
                            i18n::t(lang, key::RESUME)
                        };
                        if ui.button(label).clicked() {
                            self.spawn_action(
                                PendingAction::ToggleShare {
                                    share_id,
                                    is_active: !is_active,
                                },
                                ctx,
                            );
                        }
                    });
                }
            }
        });

        ui.add_space(8.0);

        // Map visualization + location list
        let backend_url = self.app.backend_url();
        let online = !self.app.is_offline_mode();
        let binding = self.app.data();
        let mut data = binding.lock().unwrap();
        map::render_map_with(ui, &mut data, 320.0, Some(&backend_url), online, false);
        drop(data);
        self.handle_map_click(ctx);
        let data = binding.lock().unwrap();
        let (center_lat, center_lon, zoom) =
            (data.map_center_lat, data.map_center_lon, data.map_zoom);
        let locations: Vec<FriendLocation> = data.locations.clone();
        drop(data);

        ui.add_space(6.0);
        ui.group(|ui| {
            ui.horizontal(|ui| {
                let bytes = crate::tilepacks::packs_size();
                ui.label(
                    RichText::new(format!(
                        "Offline map packs: {:.1} KB cached",
                        bytes as f64 / 1024.0
                    ))
                    .size(12.0),
                );
                if ui.button("⬇ Download current area pack").clicked() {
                    self.spawn_action(
                        PendingAction::DownloadTilePack {
                            center_lat,
                            center_lon,
                            zoom,
                        },
                        ctx,
                    );
                }
            });
        });

        ui.add_space(8.0);

        // Navigation to friend locations
        ui.group(|ui| {
            ui.label(
                RichText::new(i18n::t(lang, key::NAVIGATE_TO_FRIEND))
                    .size(14.0)
                    .strong(),
            );
            ui.add_space(4.0);
            if locations.is_empty() {
                ui.label(i18n::t(lang, key::NO_LOCATIONS));
            } else {
                for loc in &locations {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} ({:.4}, {:.4})",
                            loc.user_name, loc.latitude, loc.longitude
                        ));
                        ui.add_space(4.0);
                        if ui.button(i18n::t(lang, key::NAVIGATE)).clicked() {
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
                            self.route_plan.destination = Some(crate::ui::route::RoutePoint::new(
                                loc.latitude,
                                loc.longitude,
                                loc.user_name.clone(),
                            ));
                            self.route_plan.open = true;
                            self.route_plan.error = None;
                        }
                        if ui.button(i18n::t(lang, key::OSM_MAP)).clicked() {
                            let url = format!(
                                "https://www.openstreetmap.org/#map=14/{:.4}/{:.4}",
                                loc.latitude, loc.longitude
                            );
                            let _ = webbrowser::open(&url);
                        }
                    });
                }
            }
        });

        ui.add_space(8.0);

        // Search for places (Nominatim)
        ui.group(|ui| {
            ui.label(
                RichText::new(i18n::t(lang, key::SEARCH_PLACES))
                    .size(14.0)
                    .strong(),
            );
            ui.add_space(4.0);
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            let results = data.search_results.clone();
            let error = data.search_error.clone();
            drop(data);
            ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text(i18n::t(lang, key::SEARCH_PLACES))
                    .desired_width(300.0),
            );
            ui.add_space(4.0);
            if ui.button(i18n::t(lang, key::SEARCH)).clicked()
                && !self.search_query.trim().is_empty()
            {
                let query = self.search_query.clone();
                self.spawn_action(PendingAction::SearchPlaces { query }, ctx);
            }
            if !results.is_empty() {
                ui.add_space(4.0);
                ui.label(format!(
                    "{} ({}):",
                    i18n::t(lang, key::RESULTS),
                    results.len()
                ));
                for place in results {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} ({:.4}, {:.4})",
                            place.display_name, place.latitude, place.longitude
                        ));
                        ui.add_space(4.0);
                        if ui.small_button("★ Save").clicked() {
                            let store = crate::bookmarks::BookmarkStore::default_location();
                            store.add(&place.short_name, place.latitude, place.longitude);
                            self.app
                                .set_notice(Some(format!("Saved {}", place.short_name)));
                        }
                        if ui.button("Directions").clicked() {
                            let data = self.app.data();
                            let mut d = data.lock().unwrap();
                            d.selected_place = Some(place.clone());
                            d.map_center_lat = place.latitude;
                            d.map_center_lon = place.longitude;
                            d.map_zoom = 15;
                            drop(d);
                            self.route_plan.open_with_destination(&place);
                        }
                        if ui.button(i18n::t(lang, key::OSM_MAP)).clicked() {
                            let url = format!(
                                "https://www.openstreetmap.org/#map=14/{:.4}/{:.4}",
                                place.latitude, place.longitude
                            );
                            let _ = webbrowser::open(&url);
                        }
                    });
                }
            }
            if let Some(err) = error {
                ui.colored_label(Color32::RED, err);
            }
        });
    }
}

impl PinpointAppUI {
    /// Map-first touch layout that mirrors the web shell: full-bleed map,
    /// floating search/zoom/locate controls, results dropdown and a bottom
    /// place/publish sheet.
    fn draw_mobile_map_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let top = ui.next_widget_position().y;
        let width = ctx.screen_rect().width();
        let height = (ui.available_height() - 26.0).max(320.0);
        let backend_url = self.app.backend_url();
        let online = !self.app.is_offline_mode();
        {
            let binding = self.app.data();
            let mut data = binding.lock().unwrap();
            crate::map::render_map_with(ui, &mut data, height, Some(&backend_url), online, true);
        }
        self.handle_map_click(ctx);

        let panel = Color32::from_rgba_unmultiplied(15, 23, 42, 246);
        let panel_solid = Color32::from_rgb(15, 23, 42);
        let border = Color32::from_rgb(31, 41, 55);
        let muted = Color32::from_rgb(156, 163, 175);

        // Floating search pill (web `.search-form`).
        let mut submit = false;
        egui::Area::new(egui::Id::new("mobile_map_search"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(12.0, top + 10.0))
            .show(ctx, |ui| {
                ui.set_width(width - 24.0);
                egui::Frame::new()
                    .fill(panel)
                    .stroke(egui::Stroke::new(1.0_f32, border))
                    .corner_radius(egui::CornerRadius::same(22))
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text("Search places…")
                                    .frame(false)
                                    .desired_width((ui.available_width() - 78.0).max(120.0)),
                            );
                            submit = ui.button("Search").clicked()
                                || (edit.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                        });
                    });
            });
        if submit && !self.search_query.trim().is_empty() {
            let query = self.search_query.clone();
            self.spawn_action(PendingAction::SearchPlaces { query }, ctx);
        }

        // Category chips when there are no results (web `.category-chips`).
        let results = self.app.data().lock().unwrap().search_results.clone();
        if results.is_empty() {
            let mut chip_query: Option<&'static str> = None;
            egui::Area::new(egui::Id::new("mobile_map_chips"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(12.0, top + 64.0))
                .show(ctx, |ui| {
                    ui.set_width(width - 24.0);
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        ui.set_width(width - 76.0);
                        ui.horizontal(|ui| {
                            for (label, query) in [
                                ("Food", "restaurant"),
                                ("Cafe", "cafe"),
                                ("Fuel", "fuel"),
                                ("Pharmacy", "pharmacy"),
                                ("Hospital", "hospital"),
                                ("Parking", "parking"),
                                ("ATM", "atm"),
                                ("Hotel", "hotel"),
                                ("Park", "park"),
                            ] {
                                let button = egui::Button::new(RichText::new(label).size(12.0))
                                    .fill(panel)
                                    .stroke(egui::Stroke::new(1.0_f32, border))
                                    .corner_radius(egui::CornerRadius::same(14))
                                    .min_size(egui::vec2(0.0, 30.0));
                                if ui.add(button).clicked() {
                                    chip_query = Some(query);
                                }
                            }
                        });
                    });
                });
            if let Some(query) = chip_query {
                self.search_query = query.to_string();
                self.spawn_action(
                    PendingAction::SearchPlaces {
                        query: query.to_string(),
                    },
                    ctx,
                );
            }
        }

        // Search results dropdown (web `.search-results`).
        if !results.is_empty() {
            let mut chosen = None;
            egui::Area::new(egui::Id::new("mobile_search_results"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(12.0, top + 64.0))
                .show(ctx, |ui| {
                    ui.set_width(width - 24.0);
                    egui::Frame::new()
                        .fill(panel_solid)
                        .stroke(egui::Stroke::new(1.0_f32, border))
                        .corner_radius(egui::CornerRadius::same(12))
                        .inner_margin(egui::Margin::same(8))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(300.0)
                                .show(ui, |ui| {
                                    for place in &results {
                                        let text =
                                            format!("{}\n{}", place.short_name, place.display_name);
                                        if ui
                                            .add_sized(
                                                egui::vec2(ui.available_width(), 42.0),
                                                egui::Button::new(text).wrap(),
                                            )
                                            .clicked()
                                        {
                                            chosen = Some(place.clone());
                                        }
                                        ui.separator();
                                    }
                                });
                        });
                });
            if let Some(place) = chosen {
                let binding = self.app.data();
                let mut data = binding.lock().unwrap();
                data.map_center_lat = place.latitude;
                data.map_center_lon = place.longitude;
                data.map_zoom = 15;
                data.search_results.clear();
                data.selected_place = Some(place.clone());
                drop(data);
                self.mobile_selected_place = Some(place);
                self.mobile_publish_open = false;
            }
        }

        // Floating zoom / locate / drawer controls.
        let mut zoom_delta = 0i8;
        let mut locate = false;
        let mut menu = false;
        let mut toggle_style = false;
        let mut open_route = false;
        let map_dark = self.app.data().lock().unwrap().map_dark;
        if !self.mobile_nav_open {
            egui::Area::new(egui::Id::new("mobile_map_controls"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(width - 56.0, top + 120.0))
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        if ui
                            .add(
                                egui::Button::new("+")
                                    .min_size(egui::vec2(40.0, 40.0))
                                    .corner_radius(egui::CornerRadius::same(20)),
                            )
                            .clicked()
                        {
                            zoom_delta = 1;
                        }
                        if ui
                            .add(
                                egui::Button::new("−")
                                    .min_size(egui::vec2(40.0, 40.0))
                                    .corner_radius(egui::CornerRadius::same(20)),
                            )
                            .clicked()
                        {
                            zoom_delta = -1;
                        }
                        if ui
                            .add(
                                egui::Button::new("◎")
                                    .min_size(egui::vec2(40.0, 40.0))
                                    .corner_radius(egui::CornerRadius::same(20)),
                            )
                            .clicked()
                        {
                            locate = true;
                        }
                        if ui
                            .add(
                                egui::Button::new(if map_dark { "L" } else { "D" })
                                    .min_size(egui::vec2(40.0, 40.0))
                                    .corner_radius(egui::CornerRadius::same(20)),
                            )
                            .on_hover_text(if map_dark {
                                "Use light map"
                            } else {
                                "Use dark map"
                            })
                            .clicked()
                        {
                            toggle_style = true;
                        }
                        if ui
                            .add(
                                egui::Button::new("↗")
                                    .min_size(egui::vec2(40.0, 40.0))
                                    .corner_radius(egui::CornerRadius::same(20)),
                            )
                            .on_hover_text("Route")
                            .clicked()
                        {
                            open_route = true;
                        }
                        if ui
                            .add(
                                egui::Button::new("☰")
                                    .min_size(egui::vec2(40.0, 40.0))
                                    .corner_radius(egui::CornerRadius::same(20)),
                            )
                            .clicked()
                        {
                            menu = true;
                        }
                    });
                });
        }
        if zoom_delta != 0 {
            let binding = self.app.data();
            let mut data = binding.lock().unwrap();
            let next = (data.map_zoom as i8 + zoom_delta).clamp(2, 18);
            data.map_zoom = next as u8;
        }
        if toggle_style {
            let binding = self.app.data();
            let mut data = binding.lock().unwrap();
            data.map_dark = !data.map_dark;
        }
        if locate {
            self.spawn_action(PendingAction::Locate, ctx);
        }
        if menu {
            self.mobile_nav_open = !self.mobile_nav_open;
        }
        if open_route {
            self.route_plan.open = true;
            if self.route_plan.origin.is_none() {
                let (lat, lon, name) = {
                    let d = self.app.data();
                    let d = d.lock().unwrap();
                    d.my_location
                        .as_ref()
                        .map(|l| (l.latitude, l.longitude, "My location".to_string()))
                        .unwrap_or((d.map_center_lat, d.map_center_lon, "Map centre".to_string()))
                };
                self.route_plan.origin = Some(crate::ui::route::RoutePoint::new(lat, lon, name));
            }
        }

        // Bottom sheet: publish form or selected place.
        if self.mobile_publish_open {
            let mut publish = false;
            let mut gps = false;
            let mut close = false;
            egui::Area::new(egui::Id::new("mobile_publish_sheet"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0))
                .show(ctx, |ui| {
                    ui.set_width(width - 24.0);
                    egui::Frame::new()
                        .fill(panel_solid)
                        .stroke(egui::Stroke::new(1.0_f32, border))
                        .corner_radius(egui::CornerRadius::same(12))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(i18n::t(self.lang, key::PUBLISH_MY_LOCATION))
                                        .size(16.0)
                                        .strong(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("×").clicked() {
                                            close = true;
                                        }
                                    },
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.publish_lat)
                                        .hint_text(i18n::t(self.lang, key::LATITUDE))
                                        .desired_width(120.0),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.publish_lon)
                                        .hint_text(i18n::t(self.lang, key::LONGITUDE))
                                        .desired_width(120.0),
                                );
                            });
                            ui.horizontal_wrapped(|ui| {
                                if ui.button(i18n::t(self.lang, key::PUBLISH)).clicked() {
                                    publish = true;
                                }
                                if ui.button("◎ Use GPS").clicked() {
                                    gps = true;
                                }
                            });
                        });
                });
            if close {
                self.mobile_publish_open = false;
            }
            if gps {
                self.request_gps_publish(ctx);
            }
            if publish {
                match (
                    self.publish_lat.trim().parse::<f64>(),
                    self.publish_lon.trim().parse::<f64>(),
                ) {
                    (Ok(lat), Ok(lon))
                        if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) =>
                    {
                        self.spawn_action(
                            PendingAction::PublishLocation {
                                lat,
                                lon,
                                accuracy: 10.0,
                            },
                            ctx,
                        );
                        self.mobile_publish_open = false;
                    }
                    _ => self.app.set_error(Some(
                        "Enter a valid latitude (-90..90) and longitude (-180..180)".to_string(),
                    )),
                }
            }
        } else if let Some(place) = self.mobile_selected_place.clone() {
            let mut close = false;
            let mut save = false;
            let mut directions = false;
            let mut set_origin = false;
            let mut copy = false;
            let mut share = false;
            let mut open_url: Option<String> = None;
            let name = encode_component(&place.short_name);
            let distance_m = {
                let d = self.app.data();
                let d = d.lock().unwrap();
                crate::ui::route::haversine_m(
                    d.map_center_lat,
                    d.map_center_lon,
                    place.latitude,
                    place.longitude,
                )
            };
            egui::Area::new(egui::Id::new("mobile_place_sheet"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0))
                .show(ctx, |ui| {
                    ui.set_width(width - 24.0);
                    egui::Frame::new()
                        .fill(panel_solid)
                        .stroke(egui::Stroke::new(1.0_f32, border))
                        .corner_radius(egui::CornerRadius::same(12))
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&place.short_name)
                                        .size(18.0)
                                        .strong(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("×").clicked() {
                                            close = true;
                                        }
                                    },
                                );
                            });
                            ui.label(
                                RichText::new(&place.display_name)
                                    .size(12.0)
                                    .color(muted),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} from map centre",
                                    crate::ui::route::format_distance(distance_m)
                                ))
                                .size(11.0)
                                .color(muted),
                            );
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                if ui.button("Directions").clicked() {
                                    directions = true;
                                }
                                if ui.button("Set as origin").clicked() {
                                    set_origin = true;
                                }
                                if ui.button("Save").clicked() {
                                    save = true;
                                }
                                if ui.button("Copy").clicked() {
                                    copy = true;
                                }
                                if ui.button("Share").clicked() {
                                    share = true;
                                }
                                if ui.button("OSM").clicked() {
                                    open_url = Some(format!(
                                        "https://www.openstreetmap.org/?mlat={:.4}&mlon={:.4}#map=15/{:.4}/{:.4}",
                                        place.latitude,
                                        place.longitude,
                                        place.latitude,
                                        place.longitude
                                    ));
                                }
                                if ui.button("Google").clicked() {
                                    open_url = Some(format!(
                                        "https://www.google.com/maps/search/?api=1&query={:.4},{:.4}",
                                        place.latitude, place.longitude
                                    ));
                                }
                                if ui.button("Organic").clicked() {
                                    open_url = Some(format!(
                                        "https://omaps.app/map?v=1&ll={:.4},{:.4}&n={name}",
                                        place.latitude, place.longitude
                                    ));
                                }
                                if ui.button("Magic Earth").clicked() {
                                    open_url = Some(format!(
                                        "magicearth://?show_on_map&lat={:.4}&lon={:.4}&name={name}",
                                        place.latitude, place.longitude
                                    ));
                                }
                            });
                        });
                });
            if close {
                self.mobile_selected_place = None;
                self.app.data().lock().unwrap().selected_place = None;
            }
            if save {
                let store = crate::bookmarks::BookmarkStore::default_location();
                store.add(&place.short_name, place.latitude, place.longitude);
                self.app
                    .set_notice(Some(format!("Saved {}", place.short_name)));
            }
            if copy {
                ctx.copy_text(format!(
                    "{} ({:.5}, {:.5})",
                    place.short_name, place.latitude, place.longitude
                ));
                self.app.set_notice(Some("Place copied.".to_string()));
            }
            if share {
                self.share_text(
                    ctx,
                    &format!("geo:{:.6},{:.6}?q={name}", place.latitude, place.longitude),
                );
            }
            if directions {
                self.route_plan.open_with_destination(&place);
                self.mobile_selected_place = None;
                self.app.data().lock().unwrap().selected_place = None;
            }
            if set_origin {
                self.route_plan.open_with_origin(&place);
                self.mobile_selected_place = None;
                self.app.data().lock().unwrap().selected_place = None;
            }
            if let Some(url) = open_url {
                let _ = webbrowser::open(&url);
            }
        }
        self.draw_route_sheet(ui, ctx, true, top);
    }
}

impl PinpointAppUI {
    fn request_gps_publish(&mut self, ctx: &egui::Context) {
        match crate::platform::ensure_location_permission() {
            crate::platform::LocationPermission::Granted => {
                self.spawn_action(PendingAction::PublishGpsLocation, ctx);
            }
            crate::platform::LocationPermission::Requested => {
                self.app.set_notice(Some(
                    "Location permission requested. Tap Use GPS again after granting it."
                        .to_string(),
                ));
            }
            crate::platform::LocationPermission::Unsupported => {
                self.app
                    .set_error(Some("Location is unavailable on this device.".to_string()));
            }
        }
    }
}

/// Percent-encode a URL path/query component (UTF-8 bytes, unreserved kept).
fn encode_component(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
