//! Map screen
use super::live_strip;
use super::route::RoutePoint;
use super::theme;
use super::{PendingAction, PinpointAppUI};
use crate::app::CurrentScreen;
use crate::i18n::{self, key};
use crate::map;
use crate::models::{FriendLocation, MapMenuAction};
use eframe::egui::{self, RichText};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlaceSheetAction {
    None,
    Close,
    Directions,
    SetOrigin,
    Save,
    Copy,
    Share(String),
    OpenUrl(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaceSheetOutcome {
    pub action: PlaceSheetAction,
    pub close_id: Option<egui::Id>,
    pub directions_id: Option<egui::Id>,
    pub save_id: Option<egui::Id>,
    pub copy_id: Option<egui::Id>,
    pub share_id: Option<egui::Id>,
    pub set_origin_id: Option<egui::Id>,
    /// Button ids for the external maps, in `place_external_urls` order.
    pub external_ids: [Option<egui::Id>; 4],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PublishSheetAction {
    None,
    Close,
    Publish,
    Gps,
    #[cfg(target_os = "android")]
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublishSheetOutcome {
    pub action: PublishSheetAction,
    pub close_id: Option<egui::Id>,
    pub publish_id: Option<egui::Id>,
    pub gps_id: Option<egui::Id>,
}
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
        let modal_open = self.crash_dialog.is_some()
            || self.update_dialog_open
            || self.share_sheet.is_some()
            || self.route_plan.open
            || self.mobile_publish_open
            || self.mobile_selected_place.is_some()
            || self.recovery_code_visible()
            || (Self::is_compact(ctx) && self.mobile_nav_open);
        let slash = ctx.input(|input| input.key_pressed(egui::Key::Slash));
        if search_shortcut_fires(slash, modal_open, ctx.wants_keyboard_input()) {
            ctx.memory_mut(|memory| memory.request_focus(egui::Id::new(SEARCH_INPUT_ID)));
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
        let mut refresh = false;

        // Place/publish sheets are compact-only; desktop keeps the map live
        // while a search result is shown in the details column.
        let map_blocked = self.crash_dialog.is_some()
            || self.update_dialog_open
            || self.share_sheet.is_some()
            || self.route_plan.open
            || self.recovery_code_visible();
        if map_shortcuts_enabled(map_blocked, ctx.wants_keyboard_input()) {
            ctx.input(|input| {
                for key in [
                    egui::Key::Plus,
                    egui::Key::Equals,
                    egui::Key::Minus,
                    egui::Key::Num0,
                    egui::Key::F,
                    egui::Key::L,
                    egui::Key::R,
                ] {
                    if !input.key_pressed(key) {
                        continue;
                    }
                    match map_key_action(key) {
                        Some(MapKeyAction::ZoomIn) => zoom_delta += 1,
                        Some(MapKeyAction::ZoomOut) => zoom_delta -= 1,
                        Some(MapKeyAction::ResetView) => reset = true,
                        Some(MapKeyAction::FitPins) => fit_pins = true,
                        Some(MapKeyAction::Locate) => locate = true,
                        Some(MapKeyAction::Refresh) => refresh = true,
                        None => {}
                    }
                }
            });
        }

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
                    if ui.button("Locate").on_hover_text("Locate me (L)").clicked() {
                        locate = true;
                    }
                    if ui
                        .add_enabled(has_pins, egui::Button::new("Fit pins"))
                        .on_hover_text("Fit pins (F)")
                        .clicked()
                    {
                        fit_pins = true;
                    }
                    if ui
                        .button("Reset view")
                        .on_hover_text("Reset view (0)")
                        .clicked()
                    {
                        reset = true;
                    }
                    if ui.button("+").on_hover_text("Zoom in (+)").clicked() {
                        zoom_delta = 1;
                    }
                    ui.label(
                        RichText::new(format!("z{zoom}"))
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                    if ui.button("−").on_hover_text("Zoom out (-)").clicked() {
                        zoom_delta = -1;
                    }
                });
            });
            ui.add_space(6.0);
            let map_height = (ui.available_height() - 34.0).max(170.0);
            let binding = self.app.data();
            let mut data = binding.lock().unwrap();
            map::render_map_with(
                ui,
                &mut data,
                map_height,
                Some(&backend_url),
                online,
                true,
                map_blocked,
            );
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
        if refresh {
            self.spawn_action(PendingAction::RefreshAll, ctx);
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
                self.route_plan.show();
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
                self.route_plan.show();
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
                    .id(egui::Id::new(SEARCH_INPUT_ID))
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
                            self.route_plan.show();
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

/// Desktop map keyboard shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapKeyAction {
    ZoomIn,
    ZoomOut,
    ResetView,
    FitPins,
    Locate,
    Refresh,
}

struct MapShortcut {
    keys: &'static [egui::Key],
    key_label: &'static str,
    action: MapKeyAction,
    action_label: &'static str,
}

const MAP_SHORTCUTS: [MapShortcut; 6] = [
    MapShortcut {
        keys: &[egui::Key::Plus, egui::Key::Equals],
        key_label: "+ / =",
        action: MapKeyAction::ZoomIn,
        action_label: "Zoom in",
    },
    MapShortcut {
        keys: &[egui::Key::Minus],
        key_label: "-",
        action: MapKeyAction::ZoomOut,
        action_label: "Zoom out",
    },
    MapShortcut {
        keys: &[egui::Key::Num0],
        key_label: "0",
        action: MapKeyAction::ResetView,
        action_label: "Reset view",
    },
    MapShortcut {
        keys: &[egui::Key::F],
        key_label: "F",
        action: MapKeyAction::FitPins,
        action_label: "Fit pins",
    },
    MapShortcut {
        keys: &[egui::Key::L],
        key_label: "L",
        action: MapKeyAction::Locate,
        action_label: "Locate me",
    },
    MapShortcut {
        keys: &[egui::Key::R],
        key_label: "R",
        action: MapKeyAction::Refresh,
        action_label: "Refresh",
    },
];

fn map_key_action(key: egui::Key) -> Option<MapKeyAction> {
    MAP_SHORTCUTS
        .iter()
        .find(|shortcut| shortcut.keys.contains(&key))
        .map(|shortcut| shortcut.action)
}

/// (keys, action) rows for the About screen, derived from the shortcut table.
pub(crate) fn map_shortcut_rows() -> impl Iterator<Item = (&'static str, &'static str)> {
    MAP_SHORTCUTS
        .iter()
        .map(|shortcut| (shortcut.key_label, shortcut.action_label))
}

/// Map shortcuts run only when no modal/sheet is open and no text field has
/// focus, so typing "0" or "F" in a form never moves the map.
fn map_shortcuts_enabled(modal_open: bool, wants_keyboard_input: bool) -> bool {
    !modal_open && !wants_keyboard_input
}

/// Stable id for the map search field in the desktop and mobile layouts.
pub(crate) const SEARCH_INPUT_ID: &str = "pinpoint_search";

/// "/" focuses search only when nothing modal is open and no field already
/// owns the keyboard.
fn search_shortcut_fires(pressed: bool, modal_open: bool, wants_keyboard_input: bool) -> bool {
    pressed && map_shortcuts_enabled(modal_open, wants_keyboard_input)
}

/// The map bottom sheet Escape should close, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscapableSheet {
    Publish,
    Place,
}

/// Consume the compact place sheet's one-shot focus request.
pub(crate) fn take_place_focus(requested: &mut bool) -> bool {
    std::mem::take(requested)
}

fn sheet_escape_target(publish_open: bool, place_open: bool) -> Option<EscapableSheet> {
    if publish_open {
        Some(EscapableSheet::Publish)
    } else if place_open {
        Some(EscapableSheet::Place)
    } else {
        None
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
        let lang = self.lang;

        let blocked = self.crash_dialog.is_some()
            || self.update_dialog_open
            || self.share_sheet.is_some()
            || self.route_plan.open
            || self.recovery_code_visible();
        let map_blocked =
            blocked || self.mobile_selected_place.is_some() || self.mobile_publish_open;
        if super::escape_dismisses(ctx.input(|i| i.key_pressed(egui::Key::Escape)), blocked) {
            match sheet_escape_target(
                self.mobile_publish_open,
                self.mobile_selected_place.is_some(),
            ) {
                Some(EscapableSheet::Publish) => self.mobile_publish_open = false,
                Some(EscapableSheet::Place) => self.mobile_selected_place = None,
                None => {}
            }
        }

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
            crate::map::render_map_with(
                ui,
                &mut data,
                height,
                Some(&backend_url),
                online,
                true,
                map_blocked,
            );
        }
        self.handle_map_click(ctx);
        self.handle_map_menu(ctx);

        let panel_solid = theme::PANEL;
        let border = theme::BORDER;

        // Floating search pill (web `.search-form`) with the account action
        // beside it. The phone map has no separate header row.
        let signed_in = self.app.is_signed_in();
        let avatar_name = self
            .app
            .data()
            .lock()
            .unwrap()
            .user
            .as_ref()
            .map(|user| user.full_name.clone());
        let search = draw_mobile_map_search(
            ctx,
            &mut self.search_query,
            width,
            top,
            signed_in,
            avatar_name.as_deref(),
            lang,
        );
        // Everything below the search pill keeps a 12 pt gap from it.
        let chips_top = search.search_bottom + 12.0;
        if search.submit && !self.search_query.trim().is_empty() {
            let query = self.search_query.clone();
            self.spawn_action(PendingAction::SearchPlaces { query }, ctx);
        }
        if search.account_clicked {
            if signed_in {
                self.mobile_nav_open = true;
            } else {
                self.app.set_screen(CurrentScreen::Login);
            }
        }
        if search.submit && !self.search_query.trim().is_empty() {
            let query = self.search_query.clone();
            self.spawn_action(PendingAction::SearchPlaces { query }, ctx);
        }

        // "Sharing with you" strip above the tab bar. It steps aside for the
        // drawer, the sheets, the route planner and the measure tool.
        let measuring = self.app.data().lock().unwrap().measure_mode;
        let strip_hidden = map_blocked || self.mobile_nav_open || self.route_plan.open || measuring;
        if !strip_hidden {
            let entries = {
                let data = self.app.data();
                let data = data.lock().unwrap();
                live_strip::strip_entries(&data.locations, chrono::Utc::now())
            };
            if let Some(target) =
                live_strip::draw_live_strip(ctx, &entries, live_strip::ABOVE_TAB_BAR)
            {
                let data = self.app.data();
                let mut data = data.lock().unwrap();
                data.map_center_lat = target.latitude;
                data.map_center_lon = target.longitude;
                data.map_zoom = target.zoom;
            }
        }

        // Category chips when there are no results (web `.category-chips`).
        let results = self.app.data().lock().unwrap().search_results.clone();
        if results.is_empty() {
            let mut chip_query: Option<&'static str> = None;
            egui::Area::new(egui::Id::new("mobile_map_chips"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(12.0, chips_top))
                .show(ctx, |ui| {
                    // Stop short of the floating controls card on the right.
                    ui.set_width(width - 76.0);
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
                                let button = egui::Button::new(RichText::new(label).size(13.0))
                                    .fill(theme::float_fill())
                                    .stroke(egui::Stroke::new(1.0_f32, border))
                                    .corner_radius(egui::CornerRadius::same(20))
                                    .min_size(egui::vec2(0.0, 40.0));
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
                .fixed_pos(egui::pos2(12.0, chips_top))
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
                self.place_focus = true;
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
                .fixed_pos(egui::pos2(width - 56.0, chips_top))
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(theme::float_fill())
                        .stroke(egui::Stroke::new(1.0_f32, border))
                        .shadow(theme::float_shadow())
                        .corner_radius(egui::CornerRadius::same(22))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                            ui.vertical(|ui| {
                                if map_icon_button(ui, MapIcon::ZoomIn, "Zoom in").clicked() {
                                    zoom_delta = 1;
                                }
                                if map_icon_button(ui, MapIcon::ZoomOut, "Zoom out").clicked() {
                                    zoom_delta = -1;
                                }
                                if map_icon_button(ui, MapIcon::Locate, "Locate me").clicked() {
                                    locate = true;
                                }
                                if map_icon_button(
                                    ui,
                                    MapIcon::Style,
                                    if map_dark {
                                        "Use light map"
                                    } else {
                                        "Use dark map"
                                    },
                                )
                                .clicked()
                                {
                                    toggle_style = true;
                                }
                                if map_icon_button(ui, MapIcon::Route, "Route").clicked() {
                                    open_route = true;
                                }
                                if crate::platform::map_webview_supported() {
                                    if map_icon_button(
                                        ui,
                                        MapIcon::WebView {
                                            shown: webview_visible,
                                        },
                                        "Offline MapLibre map",
                                    )
                                    .clicked()
                                    {
                                        toggle_webview = true;
                                    }
                                    if !webview_visible {
                                        if let Some((lat, lon)) = self.webview_map_click {
                                            if map_icon_button(
                                                ui,
                                                MapIcon::Share,
                                                &format!(
                                                    "Share WebView point ({lat:.4}, {lon:.4})"
                                                ),
                                            )
                                            .clicked()
                                            {
                                                share_webview = true;
                                            }
                                        }
                                    }
                                }
                                if map_icon_button(ui, MapIcon::Menu, "Menu").clicked() {
                                    menu = true;
                                }
                            });
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
            self.route_plan.show();
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
            let focus_close = std::mem::take(&mut self.publish_focus);
            let outcome = draw_publish_sheet_view(
                ctx,
                self.lang,
                &mut self.publish_lat,
                &mut self.publish_lon,
                width,
                focus_close,
            );
            let dispatch = publish_sheet_dispatch(
                outcome.action,
                &self.publish_lat,
                &self.publish_lon,
                crate::platform::ensure_location_permission,
            );
            if let Some(pending) = dispatch.pending {
                self.spawn_action(pending, ctx);
            }
            if dispatch.close {
                self.mobile_publish_open = false;
            }
            if let Some(error) = dispatch.error {
                self.app.set_error(Some(error));
            }
            if let Some(notice) = dispatch.notice {
                self.app.set_notice(Some(notice));
            }
            #[cfg(target_os = "android")]
            if dispatch.settings {
                if let Err(error) = crate::platform::open_app_settings() {
                    self.app
                        .set_error(Some(format!("Could not open app settings: {error}")));
                }
            }
        } else if let Some(place) = self.mobile_selected_place.clone() {
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
            let focus_close = take_place_focus(&mut self.place_focus);
            let outcome = draw_place_sheet_view(ctx, &place, distance_m, width, focus_close);
            let store = crate::bookmarks::BookmarkStore::default_location();
            let dispatch = place_sheet_dispatch(outcome.action, &place, &store);
            if dispatch.close || dispatch.directions || dispatch.set_origin {
                self.mobile_selected_place = None;
                self.place_focus = false;
                self.app.data().lock().unwrap().selected_place = None;
            }
            if dispatch.save_bookmark.is_some() {
                self.app.refresh_bookmarks();
            }
            if let Some(text) = dispatch.copy_text {
                ctx.copy_text(text);
            }
            if let Some(url) = dispatch.share_text {
                self.share_text(ctx, &url);
            }
            if dispatch.directions {
                self.route_plan.open_with_destination(&place);
            }
            if dispatch.set_origin {
                self.route_plan.open_with_origin(&place);
            }
            if let Some(url) = dispatch.open_url {
                let _ = webbrowser::open(&url);
            }
            if let Some(notice) = dispatch.notice {
                self.app.set_notice(Some(notice));
            }
        }
        self.draw_route_sheet(ui, ctx, true, top);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct MobileMapSearchOutcome {
    pub submit: bool,
    pub account_clicked: bool,
    pub search_id: Option<egui::Id>,
    pub account_id: Option<egui::Id>,
    /// Bottom edge of the search pill, so the rows below sit a fixed gap under it.
    pub search_bottom: f32,
    /// Vertical centre of the search pill, so the account button lines up with it.
    pub search_centre_y: f32,
}

/// Floating phone-map search pill and its account button. Kept as a free view
/// so the real controls can be clicked in a headless egui test.
pub(crate) fn draw_mobile_map_search(
    ctx: &egui::Context,
    query: &mut String,
    width: f32,
    top: f32,
    signed_in: bool,
    avatar_name: Option<&str>,
    lang: crate::i18n::Lang,
) -> MobileMapSearchOutcome {
    let account_width = if signed_in { 44.0 } else { 92.0 };
    let search_width = (width - 24.0 - account_width - 8.0).max(140.0);
    let mut outcome = MobileMapSearchOutcome::default();
    let search = egui::Area::new(egui::Id::new("mobile_map_search"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(12.0, top + 10.0))
        .show(ctx, |ui| {
            ui.set_width(search_width);
            egui::Frame::new()
                .fill(theme::float_fill())
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                .shadow(theme::float_shadow())
                .corner_radius(egui::CornerRadius::same(26))
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let edit = ui.add(
                            egui::TextEdit::singleline(query)
                                .id(egui::Id::new(SEARCH_INPUT_ID))
                                .hint_text("Search places…")
                                .font(egui::FontId::proportional(15.0))
                                .frame(false)
                                .char_limit(120)
                                .desired_width((ui.available_width() - 52.0).max(100.0)),
                        );
                        let search_button = map_icon_cell(ui, MapIcon::Search, "Search", false);
                        outcome.search_id = Some(search_button.id);
                        outcome.submit = search_button.clicked()
                            || (edit.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                    });
                });
        });
    let search_rect = search.response.rect;
    outcome.search_bottom = search_rect.bottom();
    outcome.search_centre_y = search_rect.center().y;
    egui::Area::new(egui::Id::new("mobile_map_account"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(
            width - account_width - 12.0,
            search_rect.center().y - 22.0,
        ))
        .show(ctx, |ui| {
            ui.set_width(account_width);
            let response = if signed_in {
                super::phone_tabs::avatar_button(ui, avatar_name)
            } else {
                let button = egui::Button::new(
                    RichText::new(i18n::t(lang, key::SIGN_IN))
                        .strong()
                        .color(theme::ACCENT_INK),
                )
                .fill(theme::ACCENT)
                .min_size(egui::vec2(account_width, 44.0))
                .corner_radius(egui::CornerRadius::same(22));
                ui.add(button)
            };
            outcome.account_id = Some(response.id);
            outcome.account_clicked = response.clicked();
        });
    outcome
}

pub(crate) fn draw_publish_sheet_view(
    ctx: &egui::Context,
    lang: crate::i18n::Lang,
    lat: &mut String,
    lon: &mut String,
    width: f32,
    focus_close: bool,
) -> PublishSheetOutcome {
    let mut action: Option<PublishSheetAction> = None;
    let mut close_id = None;
    let mut publish_id = None;
    let mut gps_id = None;
    egui::Area::new(egui::Id::new("mobile_publish_sheet"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0))
        .show(ctx, |ui| {
            ui.set_width(width - 24.0);
            egui::Frame::new()
                .fill(theme::PANEL)
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(i18n::t(lang, key::PUBLISH_MY_LOCATION))
                                .size(16.0)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let close_button = ui.button("×");
                            if close_button.clicked() {
                                action = Some(PublishSheetAction::Close);
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
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(lat)
                                .id(egui::Id::new("publish_lat"))
                                .hint_text(i18n::t(lang, key::LATITUDE))
                                .desired_width(120.0),
                        );
                        ui.add(
                            egui::TextEdit::singleline(lon)
                                .id(egui::Id::new("publish_lon"))
                                .hint_text(i18n::t(lang, key::LONGITUDE))
                                .desired_width(120.0),
                        );
                    });
                    ui.horizontal_wrapped(|ui| {
                        let publish_button = ui.button(i18n::t(lang, key::PUBLISH));
                        if publish_button.clicked() {
                            action = Some(PublishSheetAction::Publish);
                        }
                        publish_id = Some(publish_button.id);
                        let gps_button = ui.button("◎ Use GPS");
                        if gps_button.clicked() {
                            action = Some(PublishSheetAction::Gps);
                        }
                        gps_id = Some(gps_button.id);
                        #[cfg(target_os = "android")]
                        if ui.button("Settings").clicked() {
                            action = Some(PublishSheetAction::Settings);
                        }
                    });
                });
        });
    PublishSheetOutcome {
        action: action.unwrap_or(PublishSheetAction::None),
        close_id,
        publish_id,
        gps_id,
    }
}

