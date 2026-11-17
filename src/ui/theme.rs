//! One palette and one egui `Style` for the whole native client.
//!
//! Screen modules must not hard-code colors; they read the semantic constants
//! here or use [`card`] / [`primary_button`]. The palette mirrors
//! `web/app/app.css` so desktop, Android and the web shell read as one product.

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Margin, RichText, Stroke, TextStyle, Vec2,
};

pub const BG: Color32 = Color32::from_rgb(0x0c, 0x15, 0x12);
pub const PANEL: Color32 = Color32::from_rgb(0x12, 0x20, 0x1b);
pub const PANEL_2: Color32 = Color32::from_rgb(0x17, 0x2a, 0x22);
pub const BORDER: Color32 = Color32::from_rgb(0x2a, 0x3c, 0x33);
pub const TEXT: Color32 = Color32::from_rgb(0xf0, 0xea, 0xdc);
pub const MUTED: Color32 = Color32::from_rgb(0xa5, 0xb2, 0xa6);
pub const ACCENT: Color32 = Color32::from_rgb(0xe3, 0xa4, 0x58);
pub const ACCENT_INK: Color32 = Color32::from_rgb(0x24, 0x17, 0x08);
pub const DANGER: Color32 = Color32::from_rgb(0xe0, 0x6c, 0x5a);
pub const SUCCESS: Color32 = Color32::from_rgb(0x8f, 0xc9, 0x9f);

/// Card radius in points; used by [`card`] and matching custom frames.
pub const CARD_RADIUS: u8 = 14;

/// Install the palette, type scale and widget metrics once, at startup.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BG;
    visuals.text_edit_bg_color = Some(BG);
    visuals.faint_bg_color = PANEL_2;
    visuals.code_bg_color = PANEL_2;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.window_corner_radius = CornerRadius::same(16);
    visuals.menu_corner_radius = CornerRadius::same(12);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = ACCENT;
    visuals.error_fg_color = DANGER;
    visuals.weak_text_color = Some(MUTED);
    visuals.selection = egui::style::Selection {
        bg_fill: ACCENT,
        stroke: Stroke::new(1.0_f32, ACCENT_INK),
    };

    let rounded = CornerRadius::same(12);
    let outline = Stroke::new(1.0_f32, BORDER);

    // Secondary widgets are outline-only; only the selected/pressed state
    // fills with the accent, so the primary action on a screen stands out.
    let noninteractive = egui::style::WidgetVisuals {
        bg_fill: PANEL_2,
        weak_bg_fill: PANEL_2,
        bg_stroke: outline,
        corner_radius: rounded,
        fg_stroke: Stroke::new(1.0_f32, TEXT),
        expansion: 0.0,
    };
    visuals.widgets = egui::style::Widgets {
        noninteractive,
        inactive: egui::style::WidgetVisuals {
            bg_fill: PANEL_2,
            weak_bg_fill: Color32::TRANSPARENT,
            bg_stroke: outline,
            corner_radius: rounded,
            fg_stroke: Stroke::new(1.0_f32, TEXT),
            expansion: 0.0,
        },
        hovered: egui::style::WidgetVisuals {
            bg_fill: PANEL_2,
            weak_bg_fill: PANEL_2,
            bg_stroke: Stroke::new(1.0_f32, ACCENT),
            corner_radius: rounded,
            fg_stroke: Stroke::new(1.0_f32, TEXT),
            expansion: 0.0,
        },
        active: egui::style::WidgetVisuals {
            bg_fill: ACCENT,
            weak_bg_fill: ACCENT,
            bg_stroke: Stroke::new(1.0_f32, ACCENT),
            corner_radius: rounded,
            // `strong_text_color()` reads this; keep strong labels readable on
            // dark surfaces. Primary buttons set their ink explicitly.
            fg_stroke: Stroke::new(1.0_f32, TEXT),
            expansion: 0.0,
        },
        open: egui::style::WidgetVisuals {
            bg_fill: PANEL_2,
            weak_bg_fill: PANEL_2,
            bg_stroke: Stroke::new(1.0_f32, ACCENT),
            corner_radius: rounded,
            fg_stroke: Stroke::new(1.0_f32, TEXT),
            expansion: 0.0,
        },
    };
    style.visuals = visuals;

    // 44 pt minimum touch target, readable 15 pt body, calm spacing.
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(14.0, 10.0);
    style.spacing.interact_size = Vec2::new(44.0, 44.0);
    style.spacing.menu_margin = Margin::same(12);
    style.spacing.window_margin = Margin::same(16);
    style.spacing.combo_width = 120.0;
    style.spacing.scroll.bar_width = 8.0;
    // Keep scrollbars out of the way until the pointer is over them.
    style.spacing.scroll.dormant_handle_opacity = 0.0;
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(26.0)),
        (TextStyle::Body, FontId::proportional(15.0)),
        (TextStyle::Button, FontId::proportional(15.0)),
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();

    ctx.set_style(style);
}

/// A filled, rounded, consistently padded card. Replaces bare `ui.group(...)`.
pub fn card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(PANEL_2)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::same(16))
        .show(ui, add_contents)
}

/// Like [`card`], but fills the available width. Used for dashboard columns so
/// every card lines up instead of shrink-wrapping its copy.
pub fn full_card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    card(ui, |ui| {
        let width = ui.available_width();
        ui.set_min_width(width);
        add_contents(ui)
    })
}

/// Card heading: larger and stronger than body, never a raw `ui.label`.
pub fn card_title(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(18.0).strong().color(TEXT));
}

/// Muted supporting line under a card heading.
pub fn caption(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(12.0).color(MUTED));
}

/// The single accent action of a card or screen.
pub fn primary_button(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
    let text = RichText::new(text.into()).strong().color(ACCENT_INK);
    ui.add(
        egui::Button::new(text)
            .fill(ACCENT)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(12)),
    )
}

/// The wordmark: eframe ships no serif face, so a larger semibold sans.
pub fn wordmark(ui: &mut egui::Ui) {
    ui.label(RichText::new("Pinpoint").size(24.0).strong().color(TEXT));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_matches_the_web_shell() {
        assert_eq!(BG.to_array(), [0x0c, 0x15, 0x12, 255]);
        assert_eq!(PANEL_2.to_array(), [0x17, 0x2a, 0x22, 255]);
        assert_eq!(TEXT.to_array(), [0xf0, 0xea, 0xdc, 255]);
        assert_eq!(ACCENT.to_array(), [0xe3, 0xa4, 0x58, 255]);
        assert_eq!(ACCENT_INK.to_array(), [0x24, 0x17, 0x08, 255]);
        assert_eq!(DANGER.to_array(), [0xe0, 0x6c, 0x5a, 255]);
    }

    #[test]
    fn body_text_is_readable_and_targets_are_tall_enough() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let style = ctx.style();
        assert_eq!(style.text_styles[&TextStyle::Body].size, 15.0);
        assert!(style.spacing.interact_size.y >= 44.0);
    }
}
