//! Transient snapping adapters on the existing Canvas gesture owner.
use super::*;
use crate::{object::Point, snapping::SnapSession};
impl Canvas {
    pub(super) fn begin_snapping(&mut self, surface: Surface) {
        if !self.snapping.objects && !self.snapping.grid {
            self.snap_session = None;
            return;
        }
        let targets = if self.snapping.objects {
            self.drawing.snap_targets(surface, &self.selection.expanded)
        } else {
            Vec::new()
        };
        let moving = self
            .selection
            .ids
            .iter()
            .filter_map(|id| self.drawing.alignment_bounds(*id))
            .reduce(crate::object::ObjectBounds::union);
        self.snap_session = moving.map(|bounds| {
            let mut session = SnapSession::new(targets, self.snapping);
            session.moving_bounds = Some(bounds);
            session
        });
    }
    pub(super) fn paint_snap_guides(
        &self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) {
        if self.selection.surface != surface
            || (self.selection_gesture.is_none() && self.text_resize.is_none())
        {
            return;
        }
        let Some(session) = &self.snap_session else {
            return;
        };
        let painter = ui
            .painter()
            .with_clip_rect(bounds.intersect(ui.clip_rect()));
        let color = crate::ui::style::palette(ui).accent;
        let screen = |point: Point| {
            let p = view.to_screen(point);
            egui::pos2(p[0] as f32, p[1] as f32)
        };
        for guide in session.guides.iter().flatten() {
            if let Some(gaps) = guide.gaps {
                for [start, end] in gaps {
                    let mut a = [0.; 2];
                    a[guide.axis] = start;
                    a[1 - guide.axis] = guide.coordinate;
                    let mut b = a;
                    b[guide.axis] = end;
                    let a = screen(a);
                    let b = screen(b);
                    painter
                        .line_segment([a, b], Line::new(crate::ui::style::SNAP_GUIDE_WIDTH, color));
                    for point in [a, b] {
                        let d = if guide.axis == 0 {
                            egui::vec2(0., 3.)
                        } else {
                            egui::vec2(3., 0.)
                        };
                        painter.line_segment(
                            [point - d, point + d],
                            Line::new(crate::ui::style::SNAP_GUIDE_WIDTH, color),
                        );
                    }
                    painter.text(
                        a + (b - a) * 0.5 + egui::vec2(4., -4.),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{:.1}", end - start),
                        egui::FontId::proportional(11.),
                        color,
                    );
                }
            } else {
                let mut a = [0.; 2];
                a[guide.axis] = guide.coordinate;
                a[1 - guide.axis] = guide.from;
                let mut b = a;
                b[1 - guide.axis] = guide.to;
                painter.line_segment(
                    [screen(a), screen(b)],
                    Line::new(crate::ui::style::SNAP_GUIDE_WIDTH, color),
                );
            }
        }
    }
    pub fn alignment_count(&self) -> usize {
        self.drawing.alignment_targets(&self.selection.ids).len()
    }
}

/// A handle may cross its opposite anchor. Find the moving grip in the normalized
/// result rather than accidentally snapping the now-fixed original corner index.
pub(super) fn resize_grip(
    shape: &crate::shape::Shape,
    index: usize,
    corner: bool,
    desired: Point,
) -> Point {
    if let Some(handles) = shape.note_resize_handles() {
        handles
            .into_iter()
            .enumerate()
            .filter(|(i, _)| {
                if index.is_multiple_of(2) {
                    i.is_multiple_of(2)
                } else {
                    i % 4 == index % 4
                }
            })
            .min_by(|(_, a), (_, b)| distance(*a, desired).total_cmp(&distance(*b, desired)))
            .unwrap()
            .1
    } else if corner {
        shape
            .box_corners()
            .unwrap()
            .into_iter()
            .min_by(|a, b| distance(*a, desired).total_cmp(&distance(*b, desired)))
            .unwrap()
    } else {
        shape.endpoints().unwrap()[index]
    }
}
fn distance(a: Point, b: Point) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
