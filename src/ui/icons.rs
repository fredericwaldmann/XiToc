//! Small original monoline symbols; no external fonts or image dependencies.
use super::style;
use crate::document::DocumentKind;
use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};

#[derive(Clone, Copy)]
pub enum Icon {
    Page,
    Board,
    Folder,
    Close,
    Sidebar,
    Plus,
    Minus,
    Reset,
    Grid,
}

impl From<DocumentKind> for Icon {
    fn from(kind: DocumentKind) -> Self {
        match kind {
            DocumentKind::Page => Self::Page,
            DocumentKind::Whiteboard => Self::Board,
        }
    }
}

pub fn paint(painter: &egui::Painter, center: Pos2, icon: Icon, color: Color32) {
    let stroke = Stroke::new(1.25, color);
    let rect = Rect::from_center_size(center, Vec2::splat(14.0));
    let point = |x: f32, y: f32| center + egui::vec2(x, y);
    let line = |a, b| painter.line_segment([a, b], stroke);
    match icon {
        Icon::Page => {
            painter.rect_stroke(
                rect.shrink2(egui::vec2(2.0, 0.0)),
                1.0,
                stroke,
                StrokeKind::Inside,
            );
            for y in [-2.5, 0.5, 3.5] {
                line(point(-2.5, y), point(2.5, y));
            }
        }
        Icon::Board => {
            painter.rect_stroke(
                rect.shrink2(egui::vec2(0.0, 2.0)),
                2.0,
                stroke,
                StrokeKind::Inside,
            );
            for x in [-3.5, 0.0, 3.5] {
                painter.circle_filled(point(x, 0.0), 0.8, color);
            }
        }
        Icon::Folder => {
            painter.rect_stroke(
                Rect::from_min_max(point(-7.0, -3.0), point(7.0, 6.0)),
                1.0,
                stroke,
                StrokeKind::Inside,
            );
            line(point(-6.5, -3.0), point(-6.5, -6.0));
            line(point(-6.5, -6.0), point(-1.0, -6.0));
            line(point(-1.0, -6.0), point(1.5, -3.0));
        }
        Icon::Close => {
            line(point(-3.5, -3.5), point(3.5, 3.5));
            line(point(-3.5, 3.5), point(3.5, -3.5));
        }
        Icon::Sidebar => {
            painter.rect_stroke(rect, 1.0, stroke, StrokeKind::Inside);
            line(point(-2.5, -6.0), point(-2.5, 6.0));
        }
        Icon::Plus | Icon::Minus => {
            line(point(-4.5, 0.0), point(4.5, 0.0));
            if matches!(icon, Icon::Plus) {
                line(point(0.0, -4.5), point(0.0, 4.5));
            }
        }
        Icon::Reset => {
            painter.circle_stroke(center, 5.0, stroke);
            line(point(-3.0, -5.0), point(-5.0, -5.0));
            line(point(-5.0, -5.0), point(-5.0, -2.0));
        }
        Icon::Grid => {
            for x in [-4.0, 0.0, 4.0] {
                for y in [-4.0, 0.0, 4.0] {
                    painter.circle_filled(point(x, y), 0.8, color);
                }
            }
        }
    }
}

pub fn show(ui: &mut egui::Ui, icon: Icon, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
    paint(ui.painter(), rect.center(), icon, color);
}

pub fn button(ui: &mut egui::Ui, icon: Icon, label: &str, selected: bool) -> egui::Response {
    let response = ui.add(
        egui::Button::new("")
            .min_size(Vec2::splat(style::CONTROL))
            .selected(selected)
            .frame_when_inactive(selected),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
    paint(
        ui.painter(),
        response.rect.center(),
        icon,
        style::palette(ui).text,
    );
    response.on_hover_text(label)
}
