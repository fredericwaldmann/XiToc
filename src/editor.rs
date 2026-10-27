//! One editor shell. Document metadata chooses geometry; navigation never changes type.
use crate::document::{DocumentData, Layout, Viewport};
use crate::ui::style;
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

pub struct Editor {
    viewport: Viewport,
    grid_visible: bool,
}

impl Editor {
    pub fn new(data: &DocumentData) -> Self {
        Self {
            grid_visible: matches!(data.layout(), Layout::Whiteboard { grid: true, .. }),
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

    pub fn zoom(&self) -> f64 {
        self.viewport.zoom
    }
    pub fn zoom_by(&mut self, factor: f64) {
        self.navigate([0.0; 2], factor, [0.0; 2]);
    }
    pub fn reset_view(&mut self) {
        self.viewport = Viewport::default();
    }
    pub fn grid_visible(&self) -> bool {
        self.grid_visible
    }
    pub fn toggle_grid(&mut self) {
        self.grid_visible = !self.grid_visible;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, data: &DocumentData) {
        match data.layout() {
            Layout::Page { pages } => self.pages(ui, data, pages),
            Layout::Whiteboard { .. } => self.whiteboard(ui),
        }
    }

    fn pages(
        &self,
        ui: &mut egui::Ui,
        data: &DocumentData,
        pages: &[crate::document::PageGeometry],
    ) {
        let p = style::palette(ui);
        let available = ui.available_size();
        ui.painter()
            .rect_filled(ui.available_rect_before_wrap(), 0.0, p.workspace);
        egui::ScrollArea::both()
            .id_salt(data.id())
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = (available.x - 2.0 * style::PAGE_GAP).clamp(210.0, 794.0);
                let content_width = available.x.max(width + 2.0 * style::PAGE_GAP);
                ui.set_min_width(content_width);
                ui.add_space(style::PAGE_GAP);
                for (index, page) in pages.iter().enumerate() {
                    let size = Vec2::new(width, width * (page.height_mm / page.width_mm) as f32);
                    ui.horizontal(|ui| {
                        ui.add_space((content_width - width) * 0.5);
                        let (response, painter) = ui.allocate_painter(size, Sense::hover());
                        // Shadow extends beyond the page allocation, but stays clipped
                        // to the surrounding scroll viewport.
                        ui.painter().add(
                            egui::epaint::Shadow {
                                offset: [0, 3],
                                blur: 10,
                                spread: 1,
                                color: Color32::from_black_alpha(28),
                            }
                            .as_shape(response.rect, 0),
                        );
                        painter.rect_filled(response.rect, 0.0, Color32::WHITE);
                        painter.rect_stroke(
                            response.rect,
                            0.0,
                            Stroke::new(0.75, Color32::from_gray(194)),
                            StrokeKind::Inside,
                        );
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space((content_width - 100.0) * 0.5);
                        ui.add_sized(
                            [100.0, 16.0],
                            egui::Label::new(
                                egui::RichText::new(format!("Page {}", index + 1))
                                    .small()
                                    .color(p.muted),
                            ),
                        );
                    });
                    ui.add_space(style::PAGE_GAP);
                }
            });
    }

    fn whiteboard(&mut self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size().max(Vec2::ZERO), Sense::drag());
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
        painter.rect_filled(bounds, 0.0, style::palette(ui).board);
        if self.grid_visible {
            self.grid(&painter, bounds, style::palette(ui).dot);
        }
    }

    fn grid(&self, painter: &egui::Painter, bounds: Rect, color: Color32) {
        // Only local offsets become f32: far-away world coordinates remain f64.
        let world_step = 32.0 * 2_f64.powf((-self.viewport.zoom.log2()).ceil().max(0.0));
        let spacing = (world_step * self.viewport.zoom) as f32;
        let offset = [
            self.viewport.center[0].rem_euclid(world_step) * self.viewport.zoom,
            self.viewport.center[1].rem_euclid(world_step) * self.viewport.zoom,
        ];
        let start_x = bounds.center().x - offset[0] as f32;
        let start_y = bounds.center().y - offset[1] as f32;
        let x0 = bounds.left() + (start_x - bounds.left()).rem_euclid(spacing);
        let y0 = bounds.top() + (start_y - bounds.top()).rem_euclid(spacing);
        for col in 0..=(bounds.width() / spacing) as usize {
            for row in 0..=(bounds.height() / spacing) as usize {
                painter.circle_filled(
                    Pos2::new(x0 + col as f32 * spacing, y0 + row as f32 * spacing),
                    1.0,
                    color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentKind;
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
    fn view_controls_are_transient_and_do_not_change_pdf_data() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let original = serde_json::to_vec(&data).unwrap();
        let mut editor = Editor::new(&data);
        assert!(editor.grid_visible());
        editor.toggle_grid();
        assert!(!editor.grid_visible());
        editor.zoom_by(1.25);
        assert_eq!(editor.zoom(), 1.25);
        editor.reset_view();
        assert_eq!(editor.zoom(), 1.0);
        assert_eq!(serde_json::to_vec(&data).unwrap(), original);
        assert!(Editor::new(&data).grid_visible());
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
