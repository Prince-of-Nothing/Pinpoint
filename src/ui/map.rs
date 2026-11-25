//! Map screen
use super::route::RoutePoint;
use super::theme;
use super::{PendingAction, PinpointAppUI};
use crate::app::CurrentScreen;
use crate::i18n::{self, key};
use crate::map;
use crate::models::{FriendLocation, MapMenuAction};
use eframe::egui::{self, RichText};

impl PinpointAppUI {
    pub fn draw_map_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // Android asks once on the first map view, then centres on the fix
        // when the system dialog is accepted. Desktop and iOS stay unchanged.
        let android = cfg!(target_os = "android");
        if android && !self.location_prompted {
            self.location_prompted = true;
            if crate::platform::ensure_location_permission()
                == crate::platform::LocationPermission::Granted
            {
                self.spawn_action(PendingAction::Locate, ctx);
            } else {
                self.location_pending = true;
                self.app.set_notice(Some(
                    "Allow location access to show your position on the map.".to_string(),
                ));
            }
        } else if self.location_pending && crate::platform::location_permission_granted() {
            self.location_pending = false;
            self.spawn_action(PendingAction::Locate, ctx);
        }
        if Self::is_compact(ctx) {
            self.draw_mobile_map_screen(ui, ctx);
            return;
        }
        // Map first: the canvas takes every pixel the details column does not.
        let details_width = (ui.available_width() * 0.36).clamp(300.0, 380.0);
        // 14 pt gap + the parent's 10 pt item spacing.
        let map_width = (ui.available_width() - details_width - 24.0).max(320.0);
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(map_width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.draw_map_card(ui, ctx),
            );
            ui.add_space(14.0);
            ui.allocate_ui_with_layout(
                egui::vec2(details_width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("map_details")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.draw_map_details(ui, ctx));
                },
            );
        });
        self.draw_route_sheet(ui, ctx, false, ctx.screen_rect().top() + 64.0);
    }

    fn draw_map_card(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.lang;
        let backend_url = self.app.backend_url();
        let online = !self.app.is_offline_mode();
        let (zoom, has_pins) = {
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            (data.map_zoom, !data.locations.is_empty())
        };
        let mut zoom_delta = 0i8;
        let mut fit_pins = false;
        let mut reset = false;
        let mut locate = false;

        theme::full_card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(i18n::t(lang, key::MAP)).size(18.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::platform::map_webview_supported() {
                        let visible = crate::platform::map_webview_visible();
                        let label = if visible {
                            "Close offline map"
                        } else {
                            "Offline map"
                        };
                        if ui.button(label).clicked() {
                            self.toggle_webview_map();
                        }
                    }
                    if ui.button("Locate").clicked() {
                        locate = true;
                    }
                    if ui
                        .add_enabled(has_pins, egui::Button::new("Fit pins"))
                        .clicked()
                    {
                        fit_pins = true;
                    }
                    if ui.button("Reset view").clicked() {
                        reset = true;
                    }
                    if ui.button("+").clicked() {
                        zoom_delta = 1;
                    }
                    ui.label(
                        RichText::new(format!("z{zoom}"))
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                    if ui.button("−").clicked() {
                        zoom_delta = -1;
                    }
                });
            });
            ui.add_space(6.0);
            let map_height = (ui.available_height() - 34.0).max(170.0);
            let binding = self.app.data();
            let mut data = binding.lock().unwrap();
            map::render_map_with(ui, &mut data, map_height, Some(&backend_url), online, true);
            if data.measure_mode {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(crate::models::measure_label(&data.measure_points))
                            .size(12.0)
                            .strong(),
                    );
                    if ui.button("Undo").clicked() {
                        data.measure_points.pop();
                    }
                    if ui.button("Clear").clicked() {
                        data.measure_mode = false;
                        data.measure_points.clear();
                    }
                });
            }
        });

        let binding = self.app.data();
        let mut data = binding.lock().unwrap();
        if zoom_delta != 0 {
            data.map_zoom = (data.map_zoom as i8 + zoom_delta).clamp(2, 18) as u8;
        }
        if reset {
            data.map_center_lat = map::DEFAULT_CENTER_LAT;
            data.map_center_lon = map::DEFAULT_CENTER_LON;
            data.map_zoom = map::DEFAULT_ZOOM;
        }
        if fit_pins && !data.locations.is_empty() {
            let n = data.locations.len() as f64;
            data.map_center_lat = data.locations.iter().map(|l| l.latitude).sum::<f64>() / n;
            data.map_center_lon = data.locations.iter().map(|l| l.longitude).sum::<f64>() / n;
        }
        drop(data);
        if locate {
            self.spawn_action(PendingAction::Locate, ctx);
        }
        self.handle_map_click(ctx);
        self.handle_map_menu(ctx);
    }

    /// Consume one right-click action from the native map menu.
    fn handle_map_menu(&mut self, ctx: &egui::Context) {
        let action = { self.app.data().lock().unwrap().map_menu_action.take() };
        let Some(action) = action else {
            return;
        };
        match action {
            MapMenuAction::DirectionsFrom(lat, lon) => {
                self.route_plan.origin = Some(RoutePoint::new(lat, lon, "Dropped pin"));
                self.route_plan.open = true;
                self.route_plan.error = None;
                self.app
                    .set_notice(Some("Origin set. Choose a destination.".into()));
            }
            MapMenuAction::DirectionsTo(lat, lon) => {
                if self.route_plan.origin.is_none() {
                    let origin = {
                        let binding = self.app.data();
                        let data = binding.lock().unwrap();
                        data.my_location
                            .as_ref()
                            .map(|loc| RoutePoint::new(loc.latitude, loc.longitude, "My location"))
                    }
                    .unwrap_or_else(|| {
                        RoutePoint::new(
                            map::DEFAULT_CENTER_LAT,
                            map::DEFAULT_CENTER_LON,
                            "Map centre",
                        )
                    });
                    self.route_plan.origin = Some(origin);
                }
                self.route_plan.destination = Some(RoutePoint::new(lat, lon, "Dropped pin"));
                self.route_plan.open = true;
                self.route_plan.error = None;
            }
            MapMenuAction::MeasureFrom(lat, lon) => {
                let data = self.app.data();
                let mut data = data.lock().unwrap();
                data.measure_mode = true;
                data.measure_points.clear();
                data.measure_points.push((lat, lon));
                drop(data);
                self.app
                    .set_notice(Some("Measuring. Click the map to add points.".into()));
            }
            MapMenuAction::AddNote(lat, lon) => {
                self.center_map_at(lat, lon);
                self.app.set_screen(CurrentScreen::Today);
                self.app
                    .set_notice(Some("Add a private note for this point.".into()));
            }
            MapMenuAction::CopyCoordinates(lat, lon) => {
                ctx.copy_text(format!("{lat:.5}, {lon:.5}"));
                self.app.set_notice(Some("Coordinates copied.".into()));
            }
        }
    }

    fn draw_map_details(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.lang;

        // Quick location publish (persistent inputs so typing survives frames).
        theme::full_card(ui, |ui| {
            theme::card_title(ui, i18n::t(lang, key::PUBLISH_MY_LOCATION));
            theme::caption(ui, "Manual coordinates or a fresh GPS fix.");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.publish_lat)
                        .hint_text(i18n::t(lang, key::LATITUDE))
                        .desired_width(110.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.publish_lon)
                        .hint_text(i18n::t(lang, key::LONGITUDE))
                        .desired_width(110.0),
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if theme::primary_button(ui, i18n::t(lang, key::PUBLISH)).clicked() {
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
                if ui.button("Use GPS").clicked() {
                    self.request_gps_publish(ctx);
                }
            });
        });

        ui.add_space(10.0);

        // Friendships with inline accept/reject for incoming requests.
        theme::full_card(ui, |ui| {
            theme::card_title(ui, i18n::t(lang, key::FRIENDS));
            ui.add_space(2.0);
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            if data.friendships.is_empty() {
                theme::caption(ui, i18n::t(lang, key::NO_FRIENDS));
            } else {
                for friendship in &data.friendships {
                    let color = match friendship.status.as_str() {
                        "pending" => theme::ACCENT,
                        "accepted" => theme::SUCCESS,
                        _ => theme::MUTED,
                    };
                    let label = format!(
                        "{} ↔ {}",
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
                            theme::caption(ui, i18n::t(lang, key::AWAITING_RESPONSE));
                        }
                    });
                }
            }
        });

        ui.add_space(10.0);

        // Location shares.
        theme::full_card(ui, |ui| {
            theme::card_title(ui, i18n::t(lang, key::LOCATION_SHARING));
            ui.add_space(2.0);
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            if data.shares.is_empty() {
                theme::caption(ui, i18n::t(lang, key::NO_SHARING));
            } else {
                for share in &data.shares {
                    let precision_color = match share.precision_level.as_str() {
                        "exact" => theme::SUCCESS,
                        "approximate" => theme::ACCENT,
                        "city_only" => theme::DANGER,
                        _ => theme::MUTED,
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

        ui.add_space(10.0);

        // Search for places (Nominatim).
        theme::full_card(ui, |ui| {
            theme::card_title(ui, i18n::t(lang, key::SEARCH_PLACES));
            ui.add_space(2.0);
            let (results, error) = {
                let binding = self.app.data();
                let data = binding.lock().unwrap();
                (data.search_results.clone(), data.search_error.clone())
            };
            ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text(i18n::t(lang, key::SEARCH_PLACES))
                    .char_limit(120)
                    .desired_width((ui.available_width() - 16.0).max(120.0)),
            );
            ui.add_space(4.0);
            if theme::primary_button(ui, i18n::t(lang, key::SEARCH)).clicked()
                && !self.search_query.trim().is_empty()
            {
                let query = self.search_query.clone();
                self.spawn_action(PendingAction::SearchPlaces { query }, ctx);
            }
            if !results.is_empty() {
                ui.add_space(6.0);
                theme::caption(
                    ui,
                    format!("{} ({})", i18n::t(lang, key::RESULTS), results.len()),
                );
                for place in results {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "{} ({:.4}, {:.4})",
                            place.display_name, place.latitude, place.longitude
                        ));
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Save").clicked() {
                            let store = crate::bookmarks::BookmarkStore::default_location();
                            store.add(&place.short_name, place.latitude, place.longitude);
                            self.app.refresh_bookmarks();
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
                        if ui.button("Share").clicked() {
                            self.share_text(
                                ctx,
                                &format!(
                                    "geo:{:.5},{:.5}?q={}",
                                    place.latitude,
                                    place.longitude,
                                    encode_component(&place.short_name)
                                ),
                            );
                        }
                    });
                    ui.separator();
                }
            }
            if let Some(err) = error {
                ui.colored_label(theme::DANGER, err);
            }
        });

        ui.add_space(10.0);

        // Navigation to friend locations.
        let locations: Vec<FriendLocation> = self.app.data().lock().unwrap().locations.clone();
        theme::full_card(ui, |ui| {
            theme::card_title(ui, i18n::t(lang, key::NAVIGATE_TO_FRIEND));
            ui.add_space(2.0);
            if locations.is_empty() {
                theme::caption(ui, i18n::t(lang, key::NO_LOCATIONS));
            } else {
                for loc in &locations {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&loc.user_name).strong());
                        theme::caption(
                            ui,
                            format!(
                                "{:.4}, {:.4} · updated {}",
                                loc.latitude,
                                loc.longitude,
                                crate::models::format_age(&loc.updated_at)
                            ),
                        );
                    });
                    ui.horizontal_wrapped(|ui| {
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
                    ui.separator();
                }
            }
        });

        ui.add_space(10.0);

        // Offline tile packs for the current viewport.
        let (center_lat, center_lon, zoom) = {
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            (data.map_center_lat, data.map_center_lon, data.map_zoom)
        };
        theme::full_card(ui, |ui| {
            theme::card_title(ui, "Offline maps");
            let bytes = crate::tilepacks::packs_size();
            theme::caption(
                ui,
                format!("{:.1} KB cached on this device", bytes as f64 / 1024.0),
            );
            ui.add_space(4.0);
            if ui.button("Download current area").clicked() {
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

        ui.add_space(10.0);

        #[cfg(all(
            feature = "map-webview",
            not(target_arch = "wasm32"),
            not(target_os = "android"),
            not(target_os = "ios")
        ))]
        self.draw_map_window_card(ui);

        theme::full_card(ui, |ui| {
            theme::card_title(ui, "Map data");
            theme::caption(ui, "OpenStreetMap contributors · ODbL");
            theme::caption(
                ui,
                "Pins: exact (green), approximate (amber), city-only (red).",
            );
            ui.add_space(4.0);
            map::draw_map_links(ui, &locations);
        });
    }
}

