//! In-memory editable ink and reversible commands. No UI, IO or PDF dependencies.
use crate::document::{DocumentData, Layout};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

pub const POINTS_PER_MM: f64 = 72.0 / 25.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Surface {
    Page(usize),
    Canvas,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct InkPoint {
    pub position: [f64; 2],
    /// Reserved for future stylus input. Mouse uses 1.0; M1 renders constant width.
    pub pressure: f32,
}
impl InkPoint {
    pub fn mouse(position: [f64; 2]) -> Self {
        Self {
            position,
            pressure: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub id: Uuid,
    pub document_id: Uuid,
    pub surface: Surface,
    pub points: Vec<InkPoint>,
    pub color: [u8; 4],
    /// PDF points (1/72 inch), independent of display pixels and zoom.
    pub width: f64,
}
impl Stroke {
    pub fn new(document_id: Uuid, surface: Surface, color: [u8; 4], width: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            document_id,
            surface,
            points: Vec::new(),
            color,
            width,
        }
    }
    pub fn push(&mut self, position: [f64; 2]) {
        // Keep all distinct input samples; interpolation is presentation-only.
        if self.points.last().is_none_or(|p| p.position != position) {
            self.points.push(InkPoint::mouse(position));
        }
    }
}

/// Screen = origin + (world - center) * scale. Subtract in f64 before conversion
/// at the UI boundary, including far-from-origin Whiteboard coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub origin: [f64; 2],
    pub center: [f64; 2],
    pub scale: f64,
}
impl Transform {
    pub fn to_screen(self, point: [f64; 2]) -> [f64; 2] {
        std::array::from_fn(|i| self.origin[i] + (point[i] - self.center[i]) * self.scale)
    }
    pub fn to_document(self, point: [f64; 2]) -> [f64; 2] {
        std::array::from_fn(|i| self.center[i] + (point[i] - self.origin[i]) / self.scale)
    }
}

// Each command retains just the affected objects and their original order.
// New object commands can extend this enum with their inverse state.
#[derive(Debug)]
enum Command {
    Insert { index: usize, stroke: Arc<Stroke> },
    Erase(Vec<(usize, Arc<Stroke>)>),
}
impl Command {
    fn apply(&self, strokes: &mut Vec<Arc<Stroke>>) {
        match self {
            Self::Insert { index, stroke } => strokes.insert(*index, stroke.clone()),
            Self::Erase(removed) => {
                let mut removed = removed.iter().peekable();
                let mut index = 0;
                strokes.retain(|_| {
                    let keep = removed.peek().is_none_or(|(target, _)| *target != index);
                    if !keep {
                        removed.next();
                    }
                    index += 1;
                    keep
                });
            }
        }
    }
    fn revert(&self, strokes: &mut Vec<Arc<Stroke>>) {
        match self {
            Self::Insert { index, .. } => {
                strokes.remove(*index);
            }
            Self::Erase(removed) => {
                // Merge backwards in linear time instead of shifting the tail
                // once per restored stroke. All slots contain valid Arcs.
                let original_len = strokes.len();
                strokes.extend(removed.iter().map(|(_, stroke)| stroke.clone()));
                let mut source = original_len;
                let mut removed = removed.iter().rev().peekable();
                for target in (0..strokes.len()).rev() {
                    if removed.peek().is_some_and(|(index, _)| *index == target) {
                        strokes[target] = removed.next().unwrap().1.clone();
                    } else {
                        source -= 1;
                        strokes[target] = strokes[source].clone();
                    }
                }
            }
        }
    }
}

pub struct Drawing {
    owner: Uuid,
    layout: Layout,
    strokes: Vec<Arc<Stroke>>,
    history: Vec<Command>,
    cursor: usize,
    revision: u64,
}
impl Drawing {
    pub fn new(data: &DocumentData) -> Self {
        Self {
            owner: data.id(),
            layout: data.layout().clone(),
            strokes: Vec::new(),
            history: Vec::new(),
            cursor: 0,
            revision: 0,
        }
    }
    /// Changes only when committed ink changes; invalidates derived spatial data.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn owner(&self) -> Uuid {
        self.owner
    }
    pub fn strokes(&self) -> &[Arc<Stroke>] {
        &self.strokes
    }
    pub fn dirty(&self) -> bool {
        !self.strokes.is_empty()
    }
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }
    pub fn can_redo(&self) -> bool {
        self.cursor < self.history.len()
    }
    pub fn bounds(&self, surface: Surface) -> Option<[f64; 2]> {
        match (&self.layout, surface) {
            (Layout::Page { pages }, Surface::Page(index)) => pages.get(index).map(|page| {
                [
                    page.width_mm * POINTS_PER_MM,
                    page.height_mm * POINTS_PER_MM,
                ]
            }),
            _ => None,
        }
    }
    pub fn contains(&self, surface: Surface, position: [f64; 2]) -> bool {
        if !position.iter().all(|v| v.is_finite()) {
            return false;
        }
        match (&self.layout, surface) {
            (Layout::Whiteboard { .. }, Surface::Canvas) => true,
            (Layout::Page { .. }, Surface::Page(_)) => self
                .bounds(surface)
                .is_some_and(|bounds| (0..2).all(|i| (0.0..=bounds[i]).contains(&position[i]))),
            _ => false,
        }
    }
    pub fn add(&mut self, stroke: Stroke) -> bool {
        if stroke.document_id != self.owner
            || stroke.id.is_nil()
            || stroke.points.is_empty()
            || !stroke.width.is_finite()
            || !(0.1..=24.0).contains(&stroke.width)
            || self.strokes.iter().any(|s| s.id == stroke.id)
            || stroke.points.iter().any(|p| {
                !self.contains(stroke.surface, p.position)
                    || !p.pressure.is_finite()
                    || !(0.0..=1.0).contains(&p.pressure)
            })
        {
            return false;
        }
        self.execute(Command::Insert {
            index: self.strokes.len(),
            stroke: Arc::new(stroke),
        });
        true
    }
    pub fn erase(&mut self, ids: &std::collections::HashSet<Uuid>) {
        if ids.is_empty() {
            return;
        }
        let removed: Vec<_> = self
            .strokes
            .iter()
            .enumerate()
            .filter(|(_, stroke)| ids.contains(&stroke.id))
            .map(|(index, stroke)| (index, stroke.clone()))
            .collect();
        if !removed.is_empty() {
            self.execute(Command::Erase(removed));
        }
    }
    fn execute(&mut self, command: Command) {
        self.history.truncate(self.cursor);
        command.apply(&mut self.strokes);
        self.history.push(command);
        self.cursor += 1;
        self.revision += 1;
    }
    pub fn undo(&mut self) {
        if self.can_undo() {
            self.cursor -= 1;
            self.revision += 1;
            self.history[self.cursor].revert(&mut self.strokes);
        }
    }
    pub fn redo(&mut self) {
        if self.can_redo() {
            self.history[self.cursor].apply(&mut self.strokes);
            self.cursor += 1;
            self.revision += 1;
        }
    }
}

