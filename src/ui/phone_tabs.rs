//! Phone shell: the four bottom tabs (Map, People, Today, Chats), the avatar
//! that opens the menu with Account, and the Friends | Sharing switch on the
//! People tab. Screen-to-tab mapping is plain data so it can be tested
//! without a frame; the drawing helpers only call it.

use super::theme;
use crate::app::CurrentScreen;
use eframe::egui::{self, RichText};

/// One of the four bottom tabs, in web order. Account lives in the avatar menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PhoneTab {
    Map,
    People,
    Today,
    Chats,
}

impl PhoneTab {
    /// Tabs from left to right.
    pub(crate) const ALL: [PhoneTab; 4] = [
        PhoneTab::Map,
        PhoneTab::People,
        PhoneTab::Today,
        PhoneTab::Chats,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            PhoneTab::Map => "Map",
            PhoneTab::People => "People",
            PhoneTab::Today => "Today",
            PhoneTab::Chats => "Chats",
        }
    }

    /// The screen a tap opens. People opens Friends; Sharing is one tap away.
    pub(crate) fn screen(self) -> CurrentScreen {
        match self {
            PhoneTab::Map => CurrentScreen::Map,
            PhoneTab::People => CurrentScreen::Friends,
            PhoneTab::Today => CurrentScreen::Today,
            PhoneTab::Chats => CurrentScreen::Messages,
        }
    }

    /// The tab that owns a screen, or `None` for screens reached from the menu.
    pub(crate) fn owning(screen: &CurrentScreen) -> Option<PhoneTab> {
        match screen {
            CurrentScreen::Map => Some(PhoneTab::Map),
            CurrentScreen::Friends | CurrentScreen::Sharing => Some(PhoneTab::People),
            CurrentScreen::Today => Some(PhoneTab::Today),
            CurrentScreen::Messages => Some(PhoneTab::Chats),
            _ => None,
        }
    }
}

/// The two views behind the People tab, in switch order.
pub(crate) const PEOPLE_VIEWS: [(CurrentScreen, &str); 2] = [
    (CurrentScreen::Friends, "Friends"),
    (CurrentScreen::Sharing, "Sharing"),
];

/// The avatar letter: the first letter of the name in upper case, or `?`
/// when there is no name or it does not start with a letter.
pub(crate) fn avatar_initial(full_name: Option<&str>) -> char {
    full_name
        .and_then(|name| name.trim().chars().next())
        .filter(|c| c.is_alphabetic())
        .and_then(|c| c.to_uppercase().next())
        .unwrap_or('?')
}

/// Full height of the tab bar panel: a 4 pt top margin, the floating pill, and
/// a 20 pt bottom margin that keeps the pill clear of the gesture bar.
pub(crate) const BAR_HEIGHT: f32 = 78.0;
/// Side inset of the floating pill, as on the web tab bar.
const PILL_SIDE_INSET: i8 = 12;
/// Height of one tab's target inside the pill (at least 48 pt with the pill's
/// own 4 pt padding).
const TAB_HEIGHT: f32 = 48.0;
const PILL_PADDING: i8 = 4;
const TAB_BOTTOM_INSET: i8 = 20;

/// Draws the four-tab bar as a floating rounded pill over the bottom of the
/// screen. Returns the screen to open when a different tab is tapped; tapping
/// the tab that is already open does nothing, so a tap on People keeps Sharing
/// open.
pub(crate) fn draw_tab_bar(ctx: &egui::Context, current: &CurrentScreen) -> Option<CurrentScreen> {
    let owner = PhoneTab::owning(current);
    let mut chosen: Option<CurrentScreen> = None;
    egui::TopBottomPanel::bottom("mobile_tabbar")
        .resizable(false)
        .exact_height(BAR_HEIGHT)
        .frame(
            egui::Frame::NONE
                .fill(theme::BG)
                .inner_margin(egui::Margin {
                    left: PILL_SIDE_INSET,
                    right: PILL_SIDE_INSET,
                    top: 4,
                    bottom: TAB_BOTTOM_INSET,
                }),
        )
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme::PANEL)
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                .corner_radius(egui::CornerRadius::same(22))
                .inner_margin(egui::Margin::same(PILL_PADDING))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let tab_width = ui.available_width() / PhoneTab::ALL.len() as f32;
                    ui.horizontal(|ui| {
                        for tab in PhoneTab::ALL {
                            let selected = owner == Some(tab);
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(tab_width, TAB_HEIGHT),
                                egui::Sense::click(),
                            );
                            let ink = if selected {
                                theme::ACCENT
                            } else {
                                theme::MUTED
                            };
                            let painter = ui.painter();
                            if selected {
                                painter.rect_filled(
                                    rect.shrink(2.0),
                                    egui::CornerRadius::same(16),
                                    theme::PANEL_2,
                                );
                            }
                            paint_icon(painter, tab, rect.center() + egui::vec2(0.0, -8.0), ink);
                            painter.text(
                                rect.center() + egui::vec2(0.0, 13.0),
                                egui::Align2::CENTER_CENTER,
                                tab.label(),
                                egui::FontId::proportional(14.0),
                                ink,
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    tab.label(),
                                )
                            });
                            if response.clicked() && !selected {
                                chosen = Some(tab.screen());
                            }
                        }
                    });
                });
        });
    chosen
}