#[cfg(all(
    feature = "map-webview",
    not(target_arch = "wasm32"),
    not(target_os = "android"),
    not(target_os = "ios")
))]
impl PinpointAppUI {
    /// Visible entry point for the separate MapLibre WebView window. The
    /// egui map below remains the fallback and keeps its own data/auth paths.
    fn draw_map_window_card(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        theme::full_card(ui, |ui| {
            theme::card_title(ui, "Offline map window");
            theme::caption(
                ui,
                "Opens the full map in its own window. The map above stays available as a fallback.",
            );
            ui.add_space(4.0);
            let running = self.map_window.is_running();
            let label = if running {
                "Map window open"
            } else if self.map_window.browser_fallback() {
                "Open map in browser"
            } else {
                "Open map window"
            };
            if ui.add_enabled(!running, egui::Button::new(label)).clicked() {
                match self.map_window.open() {
                    Ok(crate::map_window::LaunchOutcome::Started) => self.app.set_notice(Some(
                        "Map window opened. It loads the local MapLibre page and fixture."
                            .to_string(),
                    )),
                    Ok(crate::map_window::LaunchOutcome::BrowserFallback) => {
                        self.app.set_notice(Some(
                            "The native WebView runtime is unavailable, so the same loopback \
                             page opened in your browser. The local server stops when Pinpoint \
                             exits."
                                .to_string(),
                        ))
                    }
                    Ok(crate::map_window::LaunchOutcome::AlreadyRunning) => self
                        .app
                        .set_notice(Some("The map window is already open.".to_string())),
                    Err(error) => self.app.set_error(Some(error)),
                }
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

        // A click in the MapLibre WebView comes back through JNI. Follow it on
        // the egui map and keep the point for the share control once the
        // overlay is closed (the WebView covers egui while it is open).
        if crate::platform::map_webview_supported() {
            if let Some((lat, lon)) = crate::platform::take_map_webview_click() {
                self.webview_map_click = Some((lat, lon));
                let binding = self.app.data();
                let mut data = binding.lock().unwrap();
                data.map_center_lat = lat;
                data.map_center_lon = lon;
            }
        }

        {
            let binding = self.app.data();
            let mut data = binding.lock().unwrap();
            crate::map::render_map_with(ui, &mut data, height, Some(&backend_url), online, true);
        }
        self.handle_map_click(ctx);
        self.handle_map_menu(ctx);

        let panel = theme::PANEL.gamma_multiply(0.98);
        let panel_solid = theme::PANEL;
        let border = theme::BORDER;
        let muted = theme::MUTED;

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
                                    .char_limit(120)
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
                                    .min_size(egui::vec2(0.0, 44.0));
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
        let mut toggle_webview = false;
        let mut share_webview = false;
        let map_dark = self.app.data().lock().unwrap().map_dark;
        let webview_visible = crate::platform::map_webview_visible();
        if !self.mobile_nav_open {
            egui::Area::new(egui::Id::new("mobile_map_controls"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(width - 56.0, top + 120.0))
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        if ui.add(round_button("+")).clicked() {
                            zoom_delta = 1;
                        }
                        if ui.add(round_button("−")).clicked() {
                            zoom_delta = -1;
                        }
                        if ui.add(round_button("◎")).clicked() {
                            locate = true;
                        }
                        if ui
                            .add(round_button(if map_dark { "L" } else { "D" }))
                            .on_hover_text(if map_dark {
                                "Use light map"
                            } else {
                                "Use dark map"
                            })
                            .clicked()
                        {
                            toggle_style = true;
                        }
                        if ui.add(round_button("↗")).on_hover_text("Route").clicked() {
                            open_route = true;
                        }
                        if crate::platform::map_webview_supported() {
                            if ui
                                .add(round_button(if webview_visible { "▣" } else { "▦" }))
                                .on_hover_text("Offline MapLibre map")
                                .clicked()
                            {
                                toggle_webview = true;
                            }
                            if !webview_visible {
                                if let Some((lat, lon)) = self.webview_map_click {
                                    if ui
                                        .add(round_button("⌖"))
                                        .on_hover_text(format!(
                                            "Share WebView point ({lat:.4}, {lon:.4})"
                                        ))
                                        .clicked()
                                    {
                                        share_webview = true;
                                    }
                                }
                            }
                        }
                        if ui.add(round_button("☰")).clicked() {
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
        if toggle_webview {
            self.toggle_webview_map();
        }
        if share_webview {
            if let Some((lat, lon)) = self.webview_map_click {
                if let Err(error) =
                    crate::platform::share_map_webview_point(lat, lon, "Pinpoint map")
                {
                    self.app
                        .set_error(Some(format!("Could not share the map point: {error}")));
                }
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
                                #[cfg(target_os = "android")]
                                if ui.button("Settings").clicked() {
                                    if let Err(error) = crate::platform::open_app_settings() {
                                        self.app.set_error(Some(format!(
                                            "Could not open app settings: {error}"
                                        )));
                                    }
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
                self.app.refresh_bookmarks();
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
    /// Show or hide the Android MapLibre overlay from either map layout.
    fn toggle_webview_map(&self) {
        let result = if crate::platform::map_webview_visible() {
            crate::platform::hide_map_webview()
        } else {
            crate::platform::show_map_webview()
        };
        if let Err(error) = result {
            self.app
                .set_error(Some(format!("Offline MapLibre map: {error}")));
        }
    }

    fn request_gps_publish(&mut self, ctx: &egui::Context) {
        match crate::platform::ensure_location_permission() {
            crate::platform::LocationPermission::Granted => {
                self.spawn_action(PendingAction::PublishGpsLocation, ctx);
            }
            crate::platform::LocationPermission::Requested => {
                self.app.set_notice(Some(
                    "Location permission requested. Allow it, then tap Use GPS again. \
                     If no dialog appears, open Settings → Permissions."
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

/// Floating map control: solid panel fill so the glyph stays readable over
/// the light basemap.
fn round_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(text)
        .min_size(egui::vec2(44.0, 44.0))
        .corner_radius(egui::CornerRadius::same(20))
        .fill(theme::PANEL)
}

/// Percent-encode a URL path/query component (UTF-8 bytes, unreserved kept).
pub(crate) fn encode_component(value: &str) -> String {
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
