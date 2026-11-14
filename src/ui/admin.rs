//! Admin console + password screen.
//!
//! The web console has four modules (`admin/overview.js`, `users.js`,
//! `audit.js`, `diagnostics.js`); this screen mirrors them in one tabbed view.
use super::{PendingAction, PinpointAppUI};
use eframe::egui::{self, Color32, Direction, RichText};
use serde_json::Value;

impl PinpointAppUI {
    pub fn draw_admin_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.draw_main_toolbar(ui);
        ui.horizontal(|ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(Direction::LeftToRight),
                |ui| {
                    ui.heading("Admin Console");
                },
            );
        });
        ui.add_space(8.0);

        let (is_admin, my_id) = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            let is_admin = data
                .user
                .as_ref()
                .map(|u| {
                    (u.role.eq_ignore_ascii_case("admin")
                        || u.role.eq_ignore_ascii_case("administrator"))
                        && u.is_active
                })
                .unwrap_or(false);
            (is_admin, data.user.as_ref().map(|u| u.id))
        };

        // Server-side authorization is authoritative; this is a fail-closed
        // UI hint, matching the web console's hidden Admin tab.
        if !is_admin {
            ui.group(|ui| {
                ui.colored_label(
                    Color32::RED,
                    "Access denied: Admin or Administrator role required.",
                );
            });
            return;
        }

        if !self.admin_loaded {
            self.admin_loaded = true;
            self.spawn_action(PendingAction::AdminLoadOverview, ctx);
            self.spawn_action(PendingAction::AdminLoadUsers, ctx);
            self.spawn_action(PendingAction::AdminLoadAudit, ctx);
            self.spawn_action(PendingAction::AdminLoadDiagnostics { status: None }, ctx);
        }

        ui.horizontal_wrapped(|ui| {
            for (id, label) in [
                ("overview", "Overview"),
                ("users", "Users"),
                ("audit", "Audit"),
                ("diagnostics", "Diagnostics"),
            ] {
                if ui.selectable_label(self.admin_tab == id, label).clicked() {
                    self.admin_tab = id.to_string();
                }
            }
        });
        ui.separator();

        match self.admin_tab.as_str() {
            "users" => self.draw_admin_users(ui, ctx, my_id),
            "audit" => self.draw_admin_audit(ui),
            "diagnostics" => self.draw_admin_diagnostics(ui, ctx),
            _ => self.draw_admin_overview(ui, ctx),
        }
    }

    fn draw_admin_overview(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let overview = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            data.admin_overview.clone()
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("Deployment overview").size(14.0).strong());
            if ui.small_button("Refresh").clicked() {
                self.spawn_action(PendingAction::AdminLoadOverview, ctx);
            }
        });
        match overview {
            Some(value) => {
                if let Some(version) = value["version"].as_str() {
                    ui.label(format!(
                        "Pinpoint {version} · in-memory view when no database is configured."
                    ));
                }
                ui.horizontal_wrapped(|ui| {
                    for (key, label) in [
                        ("users", "Users"),
                        ("active_users", "Active users"),
                        ("sessions", "Sessions"),
                        ("friendships", "Friendships"),
                        ("shares", "Shares"),
                        ("diagnostics", "Open reports"),
                        ("audit_events", "Audit events"),
                    ] {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(15, 23, 42))
                            .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(31, 41, 55)))
                            .corner_radius(egui::CornerRadius::same(10))
                            .inner_margin(egui::Margin::same(12))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(value[key].as_i64().unwrap_or(0).to_string())
                                            .size(22.0)
                                            .strong(),
                                    );
                                    ui.label(RichText::new(label).size(11.0));
                                });
                            });
                    }
                });
            }
            None => {
                ui.label("Loading overview…");
            }
        }
    }

    fn draw_admin_users(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, my_id: Option<i64>) {
        let users = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            data.admin_users.clone()
        };
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.admin_search_query)
                    .hint_text("Filter by email, name or role")
                    .desired_width(260.0),
            );
            if ui.small_button("Refresh").clicked() {
                self.spawn_action(PendingAction::AdminLoadUsers, ctx);
            }
        });
        ui.add_space(4.0);
        let needle = self.admin_search_query.trim().to_lowercase();
        let rows: Vec<&Value> = users
            .iter()
            .filter(|user| {
                if needle.is_empty() {
                    return true;
                }
                ["email", "full_name", "role", "username"]
                    .iter()
                    .any(|key| {
                        user[*key]
                            .as_str()
                            .map(|v| v.to_lowercase().contains(&needle))
                            .unwrap_or(false)
                    })
            })
            .collect();
        if rows.is_empty() {
            ui.label("No matching users.");
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(440.0)
            .show(ui, |ui| {
                for user in rows {
                    let id = user["id"].as_i64().unwrap_or(0);
                    let email = user["email"].as_str().unwrap_or("");
                    let name = user["full_name"].as_str().unwrap_or("");
                    let username = user["username"].as_str().unwrap_or("");
                    let auth = user["auth_method"].as_str().unwrap_or("email");
                    let mut role = user["role"].as_str().unwrap_or("resident").to_string();
                    let mut active = user["is_active"].as_bool().unwrap_or(false);
                    let is_self = my_id == Some(id);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("#{id} {name}"));
                        ui.label(
                            RichText::new(format!("{email} · @{username} · {auth}")).size(11.0),
                        );
                        egui::ComboBox::from_id_salt(("admin_role", id))
                            .selected_text(&role)
                            .show_ui(ui, |ui| {
                                for candidate in ["resident", "admin", "administrator"] {
                                    ui.selectable_value(
                                        &mut role,
                                        candidate.to_string(),
                                        candidate,
                                    );
                                }
                            });
                        if !is_self && role != user["role"].as_str().unwrap_or("resident") {
                            self.spawn_action(
                                PendingAction::AdminUpdateUser {
                                    id,
                                    role: Some(role.clone()),
                                    is_active: None,
                                },
                                ctx,
                            );
                        }
                        if ui.checkbox(&mut active, "Active").changed() && !is_self {
                            self.spawn_action(
                                PendingAction::AdminUpdateUser {
                                    id,
                                    role: None,
                                    is_active: Some(active),
                                },
                                ctx,
                            );
                        }
                        if !is_self && ui.button("Delete").clicked() {
                            self.spawn_action(PendingAction::AdminDeleteUser { id }, ctx);
                        }
                    });
                    ui.separator();
                }
            });
    }

    fn draw_admin_audit(&mut self, ui: &mut egui::Ui) {
        let entries = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            data.admin_audit.clone()
        };
        ui.label(RichText::new(format!("Audit events ({})", entries.len())).strong());
        if entries.is_empty() {
            ui.label("No audit events loaded.");
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(460.0)
            .show(ui, |ui| {
                for entry in &entries {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "#{} @{} {} → {}",
                            entry.id, entry.actor_username, entry.action, entry.target
                        ));
                        if let Some(detail) = &entry.detail {
                            ui.label(RichText::new(detail).size(11.0));
                        }
                        ui.label(RichText::new(&entry.created_at).size(10.0));
                    });
                    ui.separator();
                }
            });
    }

    fn draw_admin_diagnostics(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let (reports, open, total) = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            (
                data.admin_reports.clone(),
                data.admin_report_open,
                data.admin_report_total,
            )
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{open} open · {total} matching"));
            let mut filter = self.admin_diag_filter.clone();
            egui::ComboBox::from_id_salt("admin_diag_status")
                .selected_text(if filter.is_empty() {
                    "All statuses"
                } else {
                    &filter
                })
                .show_ui(ui, |ui| {
                    for status in ["", "open", "triaged", "closed"] {
                        let label = if status.is_empty() {
                            "All statuses"
                        } else {
                            status
                        };
                        ui.selectable_value(&mut filter, status.to_string(), label);
                    }
                });
            if filter != self.admin_diag_filter {
                self.admin_diag_filter = filter.clone();
                self.spawn_action(
                    PendingAction::AdminLoadDiagnostics {
                        status: if filter.is_empty() {
                            None
                        } else {
                            Some(filter)
                        },
                    },
                    ctx,
                );
            }
            if ui.small_button("Refresh").clicked() {
                self.spawn_action(
                    PendingAction::AdminLoadDiagnostics {
                        status: if self.admin_diag_filter.is_empty() {
                            None
                        } else {
                            Some(self.admin_diag_filter.clone())
                        },
                    },
                    ctx,
                );
            }
        });
        if reports.is_empty() {
            ui.label("No reports.");
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(460.0)
            .show(ui, |ui| {
                for report in &reports {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "#{} {} · @{} · {}",
                            report.id,
                            report.category,
                            report.username.clone().unwrap_or_default(),
                            report.created_at
                        ));
                        let mut status = report.status.clone();
                        egui::ComboBox::from_id_salt(("admin_report_status", report.id))
                            .selected_text(&status)
                            .show_ui(ui, |ui| {
                                for candidate in ["open", "triaged", "closed"] {
                                    ui.selectable_value(
                                        &mut status,
                                        candidate.to_string(),
                                        candidate,
                                    );
                                }
                            });
                        if status != report.status {
                            self.spawn_action(
                                PendingAction::AdminUpdateReport {
                                    id: report.id,
                                    status,
                                },
                                ctx,
                            );
                        }
                    });
                    ui.label(RichText::new(&report.message).size(12.0));
                    ui.separator();
                }
            });
    }

    pub fn draw_change_password_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let forced = {
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            data.must_change_password
        };

        if !forced {
            self.draw_main_toolbar(ui);
        }
        ui.horizontal(|ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(Direction::LeftToRight),
                |ui| {
                    ui.heading("Change Password");
                },
            );
        });
        ui.add_space(8.0);

        if forced {
            ui.label("Your administrator requires a password change before continuing.");
            ui.add_space(8.0);
        }

        ui.group(|ui| {
            if !forced {
                ui.label("Current password");
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.change_password_current)
                        .password(true)
                        .hint_text("Current Password")
                        .desired_width(250.0),
                );
                ui.add_space(8.0);
            }
            ui.label("New password");
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.change_password_new)
                    .password(true)
                    .hint_text("New Password")
                    .desired_width(250.0),
            );
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.change_password_confirm)
                    .password(true)
                    .hint_text("Confirm New Password")
                    .desired_width(250.0),
            );
            ui.add_space(16.0);
            if ui.button("Update Password").clicked() {
                let current = self.change_password_current.trim().to_string();
                let new_password = self.change_password_new.clone();
                let confirm = self.change_password_confirm.clone();

                let error = if !forced && current.is_empty() {
                    Some("Enter your current password".to_string())
                } else if new_password.len() < 8 {
                    Some("New password must be at least 8 characters".to_string())
                } else if new_password != confirm {
                    Some("New passwords do not match".to_string())
                } else {
                    None
                };

                if let Some(msg) = error {
                    self.app.set_error(Some(msg));
                } else {
                    self.change_password_current.clear();
                    self.change_password_new.clear();
                    self.change_password_confirm.clear();
                    self.spawn_action(
                        PendingAction::ChangePassword {
                            current: if forced { None } else { Some(current) },
                            new_password,
                            confirm,
                        },
                        ctx,
                    );
                }
            }
        });
    }
}
