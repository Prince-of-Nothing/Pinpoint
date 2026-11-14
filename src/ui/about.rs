//! About & updates screen.
//!
//! Visual language follows the ArtCraft apps: near-black canvas, large
//! headings, rounded surface cards and a single accent action. The update
//! check is user-initiated and reads GitHub's public releases feed.
use super::{PendingAction, PinpointAppUI};
use crate::app::CurrentScreen;
use crate::update::{self, UpdateStatus};
use eframe::egui::{self, Color32, RichText};

impl PinpointAppUI {
    pub fn draw_about_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.draw_main_toolbar(ui);
        let compact = Self::is_compact(ctx);
        let bg = Color32::from_rgb(8, 12, 24);
        let panel = Color32::from_rgb(17, 24, 39);
        let border = Color32::from_rgb(31, 41, 55);
        let muted = Color32::from_rgb(148, 163, 184);
        let accent = Color32::from_rgb(52, 211, 153);
        let warn = Color32::from_rgb(250, 204, 21);
        let danger = Color32::from_rgb(248, 113, 113);

        ui.painter().rect_filled(ui.max_rect(), 0.0, bg);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical(|ui| {
                ui.add_space(if compact { 8.0 } else { 16.0 });
            ui.horizontal(|ui| {
                if ui.button("< Back").clicked() {
                    self.app.set_screen(CurrentScreen::Profile);
                }
                ui.add_space(6.0);
                ui.label(RichText::new("About Pinpoint").size(24.0).strong());
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "Version {} · {}",
                    update::current_version(),
                    crate::platform::build_identity()
                ))
                .size(12.0)
                .color(muted),
            );
            ui.add_space(12.0);

            let card = |ui: &mut egui::Ui, add: &mut dyn FnMut(&mut egui::Ui)| {
                egui::Frame::new()
                    .fill(panel)
                    .stroke(egui::Stroke::new(1.0_f32, border))
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| add(ui));
            };

            // Privacy: opening About must not contact GitHub. The release
            // check runs only when the user presses the button below.
            let status = self.app.data().lock().unwrap().update_status.clone();
            card(ui, &mut |ui| {
                ui.label(RichText::new("Updates").size(17.0).strong());
                ui.add_space(4.0);
                match &status {
                    UpdateStatus::Unknown => {
                        ui.label("Check GitHub releases to see whether a newer version exists.");
                    }
                    UpdateStatus::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Checking github.com for the latest release…");
                        });
                    }
                    UpdateStatus::UpToDate { current } => {
                        ui.colored_label(
                            accent,
                            RichText::new(format!("You are on the latest version ({current})."))
                                .strong(),
                        );
                    }
                    UpdateStatus::Available { current, latest } => {
                        ui.colored_label(
                            warn,
                            RichText::new(format!(
                                "Update available: {} (you have {current}).",
                                latest.tag
                            ))
                            .size(15.0)
                            .strong(),
                        );
                        if !latest.title.is_empty() {
                            ui.label(RichText::new(&latest.title).size(12.0).color(muted));
                        }
                        if !latest.notes.is_empty() {
                            ui.add_space(4.0);
                            ui.label(RichText::new(&latest.notes).size(11.0).color(muted));
                        }
                        ui.add_space(4.0);
                        if ui.button("Open release page").clicked() {
                            let _ = webbrowser::open(&latest.url);
                        }
                    }
                    UpdateStatus::NoReleases => {
                        ui.colored_label(
                            muted,
                            "No GitHub releases are published yet, so there is nothing to compare.",
                        );
                        ui.label(
                            RichText::new("Publish a tagged release on GitHub to enable updates.")
                                .size(11.0)
                                .color(muted),
                        );
                    }
                    UpdateStatus::Error(error) => {
                        ui.colored_label(danger, format!("Could not check for updates: {error}"));
                    }
                }
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !matches!(&status, UpdateStatus::Checking),
                            egui::Button::new("Check for updates"),
                        )
                        .clicked()
                    {
                        self.spawn_action(PendingAction::CheckForUpdates, ctx);
                    }
                    if ui.button("Open releases").clicked() {
                        let _ = webbrowser::open(update::RELEASES_URL);
                    }
                });
                ui.label(
                    RichText::new(
                        "The check contacts github.com only when you press the button. No account, \
                         location or friend data is sent.",
                    )
                    .size(10.0)
                    .color(muted),
                );
            });

            ui.add_space(10.0);
            card(ui, &mut |ui| {
                ui.label(RichText::new("Project").size(17.0).strong());
                ui.add_space(4.0);
                ui.label(
                    "Pinpoint is a privacy-first location-sharing demo: latest-only locations, \
                          explicit revocable consent, no trackers and no third-party scripts.",
                );
                ui.add_space(4.0);
                if ui.button("GitHub repository").clicked() {
                    let _ = webbrowser::open("https://github.com/Prince-of-Nothing/Pinpoint");
                }
                if ui.button("OpenStreetMap copyright").clicked() {
                    let _ = webbrowser::open(crate::map::OSM_COPYRIGHT_URL);
                }
            });

            let pending = crate::crash::pending_report();
            let has_pending = pending.is_some();
            let pending_for_button = pending;
            let device = crate::crash::device_summary();
            let log_path = crate::logging::log_path();

            ui.add_space(10.0);
            card(ui, &mut |ui| {
                ui.label(RichText::new("Troubleshooting").size(17.0).strong());
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Crash reports and logs stay on this device until you choose to email them.",
                    )
                    .size(11.0)
                    .color(muted),
                );
                ui.add_space(4.0);
                if ui.button("Report a problem").clicked() {
                    self.open_manual_report();
                }
                if has_pending && ui.button("View last crash report").clicked() {
                    if let Some(stored) = pending_for_button.clone() {
                        self.crash_dialog =
                            Some(crate::ui::crash_dialog::CrashDialog::from_pending(stored));
                    }
                }
                if ui.button("Copy recent log").clicked() {
                    if let Err(error) = crate::ui::crash_dialog::copy_to_clipboard(
                        ctx,
                        &crate::logging::tail(200),
                    ) {
                        self.app
                            .set_error(Some(format!("Could not copy the log: {error}")));
                    }
                }
                if ui.button("Clear log").clicked() {
                    crate::logging::clear();
                }
                ui.add_space(4.0);
                ui.label(RichText::new(device.as_str()).size(10.0).color(muted));
                if let Some(path) = &log_path {
                    ui.label(
                        RichText::new(format!("Log: {}", path.display()))
                            .size(10.0)
                            .color(muted),
                    );
                }
            });

            ui.add_space(10.0);
            ui.label(
                RichText::new("Map data © OpenStreetMap contributors (ODbL)")
                    .size(10.0)
                    .color(muted),
            );
            });
        });
    }
}
