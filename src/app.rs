use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

#[derive(Default, PartialEq)]
enum PreviewMode {
    #[default]
    A4,
    Whiteboard,
}

#[derive(Default)]
pub struct XiTocApp {
    mode: PreviewMode,
}

impl eframe::App for XiTocApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("explorer")
            .resizable(true)
            .default_size(220.0)
            .size_range(140.0..=400.0)
            .show(ui, |ui| {
                ui.heading("Explorer");
                ui.separator();
                ui.label("No workspace open");
                ui.add_space(8.0);
                ui.weak("Real folders and PDF files will appear here when workspace support is available.");
                ui.take_available_space();
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("XiToc");
                ui.separator();
                ui.label("Canvas preview");
                ui.weak("M0 · session only");
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label("Mode:");
                ui.selectable_value(&mut self.mode, PreviewMode::A4, "A4");
                ui.selectable_value(&mut self.mode, PreviewMode::Whiteboard, "Whiteboard");
                ui.separator();
                ui.add_enabled(false, egui::Button::new("Pen"));
                ui.add_enabled(false, egui::Button::new("Eraser"));
                ui.add_enabled(false, egui::Button::new("Hand"));
            });
            ui.label("Preview only · drawing, pan/zoom, files and saving are not implemented.");
            ui.separator();
            self.canvas(ui);
        });
    }
}

impl XiTocApp {
    fn canvas(&self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
        let bounds = response.rect;
        painter.rect_filled(bounds, 0.0, Color32::from_gray(65));
        match self.mode {
            PreviewMode::A4 => {
                let available = (bounds.size() - Vec2::splat(40.0)).max(Vec2::ZERO);
                let scale = (available.x / 210.0).min(available.y / 297.0);
                let page = Rect::from_center_size(bounds.center(), Vec2::new(210.0, 297.0) * scale);
                painter.rect_filled(
                    page.translate(Vec2::splat(4.0)),
                    0.0,
                    Color32::from_gray(40),
                );
                painter.rect_filled(page, 0.0, Color32::WHITE);
                painter.text(
                    page.center(),
                    egui::Align2::CENTER_CENTER,
                    "A4 · 210 × 297 mm",
                    egui::FontId::proportional(16.0),
                    Color32::from_gray(100),
                );
            }
            PreviewMode::Whiteboard => {
                let spacing = 32.0;
                let stroke = Stroke::new(1.0, Color32::from_gray(85));
                for column in 0..=(bounds.width() / spacing) as usize {
                    let x = bounds.left() + column as f32 * spacing;
                    painter.line_segment(
                        [Pos2::new(x, bounds.top()), Pos2::new(x, bounds.bottom())],
                        stroke,
                    );
                }
                for row in 0..=(bounds.height() / spacing) as usize {
                    let y = bounds.top() + row as f32 * spacing;
                    painter.line_segment(
                        [Pos2::new(bounds.left(), y), Pos2::new(bounds.right(), y)],
                        stroke,
                    );
                }
                painter.text(
                    bounds.center(),
                    egui::Align2::CENTER_CENTER,
                    "Whiteboard · unbounded canvas preview",
                    egui::FontId::proportional(16.0),
                    Color32::WHITE,
                );
            }
        }
    }
}
