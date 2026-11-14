//! Crash-report dialog and manual report action.
//!
//! The report stays local until the user chooses to open their email app.
//! Nothing is uploaded by the app.

use super::PinpointAppUI;
use crate::crash::{self, StoredReport};
use crate::platform;
use eframe::egui::{self, Align2, Color32};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CrashSource {
    Startup,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrashDialogOutcome {
    Discard,
    Close,
}

pub(crate) struct CrashDialog {
    pub report: String,
    pub path: Option<PathBuf>,
    pub copied: bool,
    pub error: Option<String>,
    pub source: CrashSource,
}

impl CrashDialog {
    pub fn from_pending(stored: StoredReport) -> Self {
        Self {
            report: stored.text,
            path: Some(stored.path),
            copied: false,
            error: None,
            source: CrashSource::Startup,
        }
    }

    pub fn manual(report: String) -> Self {
        Self {
            report,
            path: None,
            copied: false,
            error: None,
            source: CrashSource::Manual,
        }
    }
}

impl PinpointAppUI {
    pub fn open_manual_report(&mut self) {
        let report = crash::manual_report("User-reported problem", "");
        self.crash_dialog = Some(CrashDialog::manual(report));
    }

    pub fn open_manual_report_with_note(&mut self, summary: &str, note: &str) {
        let report = crash::manual_report(summary, note);
        self.crash_dialog = Some(CrashDialog::manual(report));
    }

    pub fn draw_crash_dialog(&mut self, ctx: &egui::Context) {
        let mut outcome: Option<CrashDialogOutcome> = None;
        {
            let Some(dialog) = self.crash_dialog.as_mut() else {
                return;
            };
            let compact = Self::is_compact(ctx);
            let title = match dialog.source {
                CrashSource::Startup => "Pinpoint closed unexpectedly",
                CrashSource::Manual => "Report a problem",
            };

            egui::Window::new(title)
                .collapsible(false)
                .resizable(true)
                .default_width(if compact { 330.0 } else { 560.0 })
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        "Nothing is sent automatically. Review or edit the report, then \
                         open your email app. The full report is copied to the clipboard \
                         before the email opens.",
                    );
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut dialog.report)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(12)
                                    .font(egui::TextStyle::Monospace),
                            );
                        });
                    if dialog.copied {
                        ui.colored_label(Color32::from_rgb(52, 211, 153), "Report copied.");
                    }
                    if let Some(error) = &dialog.error {
                        ui.colored_label(Color32::from_rgb(248, 113, 113), error);
                    }
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Email report").clicked() {
                            ctx.copy_text(dialog.report.clone());
                            dialog.copied = true;
                            match platform::open_url(&crash::mailto_url(&dialog.report)) {
                                Ok(()) => dialog.error = None,
                                Err(error) => {
                                    dialog.error = Some(format!(
                                        "Could not open the email app: {error}. \
                                         The full report is on the clipboard."
                                    ))
                                }
                            }
                        }
                        if ui.button("Copy report").clicked() {
                            ctx.copy_text(dialog.report.clone());
                            dialog.copied = true;
                        }
                        if dialog.path.is_some() && ui.button("Discard").clicked() {
                            outcome = Some(CrashDialogOutcome::Discard);
                        }
                        if ui.button("Close").clicked() {
                            outcome = Some(CrashDialogOutcome::Close);
                        }
                    });
                });
        }

        match outcome {
            Some(CrashDialogOutcome::Discard) => {
                if let Some(dialog) = self.crash_dialog.as_ref() {
                    if let Some(path) = dialog.path.as_ref() {
                        crash::discard_report(path);
                    }
                }
                self.crash_dialog = None;
            }
            Some(CrashDialogOutcome::Close) => self.crash_dialog = None,
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crash::StoredReport;
    use std::path::PathBuf;

    #[test]
    fn from_pending_keeps_path_and_text() {
        let stored = StoredReport {
            path: PathBuf::from("/tmp/crash.txt"),
            text: "report".to_string(),
        };
        let dialog = CrashDialog::from_pending(stored);
        assert_eq!(dialog.report, "report");
        assert_eq!(dialog.path, Some(PathBuf::from("/tmp/crash.txt")));
        assert_eq!(dialog.source, CrashSource::Startup);
    }

    #[test]
    fn manual_dialog_has_no_path() {
        let dialog = CrashDialog::manual("manual".to_string());
        assert!(dialog.path.is_none());
        assert_eq!(dialog.source, CrashSource::Manual);
    }
}
