//! One editor shell. Document metadata chooses geometry; navigation never changes type.
use crate::document::{DocumentData, DocumentKind, Layout, Viewport};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};

pub struct Editor {
    viewport: Viewport,
}

impl Editor {
    pub fn new(data: &DocumentData) -> Self {
        Self {
            viewport: match data.layout() {
                Layout::Whiteboard { viewport, .. } => viewport.clone(),
                Layout::Page { .. } => Viewport::default(),
            },
        }
    }

    fn navigate(&mut self, delta: [f64; 2], factor: f64, anchor: [f64; 2]) {
        let old = self.viewport.zoom;
        let new = (old * factor).clamp(0.1, 8.0);
        for axis in 0..2 {
            self.viewport.center[axis] +=
                anchor[axis] * (1.0 / old - 1.0 / new) - delta[axis] / new;
        }
        self.viewport.zoom = new;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, data: &DocumentData) {
        match data.layout() {
            Layout::Page { pages } => {
                ui.weak("A4 · scroll to navigate pages");
                egui::Frame::new()
                    .fill(Color32::from_gray(65))
                    .show(ui, |ui| {
                        egui::ScrollArea::both().id_salt(data.id()).show(ui, |ui| {
                            ui.set_min_size(ui.available_size());
                            for (index, page) in pages.iter().enumerate() {
                                let width = (ui.available_width() - 40.0).clamp(210.0, 794.0);
                                let size = Vec2::new(
                                    width,
                                    width * (page.height_mm / page.width_mm) as f32,
                                );
                                ui.add_space(20.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(20.0);
                                    let (response, painter) =
                                        ui.allocate_painter(size, Sense::hover());
                                    painter.rect_filled(response.rect, 0.0, Color32::WHITE);
                                    painter.text(
                                        response.rect.center_bottom() - Vec2::new(0.0, 20.0),
                                        egui::Align2::CENTER_BOTTOM,
                                        format!("Page {}", index + 1),
                                        egui::FontId::proportional(12.0),
                                        Color32::GRAY,
                                    );
                                });
                            }
                            ui.add_space(20.0);
                        });
                    });
            }
            Layout::Whiteboard { grid, .. } => {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("−").on_hover_text("Zoom out").clicked() {
                        self.navigate([0.0; 2], 0.8, [0.0; 2]);
                    }
                    ui.label(format!("{:.0}%", self.viewport.zoom * 100.0));
                    if ui.button("+").on_hover_text("Zoom in").clicked() {
                        self.navigate([0.0; 2], 1.25, [0.0; 2]);
                    }
                    if ui.button("Reset view").clicked() {
                        self.viewport = Viewport::default();
                    }
                    ui.weak("Drag / space-drag to pan · Ctrl/Cmd-wheel or pinch to zoom");
                });
                let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::drag());
                let bounds = response.rect;
                let delta = if response.dragged() {
                    response.drag_delta()
                } else {
                    Vec2::ZERO
                };
                let factor = if response.hovered() {
                    ui.input(|i| i.zoom_delta()) as f64
                } else {
                    1.0
                };
                let anchor = response.hover_pos().unwrap_or(bounds.center()) - bounds.center();
                self.navigate(
                    [delta.x as f64, delta.y as f64],
                    factor,
                    [anchor.x as f64, anchor.y as f64],
                );
                painter.rect_filled(bounds, 0.0, Color32::from_gray(65));
                if *grid {
                    self.grid(&painter, bounds);
                }
            }
        }
    }

    fn grid(&self, painter: &egui::Painter, bounds: Rect) {
        // Only local offsets become f32: far-away world coordinates remain f64.
        let world_step = 32.0 * 2_f64.powf((-self.viewport.zoom.log2()).ceil().max(0.0));
        let spacing = (world_step * self.viewport.zoom) as f32;
        let offset = [
            self.viewport.center[0].rem_euclid(world_step) * self.viewport.zoom,
            self.viewport.center[1].rem_euclid(world_step) * self.viewport.zoom,
        ];
        let stroke = Stroke::new(1.0, Color32::from_gray(85));
        let start_x = bounds.center().x - offset[0] as f32;
        let start_y = bounds.center().y - offset[1] as f32;
        let x0 = bounds.left() + (start_x - bounds.left()).rem_euclid(spacing);
        let y0 = bounds.top() + (start_y - bounds.top()).rem_euclid(spacing);
        for col in 0..=(bounds.width() / spacing) as usize {
            let x = x0 + col as f32 * spacing;
            painter.line_segment(
                [Pos2::new(x, bounds.top()), Pos2::new(x, bounds.bottom())],
                stroke,
            );
        }
        for row in 0..=(bounds.height() / spacing) as usize {
            let y = y0 + row as f32 * spacing;
            painter.line_segment(
                [Pos2::new(bounds.left(), y), Pos2::new(bounds.right(), y)],
                stroke,
            );
        }
    }
}

pub fn indicator(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::Page => "▤ Page Note",
        DocumentKind::Whiteboard => "◇ Whiteboard",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_layouts_initialize_from_metadata_and_navigation_never_converts_a_document() {
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let data = DocumentData::blank(kind);
            let original = serde_json::to_vec(&data).unwrap();
            let mut editor = Editor::new(&data);
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| editor.show(ui, &data));
            let has_a4_page = output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Rect(rect) => {
                    rect.fill == Color32::WHITE
                        && rect.rect.width() >= 200.0
                        && (rect.rect.height() / rect.rect.width() - 297.0 / 210.0).abs() < 0.01
                }
                _ => false,
            });
            assert_eq!(has_a4_page, kind == DocumentKind::Page);
            output.textures_delta.clear();
            editor.navigate([100.0, -25.0], 2.0, [20.0, 10.0]);
            assert_eq!(data.kind(), kind);
            assert_eq!(serde_json::to_vec(&data).unwrap(), original);
            assert!(editor.viewport.zoom.is_finite());
        }
    }
    #[test]
    fn zoom_keeps_cursor_world_position_and_pan_has_no_page_bounds() {
        let mut editor = Editor::new(&DocumentData::blank(DocumentKind::Whiteboard));
        editor.viewport.center = [1e12, -1e12];
        let before = editor.viewport.center[0] + 100.0 / editor.viewport.zoom;
        editor.navigate([0.0; 2], 2.0, [100.0, 0.0]);
        assert!(
            (before - (editor.viewport.center[0] + 100.0 / editor.viewport.zoom)).abs() < 0.001
        );
        editor.navigate([1e7, 1e7], 1.0, [0.0; 2]);
        assert!(editor.viewport.center[1] < -1e12);
    }
}