/// Midpoint quadratic interpolation stays inside the samples' convex hull (A4
/// bounds included). Original points remain editable and available for M2.
pub fn smooth(points: &[InkPoint]) -> Vec<[f64; 2]> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let mut result = vec![first.position];
    let mut start = first.position;
    for pair in points.windows(2) {
        let control = pair[0].position;
        let end = std::array::from_fn(|i| (pair[0].position[i] + pair[1].position[i]) * 0.5);
        let distance = distance(start, control) + distance(control, end);
        let steps = (distance / 2.0).ceil().clamp(1.0, 32.0) as usize;
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            result.push(std::array::from_fn(|i| {
                (1.0 - t).powi(2) * start[i] + 2.0 * (1.0 - t) * t * control[i] + t * t * end[i]
            }));
        }
        start = end;
    }
    if let Some(last) = points.last() {
        result.push(last.position);
    }
    result
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn point_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let len2 = (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2);
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / len2).clamp(0.0, 1.0)
    };
    distance(p, std::array::from_fn(|i| a[i] + t * (b[i] - a[i])))
}
/// Swept eraser hit-test against the same smoothed centerline as rendering.
pub fn hit_path(path: &[[f64; 2]], from: [f64; 2], to: [f64; 2], radius: f64) -> bool {
    if path.len() == 1 {
        return point_segment(path[0], from, to) <= radius;
    }
    path.windows(2).any(|pair| {
        let (a, b) = (pair[0], pair[1]);
        if (0..2).any(|i| {
            (a[i].max(b[i]) + radius).next_up() < from[i].min(to[i])
                || (from[i].max(to[i]) + radius).next_up() < a[i].min(b[i])
        }) {
            return false;
        }
        let intersects = cross(a, b, from) * cross(a, b, to) < 0.0
            && cross(from, to, a) * cross(from, to, b) < 0.0;
        intersects
            || [
                point_segment(from, a, b),
                point_segment(to, a, b),
                point_segment(a, from, to),
                point_segment(b, from, to),
            ]
            .into_iter()
            .any(|d| d <= radius)
    })
}

