//! Transient object selection and pure lasso geometry. No egui or platform IO.
use crate::{
    drawing::{Drawing, PressureStyle, Surface},
    object::{CanvasObject, ObjectBounds, ObjectId, Placement, Point},
};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SelectionCommand {
    SelectAll,
    Copy,
    Paste,
    Duplicate,
    Delete,
    Undo,
    Redo,
    Smaller,
    Larger,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Membership {
    Replace,
    Add,
    Subtract,
    Toggle,
}

pub struct Selection {
    pub ids: HashSet<ObjectId>,
    pub surface: Surface,
    pub bounds: Option<ObjectBounds>,
    pub geometry: Option<ObjectBounds>,
    pub objects: Vec<(ObjectId, ObjectBounds)>,
    revision: Option<u64>,
}
impl Selection {
    pub fn new(surface: Surface) -> Self {
        Self {
            ids: HashSet::new(),
            surface,
            bounds: None,
            geometry: None,
            objects: Vec::new(),
            revision: None,
        }
    }
    pub fn scope(&mut self, surface: Surface) {
        if self.surface != surface {
            self.clear();
            self.surface = surface;
        }
    }
    pub fn clear(&mut self) {
        self.ids.clear();
        self.bounds = None;
        self.geometry = None;
        self.objects.clear();
        self.revision = None;
    }
    pub fn change(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: Membership,
        drawing: &Drawing,
    ) {
        if mode == Membership::Replace {
            self.ids.clear();
        }
        for id in ids {
            match mode {
                Membership::Subtract => {
                    self.ids.remove(&id);
                }
                Membership::Toggle => {
                    if !self.ids.remove(&id) {
                        self.ids.insert(id);
                    }
                }
                _ => {
                    self.ids.insert(id);
                }
            }
        }
        self.revision = None;
        self.refresh(drawing);
    }
    pub fn refresh(&mut self, drawing: &Drawing) {
        if self.revision == Some(drawing.revision()) {
            return;
        }
        self.revision = Some(drawing.revision());
        self.bounds = None;
        self.geometry = None;
        self.objects.clear();
        if self.ids.is_empty() {
            return;
        }
        let mut live = HashSet::with_capacity(self.ids.len());
        for stroke in drawing
            .strokes()
            .iter()
            .filter(|s| s.surface() == self.surface && self.ids.contains(&s.object_id()))
        {
            live.insert(stroke.id);
            if let Some(b) = stroke.world_bounds() {
                self.objects.push((stroke.id, b));
                self.bounds = Some(self.bounds.map_or(b, |old| old.union(b)));
            }
            if let Some(b) = ObjectBounds::from_points(
                stroke
                    .points
                    .iter()
                    .map(|p| stroke.placement.apply(p.position)),
            ) {
                self.geometry = Some(self.geometry.map_or(b, |old| old.union(b)));
            }
        }
        self.ids = live;
    }
}

pub fn inside(point: Point, polygon: &[Point]) -> bool {
    let mut contained = false;
    for (a, b) in edges(polygon) {
        if (a[1] > point[1]) != (b[1] > point[1]) {
            let x = (b[0] - a[0]) * ((point[1] - a[1]) / (b[1] - a[1])) + a[0];
            if point[0] < x {
                contained = !contained;
            }
        }
    }
    contained
}
fn edges(polygon: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    polygon
        .iter()
        .copied()
        .zip(polygon.iter().copied().cycle().skip(1))
        .take(polygon.len())
}
pub fn valid_lasso(polygon: &[Point]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let a = polygon[0];
    polygon.windows(2).any(|p| {
        ((p[0][0] - a[0]) * (p[1][1] - a[1]) - (p[0][1] - a[1]) * (p[1][0] - a[0])).abs() > 1e-8
    })
}
/// Reuse the path-block broad phase for polygon edges: containment must not
/// scan every lasso edge for every vertex of a long handwritten stroke.
pub struct LassoRegion {
    polygon: Vec<Point>,
    index: crate::eraser_index::EraserIndex,
    pub bounds: ObjectBounds,
}
impl LassoRegion {
    pub fn new(mut polygon: Vec<Point>) -> Option<Self> {
        if !valid_lasso(&polygon) {
            return None;
        }
        let bounds = ObjectBounds::from_points(polygon.iter().copied())?;
        polygon.push(polygon[0]);
        let index = crate::eraser_index::EraserIndex::for_path(&polygon, 0.0, Surface::Canvas);
        Some(Self {
            polygon,
            index,
            bounds,
        })
    }
    /// Full rounded pressure envelope containment. Boundary contact is excluded.
    /// Segment/edge checks reject paths leaving/re-entering a concave polygon.
    pub fn encloses(
        &self,
        path: &[Point],
        pressures: &[Option<f32>],
        width: f64,
        pressure: PressureStyle,
    ) -> bool {
        if path.is_empty() || !self.bounds.contains(path[0]) || !inside(path[0], &self.polygon) {
            return false;
        }
        let radius = |i| width * 0.5 * pressure.factor(pressures[i]);
        for i in 0..path.len().saturating_sub(1).max(1) {
            let next = (i + 1).min(path.len() - 1);
            let a = path[i];
            let b = path[next];
            let radii = [radius(i), radius(next)];
            let mut crossed = false;
            self.index
                .query(Surface::Canvas, a, b, radii[0].max(radii[1]), |start| {
                    if crossed {
                        return;
                    }
                    let end = (start + crate::eraser_index::SEGMENTS_PER_ENTRY)
                        .min(self.polygon.len() - 1);
                    for edge in start..end {
                        if crate::partial_erase::clip_tapered(
                            a,
                            b,
                            self.polygon[edge],
                            self.polygon[edge + 1],
                            0.0,
                            radii,
                        )
                        .is_some()
                        {
                            crossed = true;
                            break;
                        }
                    }
                });
            if crossed {
                return false;
            }
        }
        true
    }
}
#[cfg(test)]
fn encloses(
    polygon: &[Point],
    path: &[Point],
    pressures: &[Option<f32>],
    width: f64,
    pressure: PressureStyle,
) -> bool {
    LassoRegion::new(polygon.to_vec()).is_some_and(|r| r.encloses(path, pressures, width, pressure))
}

/// Clamp a translation to the page without jumping ink already clipped at edges.
pub fn move_by(bounds: ObjectBounds, mut delta: Point, page: Option<Point>) -> Placement {
    if let Some(page) = page {
        for i in 0..2 {
            delta[i] = delta[i].clamp(-bounds.min[i].max(0.0), (page[i] - bounds.max[i]).max(0.0));
        }
    }
    Placement {
        origin: bounds.min,
        position: std::array::from_fn(|i| bounds.min[i] + delta[i]),
        scale: 1.0,
        rotation: 0.0,
    }
}
pub fn scale_about(
    geometry: ObjectBounds,
    anchor: Point,
    factor: f64,
    page: Option<Point>,
) -> Placement {
    let mut lo: f64 = 1e-4;
    let mut hi: f64 = 1e4;
    if let Some(page) = page {
        for i in 0..2 {
            for value in [geometry.min[i], geometry.max[i]] {
                let d = value - anchor[i];
                if d > 0.0 {
                    lo = lo.max(-anchor[i] / d);
                    hi = hi.min((page[i] - anchor[i]) / d);
                } else if d < 0.0 {
                    lo = lo.max((page[i] - anchor[i]) / d);
                    hi = hi.min(-anchor[i] / d);
                }
            }
        }
    }
    let scale = if lo <= hi { factor.clamp(lo, hi) } else { 1.0 };
    Placement {
        origin: anchor,
        position: anchor,
        scale,
        rotation: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_envelope_lasso_handles_width_pressure_concavity_and_self_intersections() {
        let square = [[0., 0.], [10., 0.], [10., 10.], [0., 10.]];
        let off = PressureStyle::default();
        assert!(valid_lasso(&square));
        assert!(encloses(
            &square,
            &[[2., 2.], [8., 8.]],
            &[None; 2],
            1.,
            off
        ));
        assert!(!encloses(
            &square,
            &[[2., 2.], [8., 8.]],
            &[None; 2],
            4.,
            off
        ));
        assert!(encloses(&square, &[[5., 5.]], &[None], 2., off));
        assert!(!encloses(&square, &[[11., 5.]], &[None], 1., off));
        let pressure = PressureStyle {
            enabled: true,
            strength: 100,
            minimum: 20,
        };
        assert!(encloses(
            &square,
            &[[1., 2.], [1., 8.]],
            &[Some(0.0); 2],
            4.,
            pressure
        ));
        assert!(!encloses(
            &square,
            &[[1., 2.], [1., 8.]],
            &[Some(1.0); 2],
            4.,
            pressure
        ));
        let notch = [
            [0., 0.],
            [10., 0.],
            [10., 10.],
            [7., 10.],
            [7., 3.],
            [3., 3.],
            [3., 10.],
            [0., 10.],
        ];
        assert!(inside([1., 8.], &notch) && inside([9., 8.], &notch));
        assert!(
            !encloses(&notch, &[[1., 8.], [9., 8.]], &[None; 2], 0.5, off),
            "Vertices inside must not select a segment crossing the notch"
        );
        let bow = [[0., 0.], [10., 10.], [0., 10.], [10., 0.]];
        assert!(valid_lasso(&bow));
        assert!(encloses(&bow, &[[5., 2.]], &[None], 0.5, off));
        assert!(!encloses(&bow, &[[5., 5.]], &[None], 0.5, off));
        assert!(!valid_lasso(&[[0., 0.], [1., 1.], [2., 2.]]));
        assert!(!valid_lasso(&[[0., 0.], [0., 0.]]));
    }
    #[test]
    fn page_constraints_and_large_origin_transforms() {
        let bounds = ObjectBounds {
            min: [20., 30.],
            max: [80., 90.],
        };
        let moved = move_by(bounds, [-1e5, 1e5], Some([100., 100.]));
        assert_eq!(bounds.placed(moved).min, [0., 40.]);
        assert_eq!(bounds.placed(moved).max, [60., 100.]);
        let resized = scale_about(bounds, [20., 30.], 100., Some([100., 100.]));
        assert!(bounds.placed(resized).max[1] <= 100.);
        for origin in [[0., 0.], [1e12, -1e12]] {
            let p = Placement {
                origin,
                position: [origin[0] + 40., origin[1] - 20.],
                scale: 2.,
                rotation: 0.0,
            };
            let point = [origin[0] + 0.125, origin[1] + 3.5];
            assert_eq!(p.inverse(p.apply(point)), point);
            let q = p.then(
                Placement {
                    origin: p.apply(point),
                    position: p.apply(point),
                    scale: 0.5,
                    rotation: 0.0,
                },
                point,
            );
            assert_eq!(q.apply(point), p.apply(point));
            assert!(q.valid());
        }
    }
}

#[cfg(test)]
mod performance {
    use super::*;
    #[test]
    #[ignore = "manual release-mode indexed lasso benchmark"]
    fn indexed_lasso_performance() {
        let polygon = (0..4000)
            .map(|i| {
                let a = i as f64 / 4000.0 * std::f64::consts::TAU;
                [a.cos() * 1000., a.sin() * 1000.]
            })
            .collect();
        let start = std::time::Instant::now();
        let region = LassoRegion::new(polygon).unwrap();
        eprintln!("4,000-edge lasso setup: {:?}", start.elapsed());
        let path: Vec<_> = (0..100_000)
            .map(|i| [-50. + i as f64 * 0.001, 0.])
            .collect();
        let pressures = vec![None; path.len()];
        let start = std::time::Instant::now();
        assert!(region.encloses(&path, &pressures, 2., PressureStyle::default()));
        eprintln!("100,000-point containment: {:?}", start.elapsed());
    }
}