/// Paints the tab's line icon centred on `center`: a folded map, two people,
/// a clock, and a speech bubble. Drawn with the painter so the bar needs no
/// icon font.
fn paint_icon(painter: &egui::Painter, tab: PhoneTab, center: egui::Pos2, ink: egui::Color32) {
    let stroke = egui::Stroke::new(1.6_f32, ink);
    let at = |x: f32, y: f32| center + egui::vec2(x, y);
    match tab {
        PhoneTab::Map => {
            let outline = vec![
                at(-8.0, -6.0),
                at(-3.0, -8.0),
                at(3.0, -6.0),
                at(8.0, -8.0),
                at(8.0, 6.0),
                at(3.0, 8.0),
                at(-3.0, 6.0),
                at(-8.0, 8.0),
            ];
            painter.add(egui::Shape::closed_line(outline, stroke));
            painter.line_segment([at(-3.0, -8.0), at(-3.0, 6.0)], stroke);
            painter.line_segment([at(3.0, -6.0), at(3.0, 8.0)], stroke);
        }
        PhoneTab::People => {
            painter.circle_stroke(at(-3.0, -4.0), 3.5, stroke);
            painter.circle_stroke(at(4.5, -5.0), 2.8, stroke);
            let arc: Vec<egui::Pos2> = (0..=12)
                .map(|step| {
                    let angle = std::f32::consts::PI + step as f32 * std::f32::consts::PI / 12.0;
                    at(-3.0 + 6.5 * angle.cos(), 9.0 + 6.5 * angle.sin())
                })
                .collect();
            painter.add(egui::Shape::line(arc, stroke));
        }
        PhoneTab::Today => {
            painter.circle_stroke(center, 8.0, stroke);
            painter.line_segment([center, at(0.0, -4.5)], stroke);
            painter.line_segment([center, at(3.5, 1.5)], stroke);
        }
        PhoneTab::Chats => {
            let bubble = egui::Rect::from_min_max(at(-8.0, -7.0), at(8.0, 4.0));
            painter.rect_stroke(
                bubble,
                egui::CornerRadius::same(4),
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment([at(-3.0, 4.0), at(-6.0, 8.0)], stroke);
            painter.line_segment([at(-6.0, 8.0), at(-1.0, 4.0)], stroke);
        }
    }
}

/// Friends | Sharing switch at the top of the People tab. Returns the view
/// that was tapped when it is not the one showing.
pub(crate) fn draw_people_switch(
    ui: &mut egui::Ui,
    current: &CurrentScreen,
) -> Option<CurrentScreen> {
    let mut chosen: Option<CurrentScreen> = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let width = ui.available_width() / PEOPLE_VIEWS.len() as f32;
        for (screen, label) in PEOPLE_VIEWS {
            let selected = *current == screen;
            let text_color = if selected {
                theme::ACCENT_INK
            } else {
                theme::TEXT
            };
            let button =
                egui::Button::new(RichText::new(label).size(14.0).strong().color(text_color))
                    .fill(if selected {
                        theme::ACCENT
                    } else {
                        theme::PANEL_2
                    })
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        if selected {
                            theme::ACCENT
                        } else {
                            theme::BORDER
                        },
                    ))
                    .min_size(egui::vec2(width, TAB_HEIGHT))
                    .corner_radius(egui::CornerRadius::same(10));
            if ui.add(button).clicked() && !selected {
                chosen = Some(screen);
            }
        }
    });
    chosen
}