pub(crate) fn draw_place_sheet_view(
    ctx: &egui::Context,
    place: &crate::osm::SearchPlace,
    distance_m: f64,
    width: f32,
    focus_close: bool,
) -> PlaceSheetOutcome {
    let mut action: Option<PlaceSheetAction> = None;
    let mut close_id = None;
    let mut directions_id = None;
    let mut save_id = None;
    let mut copy_id = None;
    let mut share_id = None;
    let mut set_origin_id = None;
    let mut external_ids: [Option<egui::Id>; 4] = [None; 4];
    egui::Area::new(egui::Id::new("mobile_place_sheet"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0))
        .show(ctx, |ui| {
            ui.set_width(width - 24.0);
            egui::Frame::new()
                .fill(theme::PANEL)
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::same(12))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&place.short_name).size(18.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let close_button = ui.button("×");
                            if close_button.clicked() {
                                action = Some(PlaceSheetAction::Close);
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
                    ui.label(
                        RichText::new(&place.display_name)
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                    ui.label(
                        RichText::new(format!(
                            "{} from map centre",
                            crate::ui::route::format_distance(distance_m)
                        ))
                        .size(11.0)
                        .color(theme::MUTED),
                    );
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        let directions_button = ui.button("Directions");
                        if directions_button.clicked() {
                            action = Some(PlaceSheetAction::Directions);
                        }
                        directions_id = Some(directions_button.id);
                        let origin_button = ui.button("Set as origin");
                        if origin_button.clicked() {
                            action = Some(PlaceSheetAction::SetOrigin);
                        }
                        set_origin_id = Some(origin_button.id);
                        let save_button = ui.button("Save");
                        if save_button.clicked() {
                            action = Some(PlaceSheetAction::Save);
                        }
                        save_id = Some(save_button.id);
                        let copy_button = ui.button("Copy");
                        if copy_button.clicked() {
                            action = Some(PlaceSheetAction::Copy);
                        }
                        copy_id = Some(copy_button.id);
                        let share_button = ui.button("Share");
                        if share_button.clicked() {
                            action = Some(PlaceSheetAction::Share(place_share_url(place)));
                        }
                        share_id = Some(share_button.id);
                        for (index, (label, url)) in
                            place_external_urls(place).into_iter().enumerate()
                        {
                            let external_button = ui.button(label);
                            if external_button.clicked() {
                                action = Some(PlaceSheetAction::OpenUrl(url));
                            }
                            external_ids[index] = Some(external_button.id);
                        }
                    });
                });
        });
    PlaceSheetOutcome {
        action: action.unwrap_or(PlaceSheetAction::None),
        close_id,
        directions_id,
        save_id,
        copy_id,
        share_id,
        set_origin_id,
        external_ids,
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
        let dispatch = publish_sheet_dispatch(
            PublishSheetAction::Gps,
            "",
            "",
            crate::platform::ensure_location_permission,
        );
        if let Some(pending) = dispatch.pending {
            self.spawn_action(pending, ctx);
        }
        if let Some(notice) = dispatch.notice {
            self.app.set_notice(Some(notice));
        }
        if let Some(error) = dispatch.error {
            self.app.set_error(Some(error));
        }
    }
}