// Original M1 exact test retained as an independent regression/benchmark oracle.
#[cfg(test)]
pub(crate) fn reference_hit_path(
    path: &[[f64; 2]],
    from: [f64; 2],
    to: [f64; 2],
    radius: f64,
) -> bool {
    if path.len() == 1 {
        return point_segment(path[0], from, to) <= radius;
    }
    path.windows(2).any(|pair| {
        let (a, b) = (pair[0], pair[1]);
        let intersects = cross(a, b, from) * cross(a, b, to) < 0.0
            && cross(from, to, a) * cross(from, to, b) < 0.0;
        intersects
            || [
                point_segment(from, a, b),
                point_segment(to, a, b),
                point_segment(a, from, to),
                point_segment(b, from, to),
            ]
            .into_iter()
            .any(|d| d <= radius)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentKind;
    use std::collections::HashSet;

    fn stroke(data: &DocumentData, surface: Surface, points: &[[f64; 2]]) -> Stroke {
        let mut stroke = Stroke::new(data.id(), surface, [20, 80, 180, 255], 2.5);
        for point in points {
            stroke.push(*point);
        }
        stroke
    }
    #[test]
    fn strokes_have_identity_pressure_and_serializable_vector_samples() {
        let data = DocumentData::blank(DocumentKind::Page);
        let a = stroke(&data, Surface::Page(0), &[[1.0, 2.0], [30.0, 40.0]]);
        let b = stroke(&data, Surface::Page(0), &[[1.0, 2.0]]);
        assert_ne!(a.id, b.id);
        assert_eq!(a.points[0].pressure, 1.0);
        let bytes = serde_json::to_vec(&a).unwrap();
        assert_eq!(serde_json::from_slice::<Stroke>(&bytes).unwrap(), a);
        let mut drawing = Drawing::new(&data);
        assert!(drawing.add(a.clone()));
        assert!(!drawing.add(a));
        assert!(drawing.dirty());
    }
    #[test]
    fn transformations_round_trip_at_zoom_pan_resize_and_large_origins() {
        for scale in [0.1, 1.0, 2.75, 8.0] {
            for origin in [[30.0, 50.0], [900.0, 400.0]] {
                let transform = Transform {
                    origin,
                    center: [1e12, -1e12],
                    scale,
                };
                let world = [1e12 + 125.0, -1e12 - 20.0];
                let screen = transform.to_screen(world);
                let restored = transform.to_document(screen);
                assert!((world[0] - restored[0]).abs() < 0.001);
                assert!((world[1] - restored[1]).abs() < 0.001);
                // DPI does not enter a logical-coordinate transform.
                for dpi in [1.0, 1.5, 2.0] {
                    let physical = screen.map(|v| v * dpi);
                    assert_eq!(transform.to_document(physical.map(|v| v / dpi)), restored);
                }
            }
        }
    }
    #[test]
    fn page_boundaries_and_document_ownership_are_enforced() {
        let page = DocumentData::blank(DocumentKind::Page);
        let board = DocumentData::blank(DocumentKind::Whiteboard);
        let mut drawing = Drawing::new(&page);
        for (surface, point) in [
            (Surface::Canvas, [0.0, 0.0]),
            (Surface::Page(1), [0.0, 0.0]),
            (Surface::Page(0), [-0.1, 10.0]),
            (Surface::Page(0), [600.0, 10.0]),
            (Surface::Page(0), [10.0, f64::NAN]),
        ] {
            assert!(!drawing.add(stroke(&page, surface, &[point])));
        }
        assert!(!drawing.add(stroke(&board, Surface::Page(0), &[[1.0, 1.0]])));
        assert!(drawing.add(stroke(
            &page,
            Surface::Page(0),
            &[[0.0, 0.0], [210.0 * POINTS_PER_MM, 297.0 * POINTS_PER_MM]]
        )));
        let path = smooth(&drawing.strokes()[0].points);
        assert!(path.iter().all(|p| drawing.contains(Surface::Page(0), *p)));
        let mut whiteboard = Drawing::new(&board);
        assert!(whiteboard.add(stroke(&board, Surface::Canvas, &[[-1e12, 1e12]])));
        assert_eq!(drawing.strokes().len(), 1);
    }
    #[test]
    fn swept_eraser_hits_segments_dots_and_stroke_width() {
        let path = vec![[0.0, 0.0], [100.0, 0.0]];
        assert!(hit_path(&path, [50.0, -100.0], [50.0, 100.0], 1.0));
        assert!(hit_path(&path, [40.0, 4.0], [60.0, 4.0], 5.0));
        assert!(!hit_path(&path, [40.0, 7.0], [60.0, 7.0], 5.0));
        assert!(hit_path(&[[3.0, 4.0]], [3.0, 4.0], [3.0, 4.0], 1.0));
        assert!(!hit_path(&[], [0.0, 0.0], [1.0, 1.0], 10.0));
    }
    #[test]
    fn commands_undo_redo_multiple_erasures_in_order_and_branch() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let mut drawing = Drawing::new(&data);
        let strokes: Vec<_> = (0..4)
            .map(|i| stroke(&data, Surface::Canvas, &[[i as f64, 0.0]]))
            .collect();
        for s in &strokes {
            assert!(drawing.add(s.clone()));
        }
        drawing.erase(&HashSet::from([strokes[0].id, strokes[2].id]));
        assert_eq!(
            drawing.strokes().iter().map(|s| s.id).collect::<Vec<_>>(),
            vec![strokes[1].id, strokes[3].id]
        );
        drawing.undo();
        assert_eq!(
            drawing.strokes().iter().map(|s| s.id).collect::<Vec<_>>(),
            strokes.iter().map(|s| s.id).collect::<Vec<_>>()
        );
        drawing.redo();
        drawing.undo();
        for _ in 0..4 {
            drawing.undo();
        }
        assert!(!drawing.dirty());
        assert!(!drawing.can_undo());
        for _ in 0..4 {
            drawing.redo();
        }
        assert_eq!(drawing.strokes().len(), 4);
        drawing.undo();
        drawing.add(stroke(&data, Surface::Canvas, &[[9.0, 9.0]]));
        assert!(!drawing.can_redo());
    }
    #[test]
    fn histories_are_isolated_between_documents() {
        let a = DocumentData::blank(DocumentKind::Whiteboard);
        let b = DocumentData::blank(DocumentKind::Whiteboard);
        let mut one = Drawing::new(&a);
        let mut two = Drawing::new(&b);
        one.add(stroke(&a, Surface::Canvas, &[[10.0, 20.0]]));
        two.add(stroke(&b, Surface::Canvas, &[[30.0, 40.0]]));
        one.undo();
        assert!(one.strokes().is_empty());
        assert_eq!(two.strokes().len(), 1);
        assert!(!two.add(stroke(&a, Surface::Canvas, &[[0.0, 0.0]])));
        assert!(two.dirty());
    }
}
