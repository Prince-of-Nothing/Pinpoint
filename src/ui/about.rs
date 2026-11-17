//! About & updates screen.
//!
//! Visual language follows the ArtCraft apps: near-black canvas, large
//! headings, rounded surface cards and a single accent action. The update
//! check runs at most once a day at launch (opt-out) or on demand and reads
//! GitHub's public releases feed.
use super::theme;
use super::PinpointAppUI;
use crate::app::CurrentScreen;
use crate::update::{self, UpdateStatus};
use eframe::egui::{self, RichText};

impl PinpointAppUI {
    pub fn draw_about_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let compact = Self::is_compact(ctx);

        ui.painter().rect_filled(ui.max_rect(), 0.0, theme::BG);
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
                .color(theme::MUTED),
            );
            ui.add_space(12.0);

            // Privacy: opening About itself never contacts GitHub. The
            // throttled launch check may already have run; the button below
            // forces a fresh check.
            let status = self.app.data().lock().unwrap().update_status.clone();
            theme::card(ui, |ui| {
                ui.label(RichText::new("Updates").size(17.0).strong());
                ui.add_space(4.0);
                match &status {
                    UpdateStatus::Unknown => {
                        ui.label(
                            "Pinpoint checks GitHub once a day at launch. You can also check now.",
                        );
                    }
                    UpdateStatus::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Checking github.com for the latest release…");
                        });
                    }
                    UpdateStatus::UpToDate { current } => {
                        ui.colored_label(
                            theme::SUCCESS,
                            RichText::new(format!("You are on the latest version ({current})."))
                                .strong(),
                        );
                    }
                    UpdateStatus::Available { current, latest } => {
                        ui.colored_label(
                            theme::ACCENT,
                            RichText::new(format!(
                                "Update available: {} (you have {current}).",
                                latest.tag
                            ))
                            .size(15.0)
                            .strong(),
                        );
                        if !latest.title.is_empty() {
                            ui.label(RichText::new(&latest.title).size(12.0).color(theme::MUTED));
                        }
                        if !latest.notes.is_empty() {
                            ui.add_space(4.0);
                            ui.label(RichText::new("What's new").size(12.0).strong());
                            egui::ScrollArea::vertical()
                                .id_salt("about_release_notes")
                                .max_height(220.0)
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(update::notes_plain(&latest.notes))
                                            .size(11.0)
                                            .color(theme::MUTED),
                                    );
                                });
                        }
                        ui.add_space(6.0);
                        if cfg!(target_os = "android") {
                            if let Some(download) = latest.download_url() {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("Download APK")
                                                .strong()
                                                .color(theme::ACCENT_INK),
                                        )
                                        .fill(theme::ACCENT),
                                    )
                                    .clicked()
                                {
                                    let _ = webbrowser::open(download);
                                }
                                if let Some(name) = latest.apk_name() {
                                    let size = latest
                                        .assets
                                        .iter()
                                        .find(|asset| asset.name == name)
                                        .map(|asset| asset.size)
                                        .unwrap_or(0);
                                    ui.label(
                                        RichText::new(format!(
                                            "{name} · {:.1} MB · direct download",
                                            size as f64 / 1_048_576.0
                                        ))
                                        .size(10.0)
                                        .color(theme::MUTED),
                                    );
                                }
                                if ui.button("Copy download link").clicked() {
                                    ctx.copy_text(download.to_string());
                                }
                            }
                        }
                        if ui.button("Open release page").clicked() {
                            let _ = webbrowser::open(&latest.url);
                        }
                    }
                    UpdateStatus::NoReleases => {
                        ui.colored_label(
                            theme::MUTED,
                            "No GitHub releases are published yet, so there is nothing to compare.",
                        );
                        ui.label(
                            RichText::new("Publish a tagged release on GitHub to enable updates.")
                                .size(11.0)
                                .color(theme::MUTED),
                        );
                    }
                    UpdateStatus::Error(error) => {
                        ui.colored_label(theme::DANGER, format!("Could not check for updates: {error}"));
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
                        self.request_update_check(ctx);
                    }
                    if ui.button("Open releases").clicked() {
                        let _ = webbrowser::open(update::RELEASES_URL);
                    }
                });
                ui.label(
                    RichText::new(
                        "The check fetches GitHub's public releases feed at most once a day at \
                         launch; turn it off in Profile & settings. No account, location or friend \
                         data is sent.",
                    )
                    .size(10.0)
                    .color(theme::MUTED),
                );
            });

            ui.add_space(10.0);
            theme::card(ui, |ui| {
                ui.label(RichText::new("Project").size(17.0).strong());
                ui.add_space(4.0);
                ui.label(
                    "Pinpoint is a privacy-first location-sharing app: latest-only locations, \
                          explicit revocable consent, no trackers and no third-party scripts.",
                );
                ui.add_space(4.0);
                if ui.button("Privacy policy").clicked() {
                    let url = format!(
                        "{}/privacy",
                        self.app.backend_url().trim_end_matches('/')
                    );
                    let _ = webbrowser::open(&url);
                }
                if ui.button("GitHub repository").clicked() {
                    let _ = webbrowser::open("https://github.com/Prince-of-Nothing/Pinpoint");
                }
                if ui.button("OpenStreetMap copyright").clicked() {
                    let _ = webbrowser::open(crate::map::OSM_COPYRIGHT_URL);
                }
            });

            let pending = crate::crash::pending_report();
            let pending_count = crate::crash::pending_report_count();
            let has_pending = pending.is_some();
            let pending_for_button = pending;
            let device = crate::crash::device_summary();
            let backend = redacted_backend_url(&self.app.backend_url());
            let log_path = crate::logging::log_path();

            ui.add_space(10.0);
            theme::card(ui, |ui| {
                ui.label(RichText::new("Troubleshooting").size(17.0).strong());
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Crash reports and logs stay on this device until you choose to email them.",
                    )
                    .size(11.0)
                    .color(theme::MUTED),
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
                #[cfg(target_os = "android")]
                ui.label(
                    RichText::new(
                        "Pinpoint is not registered as a text-share target until an Android \
                         onNewIntent bridge can handle repeat shares.",
                    )
                    .size(10.0)
                    .color(theme::MUTED),
                );
                if ui.button("Clear log").clicked() {
                    crate::logging::clear();
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("Backend: {backend}"))
                        .size(10.0)
                        .color(theme::MUTED),
                );
                ui.label(
                    RichText::new(format!(
                        "Pending crash reports: {pending_count} (seen reports are kept but not counted)"
                    ))
                    .size(10.0)
                    .color(theme::MUTED),
                );
                ui.label(RichText::new(device.as_str()).size(10.0).color(theme::MUTED));
                if let Some(path) = &log_path {
                    ui.label(
                        RichText::new(format!("Log: {}", path.display()))
                            .size(10.0)
                            .color(theme::MUTED),
                    );
                }
            });

            ui.add_space(10.0);
            ui.label(
                RichText::new("Map data © OpenStreetMap contributors (ODbL)")
                    .size(10.0)
                    .color(theme::MUTED),
            );
            });
        });
    }
}

/// Hide any userinfo in a backend URL before showing it in diagnostics.
fn redacted_backend_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let rest = &url[scheme_end + 3..];
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    match authority.rfind('@') {
        Some(at) => format!(
            "{}://{}{}",
            &url[..scheme_end],
            &authority[at + 1..],
            &rest[authority_end..]
        ),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::redacted_backend_url;

    #[test]
    fn backend_url_userinfo_is_redacted() {
        assert_eq!(
            redacted_backend_url("http://user:pass@localhost:8080/api"),
            "http://localhost:8080/api"
        );
        assert_eq!(
            redacted_backend_url("https://example.com"),
            "https://example.com"
        );
        assert_eq!(
            redacted_backend_url("http://127.0.0.1:8000"),
            "http://127.0.0.1:8000"
        );
    }
}
