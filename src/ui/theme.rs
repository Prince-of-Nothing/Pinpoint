//! One palette and one egui `Style` for the whole native client.
//!
//! Screen modules must not hard-code colors; they read the semantic constants
//! here or use [`card`] / [`primary_button`]. The palette mirrors
//! `web/app/app.css` so desktop, Android and the web shell read as one product.

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Margin, RichText, Stroke, TextStyle, Vec2,
};

/// The semantic colours of one theme. The values mirror the custom properties
/// in `web/app/app.css`; a test fails when either side changes alone.
#[allow(dead_code)] // The light palette is read by the parity test until the theme switch lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub panel_2: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_ink: Color32,
    /// Sunflower: the colour for "shared right now", never a button or link.
    pub live: Color32,
    pub danger: Color32,
}

/// Forest-night theme, the palette the native client draws with
/// (`[data-theme="dark"]` in the web shell).
pub(crate) const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x0e, 0x1f, 0x17),
    panel: Color32::from_rgb(0x15, 0x29, 0x1f),
    panel_2: Color32::from_rgb(0x1b, 0x33, 0x27),
    border: Color32::from_rgb(0x2a, 0x43, 0x34),
    text: Color32::from_rgb(0xe6, 0xee, 0xe6),
    muted: Color32::from_rgb(0x9f, 0xb2, 0xa5),
    accent: Color32::from_rgb(0xa7, 0xd8, 0xb6),
    accent_ink: Color32::from_rgb(0x0e, 0x1f, 0x17),
    live: Color32::from_rgb(0xf4, 0xc2, 0x1b),
    danger: Color32::from_rgb(0xf0, 0x85, 0x7a),
};

/// Limestone theme (`:root` in the web shell). Not applied yet.
#[allow(dead_code)] // Read by the parity test until the theme switch lands.
pub(crate) const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xe9, 0xec, 0xe6),
    panel: Color32::from_rgb(0xf8, 0xfa, 0xf6),
    panel_2: Color32::from_rgb(0xee, 0xf1, 0xeb),
    border: Color32::from_rgb(0xd3, 0xda, 0xcf),
    text: Color32::from_rgb(0x16, 0x26, 0x1d),
    muted: Color32::from_rgb(0x5e, 0x72, 0x66),
    accent: Color32::from_rgb(0x18, 0x38, 0x2a),
    accent_ink: Color32::from_rgb(0xf8, 0xfa, 0xf6),
    live: Color32::from_rgb(0xf4, 0xc2, 0x1b),
    danger: Color32::from_rgb(0xb4, 0x23, 0x18),
};

pub const BG: Color32 = DARK.bg;
pub const PANEL: Color32 = DARK.panel;
pub const PANEL_2: Color32 = DARK.panel_2;
pub const BORDER: Color32 = DARK.border;
pub const TEXT: Color32 = DARK.text;
pub const MUTED: Color32 = DARK.muted;
pub const ACCENT: Color32 = DARK.accent;
pub const ACCENT_INK: Color32 = DARK.accent_ink;
pub const DANGER: Color32 = DARK.danger;
pub const SUCCESS: Color32 = Color32::from_rgb(0x8f, 0xc9, 0x9f);

/// Fill for a surface that floats over content (the map's search pill, its
/// controls and chips, the sharing strip and the tab bar). It is the panel
/// colour at 93% opacity (238 of 255): near-solid, so text on it reads as it
/// does on a card, with the map only faintly showing beneath.
pub fn float_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(PANEL.r(), PANEL.g(), PANEL.b(), 238)
}

/// Elevation for a floating surface: a soft, low shadow instead of a hard edge.
pub fn float_shadow() -> egui::Shadow {
    egui::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(170),
    }
}

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

/// Elevation for a card on the page: a soft, low shadow, so a card lifts off
/// the background instead of relying on its border alone.
pub fn card_shadow() -> egui::Shadow {
    egui::Shadow {
        offset: [0, 6],
        blur: 22,
        spread: 0,
        color: Color32::from_black_alpha(160),
    }
}

/// A filled, rounded, consistently padded card. Replaces bare `ui.group(...)`.
pub fn card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    egui::Frame::new()
        .fill(PANEL_2)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .shadow(card_shadow())
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

    /// The web shell stylesheet, the source both palettes must match.
    const WEB_CSS: &str = include_str!("../../web/app/app.css");

    fn strip_css_comments(css: &str) -> String {
        let mut out = String::new();
        let mut rest = css;
        while let Some(open) = rest.find("/*") {
            out.push_str(&rest[..open]);
            rest = match rest[open..].find("*/") {
                Some(close) => &rest[open + close + 2..],
                None => "",
            };
        }
        out.push_str(rest);
        out
    }

    /// The `--name: value` declarations in the block that opens with `selector`.
    fn web_tokens(selector: &str) -> Vec<(String, String)> {
        let css = strip_css_comments(WEB_CSS);
        let start = css
            .find(selector)
            .unwrap_or_else(|| panic!("app.css has no `{selector}` block"));
        let body_start = start + css[start..].find('{').expect("block opening") + 1;
        let body_end = body_start + css[body_start..].find('}').expect("block closing");
        css[body_start..body_end]
            .split(';')
            .filter_map(|declaration| {
                let (name, value) = declaration.split_once(':')?;
                let name = name.trim().strip_prefix("--")?;
                Some((name.to_string(), value.trim().to_string()))
            })
            .collect()
    }

    fn hex_colour(value: &str) -> Color32 {
        let hex = value
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .unwrap_or_else(|| panic!("not a #rrggbb colour: {value}"));
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).expect("hex channel");
        Color32::from_rgb(channel(0), channel(2), channel(4))
    }

    fn assert_palette_matches_web(selector: &str, palette: Palette) {
        let tokens = web_tokens(selector);
        let pairs = [
            ("bg", palette.bg),
            ("panel", palette.panel),
            ("panel-2", palette.panel_2),
            ("border", palette.border),
            ("text", palette.text),
            ("muted", palette.muted),
            ("accent", palette.accent),
            ("accent-ink", palette.accent_ink),
            ("live", palette.live),
            ("danger", palette.danger),
        ];
        for (name, expected) in pairs {
            let value = tokens
                .iter()
                .find(|(token, _)| token == name)
                .map(|(_, value)| value.as_str())
                .unwrap_or_else(|| panic!("`{selector}` in app.css has no --{name}"));
            assert_eq!(
                hex_colour(value),
                expected,
                "--{name} in `{selector}` differs from theme.rs"
            );
        }
    }

    #[test]
    fn dark_palette_matches_the_web_dark_theme() {
        assert_palette_matches_web("[data-theme=\"dark\"] {", DARK);
    }

    #[test]
    fn light_palette_matches_the_web_light_theme() {
        assert_palette_matches_web(":root {", LIGHT);
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