/// The account avatar in the compact header. Tapping it opens the menu, which
/// holds Account and the tools that are not tabs.
pub(crate) fn avatar_button(ui: &mut egui::Ui, full_name: Option<&str>) -> egui::Response {
    let letter = avatar_initial(full_name).to_string();
    ui.add(
        egui::Button::new(
            RichText::new(letter)
                .size(18.0)
                .strong()
                .color(theme::ACCENT_INK),
        )
        .fill(theme::ACCENT)
        .min_size(egui::vec2(44.0, 44.0))
        .corner_radius(egui::CornerRadius::same(22)),
    )
    .on_hover_text("Account and more")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_shows_four_tabs_in_web_order_with_short_labels() {
        let labels: Vec<&str> = PhoneTab::ALL.iter().map(|tab| tab.label()).collect();
        assert_eq!(labels, ["Map", "People", "Today", "Chats"]);
    }

    #[test]
    fn each_tab_opens_a_screen_that_it_owns() {
        for tab in PhoneTab::ALL {
            assert_eq!(PhoneTab::owning(&tab.screen()), Some(tab), "{tab:?}");
        }
    }

    #[test]
    fn people_tab_opens_friends_and_owns_sharing_too() {
        assert_eq!(PhoneTab::People.screen(), CurrentScreen::Friends);
        assert_eq!(
            PhoneTab::owning(&CurrentScreen::Sharing),
            Some(PhoneTab::People)
        );
    }

    #[test]
    fn chats_tab_opens_messages_and_today_opens_today() {
        assert_eq!(PhoneTab::Chats.screen(), CurrentScreen::Messages);
        assert_eq!(PhoneTab::Today.screen(), CurrentScreen::Today);
        assert_eq!(
            PhoneTab::owning(&CurrentScreen::Messages),
            Some(PhoneTab::Chats)
        );
    }

    #[test]
    fn screens_reached_from_the_menu_belong_to_no_tab() {
        for screen in [
            CurrentScreen::Profile,
            CurrentScreen::Places,
            CurrentScreen::Bookmarks,
            CurrentScreen::Trips,
            CurrentScreen::Emergency,
            CurrentScreen::About,
            CurrentScreen::Admin,
            CurrentScreen::Kiosk,
            CurrentScreen::ChangePassword,
            CurrentScreen::Login,
        ] {
            assert_eq!(PhoneTab::owning(&screen), None, "{screen:?}");
        }
    }

    #[test]
    fn people_switch_lists_friends_before_sharing() {
        let screens: Vec<CurrentScreen> = PEOPLE_VIEWS.iter().map(|(s, _)| s.clone()).collect();
        assert_eq!(screens, [CurrentScreen::Friends, CurrentScreen::Sharing]);
    }

    #[test]
    fn avatar_uses_the_first_letter_of_the_name_in_upper_case() {
        assert_eq!(avatar_initial(Some("ada lovelace")), 'A');
        assert_eq!(avatar_initial(Some("  Émile")), 'É');
    }

    /// Runs one frame of the bar on a 360 x 800 phone with the given events.
    fn bar_frame(
        ctx: &egui::Context,
        current: &CurrentScreen,
        events: Vec<egui::Event>,
    ) -> Option<CurrentScreen> {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(360.0, 800.0),
            )),
            events,
            ..Default::default()
        };
        let mut chosen = None;
        let _ = ctx.run(input, |ctx| {
            chosen = draw_tab_bar(ctx, current);
        });
        chosen
    }

    /// Presses and releases at `pos` over two frames, as a real tap does, and
    /// returns what the release frame chose.
    fn tap(ctx: &egui::Context, current: &CurrentScreen, pos: egui::Pos2) -> Option<CurrentScreen> {
        let press = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        let release = vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }];
        let _ = bar_frame(ctx, current, press);
        bar_frame(ctx, current, release)
    }

    /// Centre of tab `index` in the 360 pt wide bar: the 12 pt side inset, the
    /// pill's 4 pt padding, and four equal columns. The row starts 4 pt below
    /// the top of the bar plus the pill's 4 pt padding.
    fn tab_centre(index: usize) -> egui::Pos2 {
        let column = (360.0 - 2.0 * 12.0 - 2.0 * 4.0) / 4.0;
        egui::pos2(
            12.0 + 4.0 + column * (index as f32 + 0.5),
            800.0 - 78.0 + 4.0 + 4.0 + 24.0,
        )
    }

    #[test]
    fn tapping_a_tab_opens_its_screen() {
        let ctx = egui::Context::default();
        // Lay out the bar once so the taps have widget rects to hit.
        let _ = bar_frame(&ctx, &CurrentScreen::Map, Vec::new());

        assert_eq!(
            tap(&ctx, &CurrentScreen::Map, tab_centre(3)),
            Some(CurrentScreen::Messages),
            "Chats tab"
        );
        assert_eq!(
            tap(&ctx, &CurrentScreen::Map, tab_centre(1)),
            Some(CurrentScreen::Friends),
            "People tab"
        );
    }

    #[test]
    fn tapping_the_open_tab_keeps_the_current_screen() {
        let ctx = egui::Context::default();
        let _ = bar_frame(&ctx, &CurrentScreen::Sharing, Vec::new());

        // People is already open on Sharing; a tap on it must not reset to Friends.
        assert_eq!(tap(&ctx, &CurrentScreen::Sharing, tab_centre(1)), None);
    }

    #[test]
    fn avatar_falls_back_to_a_question_mark() {
        assert_eq!(avatar_initial(None), '?');
        assert_eq!(avatar_initial(Some("   ")), '?');
        assert_eq!(avatar_initial(Some("7 seas")), '?');
    }
}
