//! Small QR renderer for deep links shown inside egui.

use eframe::egui::{self, Color32};

pub(crate) fn draw_qr(ui: &mut egui::Ui, data: &str, size: f32) {
    let Ok(code) = qrcode::QrCode::new(data.as_bytes()) else {
        return;
    };
    let width = code.width();
    let colors = code.to_colors();
    let module = size / width as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    for (index, color) in colors.iter().enumerate() {
        if *color == qrcode::Color::Dark {
            let x = rect.left() + (index % width) as f32 * module;
            let y = rect.top() + (index / width) as f32 * module;
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(module, module)),
                0.0,
                Color32::BLACK,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn qr_matrix_matches_width() {
        let code = qrcode::QrCode::new(b"pinpoint://kiosk?token=abc123").unwrap();
        let colors = code.to_colors();
        assert!(code.width() >= 21);
        assert_eq!(colors.len(), code.width() * code.width());
    }
}