/// Floating map control: solid panel fill so the glyph stays readable over
/// the light basemap.
/// The phone map's floating control icons. Each one is painted with line
/// strokes, so the controls need no icon font. The action behind each button
/// is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MapIcon {
    ZoomIn,
    ZoomOut,
    Locate,
    Style,
    Route,
    Search,
    WebView { shown: bool },
    Share,
    Menu,
}

/// A 44 pt control in the floating map card: a painted icon, a hairline
/// divider under it, and a hover tint that fades in rather than snapping.
fn map_icon_button(ui: &mut egui::Ui, icon: MapIcon, tip: &str) -> egui::Response {
    map_icon_cell(ui, icon, tip, true)
}

/// A 44 pt control. `divider` draws the hairline under it, which belongs to
/// the map card; a control on its own (the search pill's) leaves it out.
fn map_icon_cell(ui: &mut egui::Ui, icon: MapIcon, tip: &str, divider: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::click());
    let hover =
        ui.ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
    let painter = ui.painter();
    if hover > 0.0 {
        let alpha = (255.0 * hover).round() as u8;
        let tint = egui::Color32::from_rgba_unmultiplied(
            theme::PANEL_2.r(),
            theme::PANEL_2.g(),
            theme::PANEL_2.b(),
            alpha,
        );
        painter.rect_filled(rect.shrink(4.0), egui::CornerRadius::same(12), tint);
    }
    if divider {
        painter.hline(
            egui::Rangef::new(rect.left() + 8.0, rect.right() - 8.0),
            rect.bottom(),
            egui::Stroke::new(1.0_f32, theme::BORDER),
        );
    }
    paint_map_icon(painter, icon, rect.center(), theme::TEXT);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    response.on_hover_text(tip)
}

