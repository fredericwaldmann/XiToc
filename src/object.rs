//! Shared semantic object geometry. No UI, IO, platform or PDF dependencies.
use crate::drawing::{Stroke, Surface};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type ObjectId = Uuid;
pub type Point = [f64; 2];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub origin: Point,
    pub position: Point,
    pub scale: f64,
    /// Counterclockwise radians in logical coordinates; absent in older snapshots.
    #[serde(default)]
    pub rotation: f64,
    /// Local axis reflections before rotation; positive scale keeps nib widths valid.
    #[serde(default, skip_serializing_if = "unreflected")]
    pub reflected: [bool; 2],
}
fn unreflected(value: &[bool; 2]) -> bool {
    *value == [false; 2]
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            origin: [0.0; 2],
            position: [0.0; 2],
            scale: 1.0,
            rotation: 0.0,
            reflected: [false; 2],
        }
    }
}
impl Placement {
    pub fn valid(self) -> bool {
        self.origin
            .iter()
            .chain(&self.position)
            .all(|x| x.is_finite())
            && self.rotation.is_finite()
            && self.scale.is_finite()
            && (1e-4..=1e4).contains(&self.scale)
    }
    pub fn is_identity(self) -> bool {
        self.scale == 1.0
            && self.rotation == 0.0
            && self.reflected == [false; 2]
            && self.origin == self.position
    }
    pub fn apply(self, point: Point) -> Point {
        if self.is_identity() {
            return point;
        }
        let offset = std::array::from_fn(|i| {
            (point[i] - self.origin[i]) * self.scale * if self.reflected[i] { -1.0 } else { 1.0 }
        });
        let rotated = rotate(offset, self.rotation);
        std::array::from_fn(|i| self.position[i] + rotated[i])
    }
    pub fn inverse(self, point: Point) -> Point {
        if self.is_identity() {
            return point;
        }
        let offset = std::array::from_fn(|i| point[i] - self.position[i]);
        let rotated = rotate(offset, -self.rotation);
        std::array::from_fn(|i| {
            self.origin[i] + rotated[i] / self.scale * if self.reflected[i] { -1.0 } else { 1.0 }
        })
    }
    /// Re-anchor composition at a raw point to retain precision far from origin.
    pub fn then(self, next: Self, reference: Point) -> Self {
        Self {
            origin: reference,
            position: next.apply(self.apply(reference)),
            scale: self.scale * next.scale,
            // An odd reflection reverses the handedness of the previous rotation.
            rotation: next.rotation
                + if next.reflected[0] ^ next.reflected[1] {
                    -self.rotation
                } else {
                    self.rotation
                },
            reflected: std::array::from_fn(|i| self.reflected[i] ^ next.reflected[i]),
        }
    }
}

fn rotate([x, y]: Point, angle: f64) -> Point {
    if angle == 0.0 {
        return [x, y];
    }
    let (sin, cos) = angle.sin_cos();
    [x * cos - y * sin, x * sin + y * cos]
}

/// Only implemented content kinds belong here; unknown serde tags fail explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)] // Semantic dispatch tag; only implemented content variants belong here.
pub enum ObjectType {
    Group,
    Text,
    Formula,
    Stroke,
    Shape,
    Connector,
}

