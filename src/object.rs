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
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            origin: [0.0; 2],
            position: [0.0; 2],
            scale: 1.0,
        }
    }
}
impl Placement {
    pub fn valid(self) -> bool {
        self.origin
            .iter()
            .chain(&self.position)
            .all(|x| x.is_finite())
            && self.scale.is_finite()
            && (1e-4..=1e4).contains(&self.scale)
    }
    pub fn is_identity(self) -> bool {
        self.scale == 1.0 && self.origin == self.position
    }
    pub fn apply(self, point: Point) -> Point {
        if self.is_identity() {
            return point;
        }
        std::array::from_fn(|i| self.position[i] + (point[i] - self.origin[i]) * self.scale)
    }
    pub fn inverse(self, point: Point) -> Point {
        if self.is_identity() {
            return point;
        }
        std::array::from_fn(|i| self.origin[i] + (point[i] - self.position[i]) / self.scale)
    }
    /// Re-anchor composition at a raw point to retain precision far from origin.
    pub fn then(self, next: Self, reference: Point) -> Self {
        Self {
            origin: reference,
            position: next.apply(self.apply(reference)),
            scale: self.scale * next.scale,
        }
    }
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
        Self {
            min: p.apply(self.min),
            max: p.apply(self.max),
        }
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

/// Future semantic content types implement this same protocol. Stroke storage is
/// the first adapter; no dummy Shape/Text/Formula/Connector/Image variants exist.
pub trait CanvasObject {
    fn object_id(&self) -> ObjectId;
    fn document_id(&self) -> Uuid;
    fn surface(&self) -> Surface;
    fn placement(&self) -> Placement;
    fn local_bounds(&self) -> Option<ObjectBounds>;
    fn world_bounds(&self) -> Option<ObjectBounds> {
        self.local_bounds().map(|b| b.placed(self.placement()))
    }
}
impl CanvasObject for Stroke {
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
                scale: 2.
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
                scale: 1.
            }
        ));
        assert!(!drawing.can_redo());
    }
}