fn paint_map_icon(painter: &egui::Painter, icon: MapIcon, center: egui::Pos2, ink: egui::Color32) {
    let stroke = egui::Stroke::new(1.6_f32, ink);
    let at = |x: f32, y: f32| center + egui::vec2(x, y);
    match icon {
        MapIcon::ZoomIn => {
            painter.line_segment([at(-6.0, 0.0), at(6.0, 0.0)], stroke);
            painter.line_segment([at(0.0, -6.0), at(0.0, 6.0)], stroke);
        }
        MapIcon::ZoomOut => {
            painter.line_segment([at(-6.0, 0.0), at(6.0, 0.0)], stroke);
        }
        MapIcon::Locate => {
            painter.circle_stroke(center, 7.0, stroke);
            painter.circle_filled(center, 2.0, ink);
            for (dx, dy) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
                painter.line_segment([at(dx * 7.0, dy * 7.0), at(dx * 10.5, dy * 10.5)], stroke);
            }
        }
        MapIcon::Style => {
            painter.circle_stroke(center, 7.0, stroke);
            painter.circle_filled(center, 3.5, ink);
        }
        MapIcon::Route => {
            painter.line_segment([at(-6.0, 6.0), at(4.0, -4.0)], stroke);
            painter.line_segment([at(4.0, -4.0), at(-1.0, -4.0)], stroke);
            painter.line_segment([at(4.0, -4.0), at(4.0, 1.0)], stroke);
        }
        MapIcon::Search => {
            painter.circle_stroke(at(-1.0, -1.0), 5.5, stroke);
            painter.line_segment([at(3.0, 3.0), at(7.5, 7.5)], stroke);
        }
        MapIcon::WebView { shown } => {
            let square = egui::Rect::from_center_size(center, egui::vec2(13.0, 13.0));
            let rounding = egui::CornerRadius::same(2);
            if shown {
                painter.rect_filled(square, rounding, ink);
            } else {
                painter.rect_stroke(square, rounding, stroke, egui::StrokeKind::Inside);
            }
        }
        MapIcon::Share => {
            painter.circle_stroke(center, 6.0, stroke);
            for (dx, dy) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
                painter.line_segment([at(dx * 4.0, dy * 4.0), at(dx * 9.0, dy * 9.0)], stroke);
            }
        }
        MapIcon::Menu => {
            for dy in [-5.0, 0.0, 5.0] {
                painter.line_segment([at(-6.0, dy), at(6.0, dy)], stroke);
            }
        }
    }
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