/// Tagged semantic object snapshot shared with bounded clipboard interchange.
/// M2 must wrap this in a versioned document envelope before PDF persistence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", deny_unknown_fields)]
pub enum ObjectSnapshot {
    Group(std::sync::Arc<crate::group::Group>),
    Text(std::sync::Arc<crate::text::TextObject>),
    Formula(std::sync::Arc<crate::formula::FormulaObject>),
    Stroke(std::sync::Arc<Stroke>),
    Shape(std::sync::Arc<crate::shape::Shape>),
    Connector(std::sync::Arc<crate::connector::Connector>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectBounds {
    pub min: Point,
    pub max: Point,
}
impl ObjectBounds {
    pub fn from_points(points: impl IntoIterator<Item = Point>) -> Option<Self> {
        let mut iter = points.into_iter();
        let first = iter.next()?;
        let mut result = Self {
            min: first,
            max: first,
        };
        for point in iter {
            for (i, value) in point.into_iter().enumerate() {
                result.min[i] = result.min[i].min(value);
                result.max[i] = result.max[i].max(value);
            }
        }
        Some(result)
    }
    pub fn union(self, other: Self) -> Self {
        Self {
            min: std::array::from_fn(|i| self.min[i].min(other.min[i])),
            max: std::array::from_fn(|i| self.max[i].max(other.max[i])),
        }
    }
    pub fn expand(self, radius: f64) -> Self {
        Self {
            min: self.min.map(|v| v - radius),
            max: self.max.map(|v| v + radius),
        }
    }
    pub fn placed(self, p: Placement) -> Self {
        Self::from_points(self.corners().map(|point| p.apply(point))).unwrap()
    }
    pub fn center(self) -> Point {
        std::array::from_fn(|i| self.min[i] + (self.max[i] - self.min[i]) * 0.5)
    }
    pub fn contains(self, point: Point) -> bool {
        (0..2).all(|i| point[i] >= self.min[i] && point[i] <= self.max[i])
    }
    pub fn corners(self) -> [Point; 4] {
        [
            self.min,
            [self.max[0], self.min[1]],
            self.max,
            [self.min[0], self.max[1]],
        ]
    }
}

/// Stroke and Shape share this protocol; future content extends the same store.
/// No dummy Text/Formula/Connector/Image variants exist.
#[allow(dead_code)] // Shared domain API; some operations await future object tools.
pub trait CanvasObject {
    fn object_id(&self) -> ObjectId;
    fn object_type(&self) -> ObjectType;
    /// Geometry hit test in document coordinates, tolerance in PDF points.
    fn hit_test(&self, point: Point, tolerance: f64) -> bool;
    /// Resolve a normalized local-bounds anchor after placement. Future connectors
    /// store the target ID and normalized anchor, never a stale world position.
    fn anchor(&self, normalized: Point) -> Option<Point> {
        if !normalized
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        {
            return None;
        }
        let b = self.local_bounds()?;
        Some(self.placement().apply(std::array::from_fn(|i| {
            b.min[i] + (b.max[i] - b.min[i]) * normalized[i]
        })))
    }
    fn document_id(&self) -> Uuid;
    fn surface(&self) -> Surface;
    fn placement(&self) -> Placement;
    fn local_bounds(&self) -> Option<ObjectBounds>;
    fn geometry_bounds(&self) -> Option<ObjectBounds> {
        self.world_bounds()
    }
    fn world_bounds(&self) -> Option<ObjectBounds> {
        self.local_bounds().map(|b| b.placed(self.placement()))
    }
}
impl CanvasObject for Stroke {
    fn object_type(&self) -> ObjectType {
        ObjectType::Stroke
    }
    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        if self.points.is_empty()
            || !tolerance.is_finite()
            || tolerance < 0.0
            || !point.iter().all(|v| v.is_finite())
            || !self.placement.valid()
        {
            return false;
        }
        let path = self.path();
        let pressures = crate::partial_erase::path_pressures(self, &path)
            .unwrap_or_else(|| vec![self.points[0].pressure; path.len()]);
        hit_stroke_path(&path, &pressures, self, point, point, tolerance)
    }
    fn object_id(&self) -> ObjectId {
        self.id
    }
    fn document_id(&self) -> Uuid {
        self.document_id
    }
    fn surface(&self) -> Surface {
        self.surface
    }
    fn placement(&self) -> Placement {
        self.placement
    }
    fn geometry_bounds(&self) -> Option<ObjectBounds> {
        ObjectBounds::from_points(self.points.iter().map(|p| self.placement.apply(p.position)))
    }
    fn local_bounds(&self) -> Option<ObjectBounds> {
        ObjectBounds::from_points(self.points.iter().map(|p| p.position))
            .map(|b| b.expand(self.width * 0.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocumentData, DocumentKind};
    #[test]
    fn object_identity_placement_history_and_legacy_samples() {
        let doc = DocumentData::blank(DocumentKind::Page);
        let mut drawing = crate::drawing::Drawing::new(&doc);
        let mut stroke = Stroke::new(doc.id(), Surface::Page(0), [1, 2, 3, 89], 4.);
        stroke.push([20., 30.]);
        stroke.push([30., 40.]);
        let id = stroke.object_id();
        assert_eq!(stroke.document_id(), doc.id());
        assert_eq!(stroke.surface(), Surface::Page(0));
        assert_eq!(stroke.world_bounds().unwrap().min, [18., 28.]);
        let original = stroke.clone();
        assert!(drawing.add(stroke));
        let mut json = serde_json::to_value(&original).unwrap();
        json.as_object_mut().unwrap().remove("placement");
        assert_eq!(serde_json::from_value::<Stroke>(json).unwrap(), original);
        let ids = std::collections::HashSet::from([id]);
        assert!(drawing.transform_objects(
            &ids,
            Placement {
                origin: [20., 30.],
                position: [40., 50.],
                scale: 2.,
                rotation: 0.0,
                reflected: [false; 2],
            }
        ));
        let moved = drawing.strokes()[0].clone();
        assert_eq!(moved.id, id);
        assert_eq!(moved.points, original.points);
        assert_eq!(moved.color, original.color);
        assert_eq!(moved.width, original.width);
        assert_eq!(moved.path().first(), Some(&[40., 50.]));
        assert_eq!(
            serde_json::from_str::<Stroke>(&serde_json::to_string(&*moved).unwrap()).unwrap(),
            *moved
        );
        drawing.undo();
        assert_eq!(*drawing.strokes()[0], original);
        drawing.redo();
        assert_eq!(drawing.strokes()[0], moved);
        assert!(!drawing.transform_objects(
            &ids,
            Placement {
                scale: 0.,
                ..Default::default()
            }
        ));
        assert!(!drawing.transform_objects(
            &ids,
            Placement {
                scale: f64::NAN,
                ..Default::default()
            }
        ));
        assert!(!drawing.transform_objects(
            &ids,
            Placement {
                position: [-1000.; 2],
                ..Default::default()
            }
        ));
        assert_eq!(drawing.strokes()[0], moved);
    }
}

#[cfg(test)]
mod group_tests {
    use super::*;
    use crate::{
        document::{DocumentData, DocumentKind},
        drawing::Drawing,
    };
    #[test]
    fn group_transforms_preserve_relative_geometry_identity_and_interleaved_z_order() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let mut drawing = Drawing::new(&data);
        for x in [10., 40., 80.] {
            let mut s = Stroke::new(data.id(), Surface::Canvas, [0, 0, 0, 255], 2.);
            s.push([x, 100.]);
            s.push([x + 10., 110.]);
            assert!(drawing.add(s));
        }
        let original = drawing.strokes().to_vec();
        let ids = std::collections::HashSet::from([original[0].id, original[2].id]);
        let transform = Placement {
            origin: [10., 100.],
            position: [50., 150.],
            scale: 2.,
            rotation: 0.0,
            reflected: [false; 2],
        };
        assert!(drawing.transform_objects(&ids, transform));
        assert!(std::sync::Arc::ptr_eq(&original[1], &drawing.strokes()[1]));
        assert_eq!(
            drawing.strokes().iter().map(|s| s.id).collect::<Vec<_>>(),
            original.iter().map(|s| s.id).collect::<Vec<_>>()
        );
        let a = drawing.strokes()[0].path()[0];
        let b = drawing.strokes()[2].path()[0];
        assert_eq!(b[0] - a[0], 140.);
        assert_eq!(b[1] - a[1], 0.);
        for i in [0, 2] {
            assert_eq!(drawing.strokes()[i].points, original[i].points);
            assert_eq!(drawing.strokes()[i].width, original[i].width);
        }
        let moved = drawing.strokes().to_vec();
        drawing.undo();
        assert_eq!(drawing.strokes(), original);
        drawing.redo();
        assert_eq!(drawing.strokes(), moved);
        drawing.undo();
        assert!(drawing.transform_objects(
            &ids,
            Placement {
                origin: [10., 100.],
                position: [20., 110.],
                scale: 1.,
                rotation: 0.0,
                reflected: [false; 2],
            }
        ));
        assert!(!drawing.can_redo());
    }
}

pub(crate) fn hit_stroke_path(
    path: &[[f64; 2]],
    pressures: &[Option<f32>],
    stroke: &Stroke,
    from: [f64; 2],
    to: [f64; 2],
    radius: f64,
) -> bool {
    if !stroke.pressure_style.enabled || pressures.iter().all(Option::is_none) {
        return crate::drawing::hit_path(path, from, to, radius + stroke.visible_width() * 0.5);
    }
    if path.len() == 1 {
        return crate::drawing::hit_path(
            path,
            from,
            to,
            radius + stroke.visible_width() * 0.5 * stroke.pressure_style.factor(pressures[0]),
        );
    }
    path.windows(2).enumerate().any(|(i, pair)| {
        crate::partial_erase::clip_tapered(
            pair[0],
            pair[1],
            from,
            to,
            radius,
            [
                stroke.visible_width() * 0.5 * stroke.pressure_style.factor(pressures[i]),
                stroke.visible_width() * 0.5 * stroke.pressure_style.factor(pressures[i + 1]),
            ],
        )
        .is_some()
    })
}

#[cfg(test)]
mod architecture_tests {
    use super::*;
    use crate::{
        document::{DocumentData, DocumentKind},
        drawing::{Drawing, Smoothing},
    };
    use std::collections::HashSet;

    fn ink(doc: &DocumentData, surface: Surface) -> Stroke {
        let mut stroke = Stroke::new(doc.id(), surface, [20, 50, 90, 255], 4.);
        stroke.smoothing = Smoothing::PolylineV1;
        stroke.push([20., 30.]);
        stroke.push([40., 30.]);
        stroke
    }
    fn near(a: Point, b: Point) {
        for i in 0..2 {
            assert!((a[i] - b[i]).abs() < 1e-8, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn identity_lookup_ownership_order_and_transaction_history() {
        for (kind, surface) in [
            (DocumentKind::Page, Surface::Page(0)),
            (DocumentKind::Whiteboard, Surface::Canvas),
        ] {
            let doc = DocumentData::blank(kind);
            let mut drawing = Drawing::new(&doc);
            let strokes: Vec<_> = (0..4).map(|_| ink(&doc, surface)).collect();
            let ids: Vec<_> = strokes.iter().map(|s| s.id).collect();
            assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 4);
            assert!(drawing.insert_objects(strokes.clone()));
            assert!(!drawing.add(strokes[0].clone()));
            assert!(drawing.object(Uuid::new_v4()).is_none());
            let object = drawing.object(ids[0]).unwrap();
            assert_eq!(object.object_type(), ObjectType::Stroke);
            assert_eq!(object.surface(), surface);
            assert_eq!(object.document_id(), doc.id());
            assert_eq!(
                drawing
                    .objects()
                    .map(CanvasObject::object_id)
                    .collect::<Vec<_>>(),
                ids
            );
            assert!(drawing.set_z_order(ids[0], 2));
            let order = vec![ids[1], ids[2], ids[0], ids[3]];
            assert_eq!(
                drawing
                    .objects()
                    .map(CanvasObject::object_id)
                    .collect::<Vec<_>>(),
                order
            );
            drawing.undo();
            assert_eq!(
                drawing
                    .objects()
                    .map(CanvasObject::object_id)
                    .collect::<Vec<_>>(),
                ids
            );
            drawing.redo();
            assert_eq!(
                drawing
                    .objects()
                    .map(CanvasObject::object_id)
                    .collect::<Vec<_>>(),
                order
            );
            assert!(!drawing.set_z_order(ids[0], 99));
            assert!(!drawing.set_z_order(ids[0], 2));
            drawing.undo();
            assert!(drawing.set_z_order(ids[3], 0));
            assert!(!drawing.can_redo());
            let mut foreign = ink(&doc, surface);
            foreign.document_id = Uuid::new_v4();
            assert!(!drawing.add(foreign));
            assert!(!drawing.add(ink(&doc, Surface::Page(99))));
            if kind == DocumentKind::Page {
                assert!(!drawing.add(ink(&doc, Surface::Canvas)));
            }
        }
    }

    #[test]
    fn rotation_bounds_anchors_and_composition_preserve_raw_ink() {
        let doc = DocumentData::blank(DocumentKind::Whiteboard);
        let mut drawing = Drawing::new(&doc);
        let stroke = ink(&doc, Surface::Canvas);
        let id = stroke.id;
        assert!(drawing.add(stroke.clone()));
        let turn = Placement {
            origin: [20., 30.],
            position: [100., 200.],
            scale: 2.,
            rotation: std::f64::consts::FRAC_PI_2,
            reflected: [false; 2],
        };
        assert!(drawing.transform_objects(&HashSet::from([id]), turn));
        let object = drawing.object(id).unwrap();
        let b = object.world_bounds().unwrap();
        near(b.min, [96., 196.]);
        near(b.max, [104., 244.]);
        near(object.anchor([0.5, 0.5]).unwrap(), [100., 220.]);
        near(object.anchor([1., 0.5]).unwrap(), [100., 244.]);
        assert!(object.anchor([2., 0.]).is_none());
        assert!(object.hit_test([100., 220.], 0.));
        assert!(!object.hit_test([105., 220.], 0.));
        assert!(object.hit_test([105., 220.], 2.));
        let moved = drawing.strokes()[0].clone();
        assert_eq!(moved.points, stroke.points);
        assert_eq!(moved.width, stroke.width);
        assert_eq!(moved.smoothing, stroke.smoothing);
        assert_eq!(moved.color, stroke.color);
        let next = Placement {
            origin: [100., 200.],
            position: [-30., 70.],
            scale: 0.5,
            rotation: -0.3,
            reflected: [false; 2],
        };
        let composed = turn.then(next, [20., 30.]);
        for point in [[20., 30.], [40., 30.], [-7., 99.]] {
            near(turn.inverse(turn.apply(point)), point);
            near(composed.apply(point), next.apply(turn.apply(point)));
        }
        assert_eq!(
            serde_json::from_str::<Stroke>(&serde_json::to_string(&*moved).unwrap()).unwrap(),
            *moved
        );
        drawing.undo();
        assert_eq!(*drawing.strokes()[0], stroke);
        drawing.redo();
        assert_eq!(drawing.strokes()[0], moved);
        assert!(!drawing.transform_objects(
            &HashSet::from([id]),
            Placement {
                rotation: f64::NAN,
                ..Default::default()
            }
        ));
    }

    #[test]
    fn pressure_hits_dots_and_legacy_placement() {
        let doc = DocumentData::blank(DocumentKind::Whiteboard);
        let mut stroke = ink(&doc, Surface::Canvas);
        stroke.width = 10.;
        stroke.pressure_style.enabled = true;
        stroke.points[0].pressure = Some(0.);
        stroke.points[1].pressure = Some(1.);
        assert!(!stroke.hit_test([20., 34.], 0.));
        assert!(stroke.hit_test([40., 34.], 0.));
        stroke.points.truncate(1);
        assert!(stroke.hit_test([20., 30.5], 0.));
        assert!(!stroke.hit_test([20., 32.], 0.));
        assert!(!stroke.hit_test([f64::NAN, 0.], 1.));
        assert!(!stroke.hit_test([20., 30.], -1.));
        let mut json = serde_json::to_value(&stroke).unwrap();
        json["placement"]
            .as_object_mut()
            .unwrap()
            .remove("rotation");
        assert_eq!(serde_json::from_value::<Stroke>(json).unwrap(), stroke);
        assert!(serde_json::from_str::<ObjectType>("\"FutureShape\"").is_err());
    }
}

#[cfg(test)]
mod reflection_tests {
    use super::*;
    fn near(a: Point, b: Point) {
        for i in 0..2 {
            assert!((a[i] - b[i]).abs() < 0.001, "{a:?} != {b:?}");
        }
    }
    #[test]
    fn reflection_composition_inverse_and_anchors_for_all_parities() {
        for far in [0., 1e12] {
            for reflection in [[false, false], [true, false], [false, true], [true, true]] {
                for next_reflection in [[false, false], [true, false], [false, true], [true, true]]
                {
                    let a = Placement {
                        origin: [far, far],
                        position: [far + 20., far - 40.],
                        rotation: 0.31,
                        scale: 1.7,
                        reflected: reflection,
                    };
                    let b = Placement {
                        origin: [far - 20., far + 40.],
                        position: [far + 50., far - 50.],
                        rotation: -0.8,
                        scale: 0.6,
                        reflected: next_reflection,
                    };
                    let reference = [far + 1., far + 2.];
                    let composed = a.then(b, reference);
                    assert!(composed.valid());
                    for point in [reference, [far + 7., far - 3.], [far - 100., far + 20.]] {
                        near(a.inverse(a.apply(point)), point);
                        near(composed.apply(point), b.apply(a.apply(point)));
                        near(composed.inverse(composed.apply(point)), point);
                    }
                }
            }
        }
        let doc = crate::document::DocumentData::blank(crate::document::DocumentKind::Whiteboard);
        let mut stroke = Stroke::new(doc.id(), Surface::Canvas, [1, 2, 3, 255], 2.);
        stroke.push([20., 30.]);
        stroke.push([40., 50.]);
        stroke.placement = Placement {
            origin: [30., 40.],
            position: [30., 40.],
            reflected: [true, false],
            ..Default::default()
        };
        near(stroke.anchor([0., 0.]).unwrap(), [41., 29.]);
        assert!(!stroke.placement.is_identity());
    }
    #[test]
    fn old_snapshots_default_to_unreflected_and_reflections_round_trip() {
        let mut json = serde_json::to_value(Placement::default()).unwrap();
        json.as_object_mut().unwrap().remove("reflected");
        assert_eq!(
            serde_json::from_value::<Placement>(json).unwrap(),
            Placement::default()
        );
        for reflected in [[true, false], [false, true], [true, true]] {
            let p = Placement {
                reflected,
                ..Default::default()
            };
            let encoded = serde_json::to_string(&p).unwrap();
            assert_eq!(serde_json::from_str::<Placement>(&encoded).unwrap(), p);
        }
        assert!(
            serde_json::from_str::<Placement>(
                r#"{"origin":[0,0],"position":[0,0],"scale":1,"rotation":0,"reflected":[true]}"#
            )
            .is_err()
        );
    }
}

impl From<Stroke> for ObjectSnapshot {
    fn from(value: Stroke) -> Self {
        Self::Stroke(std::sync::Arc::new(value))
    }
}
impl From<crate::shape::Shape> for ObjectSnapshot {
    fn from(value: crate::shape::Shape) -> Self {
        Self::Shape(std::sync::Arc::new(value))
    }
}
impl From<crate::connector::Connector> for ObjectSnapshot {
    fn from(c: crate::connector::Connector) -> Self {
        Self::Connector(std::sync::Arc::new(c))
    }
}
impl From<crate::formula::FormulaObject> for ObjectSnapshot {
    fn from(value: crate::formula::FormulaObject) -> Self {
        Self::Formula(std::sync::Arc::new(value))
    }
}
impl ObjectSnapshot {
    pub fn locked(&self) -> bool {
        match self {
            Self::Group(o) => o.locked,
            Self::Stroke(o) => o.locked,
            Self::Shape(o) => o.locked,
            Self::Text(o) => o.locked,
            Self::Formula(o) => o.locked,
            Self::Connector(o) => o.locked,
        }
    }
    pub fn set_locked(&mut self, value: bool) {
        match self {
            Self::Group(o) => std::sync::Arc::make_mut(o).locked = value,
            Self::Stroke(o) => std::sync::Arc::make_mut(o).locked = value,
            Self::Shape(o) => std::sync::Arc::make_mut(o).locked = value,
            Self::Text(o) => std::sync::Arc::make_mut(o).locked = value,
            Self::Formula(o) => std::sync::Arc::make_mut(o).locked = value,
            Self::Connector(o) => std::sync::Arc::make_mut(o).locked = value,
        }
    }

    pub fn as_object(&self) -> &dyn CanvasObject {
        match self {
            Self::Group(g) => g.as_ref(),
            Self::Text(t) => t.as_ref(),
            Self::Formula(f) => f.as_ref(),
            Self::Stroke(s) => s.as_ref(),
            Self::Shape(s) => s.as_ref(),
            Self::Connector(c) => c.as_ref(),
        }
    }
    pub fn id(&self) -> ObjectId {
        match self {
            Self::Group(g) => g.id,
            Self::Text(t) => t.id,
            Self::Formula(f) => f.id,
            Self::Stroke(s) => s.id,
            Self::Shape(s) => s.id,
            Self::Connector(c) => c.id,
        }
    }
    pub fn transformed(&self, delta: Placement) -> Self {
        match self {
            Self::Group(_) => self.clone(),
            Self::Text(t) => t.transformed(delta).into(),
            Self::Formula(f) => f.transformed(delta).into(),
            Self::Connector(c) => c.transformed(delta, &Default::default()).into(),
            Self::Stroke(s) => {
                let mut t = (**s).clone();
                t.placement = s.placement.then(delta, s.points[0].position);
                t.into()
            }
            Self::Shape(s) => {
                let mut t = (**s).clone();
                t.placement = s.placement.then(delta, [0.; 2]);
                t.into()
            }
        }
    }
    pub fn relocated(&self, owner: Uuid, surface: crate::drawing::Surface) -> Self {
        match self {
            Self::Group(g) => {
                let mut g = (**g).clone();
                g.id = Uuid::new_v4();
                g.document_id = owner;
                g.surface = surface;
                Self::Group(std::sync::Arc::new(g))
            }
            Self::Text(t) => {
                let mut t = (**t).clone();
                t.id = Uuid::new_v4();
                t.document_id = owner;
                t.surface = surface;
                t.into()
            }
            Self::Formula(f) => {
                let mut f = (**f).clone();
                f.id = Uuid::new_v4();
                f.document_id = owner;
                f.surface = surface;
                f.into()
            }
            Self::Connector(c) => {
                let mut c = (**c).clone();
                c.id = Uuid::new_v4();
                c.document_id = owner;
                c.surface = surface;
                c.into()
            }
            Self::Stroke(s) => {
                let mut t = (**s).clone();
                t.id = Uuid::new_v4();
                t.document_id = owner;
                t.surface = surface;
                t.into()
            }
            Self::Shape(s) => {
                let mut t = (**s).clone();
                t.id = Uuid::new_v4();
                t.document_id = owner;
                t.surface = surface;
                t.into()
            }
        }
    }
}

impl From<crate::text::TextObject> for ObjectSnapshot {
    fn from(t: crate::text::TextObject) -> Self {
        Self::Text(std::sync::Arc::new(t))
    }
}