/// Clipboard text used by the place sheet's Copy action.
pub(crate) fn place_copy_text(place: &crate::osm::SearchPlace) -> String {
    format!(
        "{} ({:.5}, {:.5})",
        place.short_name, place.latitude, place.longitude
    )
}

/// Share URL used by the place sheet's Share action.
pub(crate) fn place_share_url(place: &crate::osm::SearchPlace) -> String {
    format!(
        "geo:{:.6},{:.6}?q={}",
        place.latitude,
        place.longitude,
        encode_component(&place.short_name)
    )
}

/// Save a place through the given bookmark store.
pub(crate) fn save_place_bookmark(
    store: &crate::bookmarks::BookmarkStore,
    place: &crate::osm::SearchPlace,
) -> crate::bookmarks::Bookmark {
    store.add(&place.short_name, place.latitude, place.longitude)
}

/// Manual publish coordinates with the accuracy the app sends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PublishFix {
    pub lat: f64,
    pub lon: f64,
    pub accuracy: f64,
}

/// Validate the publish sheet's manual coordinates.
pub(crate) fn parse_publish_fix(lat: &str, lon: &str) -> Option<PublishFix> {
    match (lat.trim().parse::<f64>(), lon.trim().parse::<f64>()) {
        (Ok(lat), Ok(lon)) if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) => {
            Some(PublishFix {
                lat,
                lon,
                accuracy: 10.0,
            })
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GpsPublishStep {
    Request,
    Waiting,
    Unavailable,
}

/// Decide what Use GPS does for the current permission state.
pub(crate) fn gps_publish_step(permission: crate::platform::LocationPermission) -> GpsPublishStep {
    match permission {
        crate::platform::LocationPermission::Granted => GpsPublishStep::Request,
        crate::platform::LocationPermission::Requested => GpsPublishStep::Waiting,
        crate::platform::LocationPermission::Unsupported => GpsPublishStep::Unavailable,
    }
}

/// External map links offered by the place sheet.
pub(crate) fn place_external_urls(place: &crate::osm::SearchPlace) -> [(&'static str, String); 4] {
    let name = encode_component(&place.short_name);
    [
        (
            "OSM",
            format!(
                "https://www.openstreetmap.org/?mlat={:.4}&mlon={:.4}#map=15/{:.4}/{:.4}",
                place.latitude, place.longitude, place.latitude, place.longitude
            ),
        ),
        (
            "Google",
            format!(
                "https://www.google.com/maps/search/?api=1&query={:.4},{:.4}",
                place.latitude, place.longitude
            ),
        ),
        (
            "Organic",
            format!(
                "https://omaps.app/map?v=1&ll={:.4},{:.4}&n={name}",
                place.latitude, place.longitude
            ),
        ),
        (
            "Magic Earth",
            format!(
                "magicearth://?show_on_map&lat={:.4}&lon={:.4}&name={name}",
                place.latitude, place.longitude
            ),
        ),
    ]
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct PublishDispatch {
    pub pending: Option<super::PendingAction>,
    pub notice: Option<String>,
    pub error: Option<String>,
    pub close: bool,
    #[cfg(target_os = "android")]
    pub settings: bool,
}

/// Map a publish-sheet action to the pending request and notices the app shows.
pub(crate) fn publish_sheet_dispatch(
    action: PublishSheetAction,
    lat: &str,
    lon: &str,
    permission: impl FnOnce() -> crate::platform::LocationPermission,
) -> PublishDispatch {
    let mut dispatch = PublishDispatch::default();
    match action {
        PublishSheetAction::None => {}
        PublishSheetAction::Close => dispatch.close = true,
        PublishSheetAction::Publish => match parse_publish_fix(lat, lon) {
            Some(fix) => {
                dispatch.pending = Some(super::PendingAction::PublishLocation {
                    lat: fix.lat,
                    lon: fix.lon,
                    accuracy: fix.accuracy,
                });
                dispatch.close = true;
            }
            None => {
                dispatch.error =
                    Some("Enter a valid latitude (-90..90) and longitude (-180..180)".to_string())
            }
        },
        PublishSheetAction::Gps => match gps_publish_step(permission()) {
            GpsPublishStep::Request => {
                dispatch.pending = Some(super::PendingAction::PublishGpsLocation)
            }
            GpsPublishStep::Waiting => {
                dispatch.notice = Some(
                    "Location permission requested. Allow it, then tap Use GPS again. \
                     If no dialog appears, open Settings → Permissions."
                        .to_string(),
                )
            }
            GpsPublishStep::Unavailable => {
                dispatch.error = Some("Location is unavailable on this device.".to_string())
            }
        },
        #[cfg(target_os = "android")]
        PublishSheetAction::Settings => dispatch.settings = true,
    }
    dispatch
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct PlaceDispatch {
    pub close: bool,
    pub save_bookmark: Option<crate::bookmarks::Bookmark>,
    pub copy_text: Option<String>,
    pub share_text: Option<String>,
    pub set_origin: bool,
    pub directions: bool,
    pub open_url: Option<String>,
    pub notice: Option<String>,
}

/// Map a place-sheet action to its bookmark, clipboard, share and route effects.
pub(crate) fn place_sheet_dispatch(
    action: PlaceSheetAction,
    place: &crate::osm::SearchPlace,
    store: &crate::bookmarks::BookmarkStore,
) -> PlaceDispatch {
    let mut dispatch = PlaceDispatch::default();
    match action {
        PlaceSheetAction::None => {}
        PlaceSheetAction::Close => dispatch.close = true,
        PlaceSheetAction::Save => {
            let bookmark = save_place_bookmark(store, place);
            dispatch.notice = Some(format!("Saved {}", bookmark.name));
            dispatch.save_bookmark = Some(bookmark);
        }
        PlaceSheetAction::Copy => {
            dispatch.copy_text = Some(place_copy_text(place));
            dispatch.notice = Some("Place copied.".to_string());
        }
        PlaceSheetAction::Share(url) => dispatch.share_text = Some(url),
        PlaceSheetAction::Directions => dispatch.directions = true,
        PlaceSheetAction::SetOrigin => dispatch.set_origin = true,
        PlaceSheetAction::OpenUrl(url) => dispatch.open_url = Some(url),
    }
    dispatch
}

#[cfg(test)]
mod tests {
    use super::egui;
    use super::{
        map_key_action, map_shortcut_rows, map_shortcuts_enabled, search_shortcut_fires,
        sheet_escape_target, take_place_focus, EscapableSheet, MapKeyAction, MAP_SHORTCUTS,
    };

    #[test]
    fn map_shortcuts_need_no_modal_and_no_text_focus() {
        assert!(map_shortcuts_enabled(false, false));
        assert!(!map_shortcuts_enabled(true, false));
        assert!(!map_shortcuts_enabled(false, true));
        assert!(!map_shortcuts_enabled(true, true));
    }

    #[test]
    fn slash_focuses_search_only_without_a_modal_or_typing() {
        assert!(search_shortcut_fires(true, false, false));
        assert!(!search_shortcut_fires(false, false, false));
        assert!(!search_shortcut_fires(true, true, false));
        assert!(!search_shortcut_fires(true, false, true));
    }

    #[test]
    fn map_shortcuts_map_to_actions() {
        assert_eq!(map_key_action(egui::Key::Plus), Some(MapKeyAction::ZoomIn));
        assert_eq!(
            map_key_action(egui::Key::Equals),
            Some(MapKeyAction::ZoomIn)
        );
        assert_eq!(
            map_key_action(egui::Key::Minus),
            Some(MapKeyAction::ZoomOut)
        );
        assert_eq!(
            map_key_action(egui::Key::Num0),
            Some(MapKeyAction::ResetView)
        );
        assert_eq!(map_key_action(egui::Key::F), Some(MapKeyAction::FitPins));
        assert_eq!(map_key_action(egui::Key::L), Some(MapKeyAction::Locate));
        assert_eq!(map_key_action(egui::Key::R), Some(MapKeyAction::Refresh));
        assert_eq!(map_key_action(egui::Key::A), None);
    }

    #[test]
    fn about_rows_match_the_shortcut_table() {
        assert_eq!(
            map_shortcut_rows().collect::<Vec<_>>(),
            vec![
                ("+ / =", "Zoom in"),
                ("-", "Zoom out"),
                ("0", "Reset view"),
                ("F", "Fit pins"),
                ("L", "Locate me"),
                ("R", "Refresh"),
            ]
        );
        for shortcut in MAP_SHORTCUTS.iter() {
            for key in shortcut.keys {
                assert_eq!(map_key_action(*key), Some(shortcut.action));
            }
        }
    }

    #[test]
    fn escape_prefers_publish_then_place() {
        assert_eq!(
            sheet_escape_target(true, false),
            Some(EscapableSheet::Publish)
        );
        assert_eq!(
            sheet_escape_target(false, true),
            Some(EscapableSheet::Place)
        );
        assert_eq!(
            sheet_escape_target(true, true),
            Some(EscapableSheet::Publish)
        );
        assert_eq!(sheet_escape_target(false, false), None);
    }

    #[test]
    fn place_sheet_focus_is_requested_once() {
        let mut requested = true;
        assert!(take_place_focus(&mut requested));
        assert!(!take_place_focus(&mut requested));
        let mut idle = false;
        assert!(!take_place_focus(&mut idle));
    }
}
