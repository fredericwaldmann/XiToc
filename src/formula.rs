//! Portable standalone mathematical formula objects. Renderer caches live in
//! the canvas adapter; this model keeps source and geometry only.
use crate::{
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectId, ObjectType, Placement, Point},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_FORMULA_BYTES: usize = 16 * 1024;
pub const MAX_FORMULA_TOKENS: usize = 128;
pub const FORMULA_FONT_SIZE: f64 = 24.0;
pub const MIN_FORMULA_SCALE: f64 = 0.05;
pub const MAX_FORMULA_SCALE: f64 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormulaLayout {
    pub width: f64,
    pub height: f64,
    /// Baseline measured from the local top edge, in document points.
    pub baseline: f64,
}
impl Default for FormulaLayout {
    fn default() -> Self {
        Self {
            width: 32.0,
            height: 28.0,
            baseline: 20.0,
        }
    }
}
impl FormulaLayout {
    pub fn valid(self) -> bool {
        [self.width, self.height, self.baseline]
            .iter()
            .all(|v| v.is_finite())
            && (1.0..=1e6).contains(&self.width)
            && (1.0..=1e6).contains(&self.height)
            && (0.0..=self.height).contains(&self.baseline)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormulaObject {
    #[serde(default)]
    pub locked: bool,
    pub id: ObjectId,
    pub document_id: Uuid,
    pub surface: Surface,
    pub placement: Placement,
    /// Exact user-authored UTF-8 math source. Never regenerated from glyphs.
    pub latex_source: String,
    pub color: [u8; 4],
    /// Last successfully measured renderer bounds; caches are always rebuildable.
    pub layout: FormulaLayout,
}
impl FormulaObject {
    pub fn new(
        owner: Uuid,
        surface: Surface,
        position: Point,
        source: String,
        color: [u8; 4],
    ) -> Self {
        Self {
            locked: false,
            id: Uuid::new_v4(),
            document_id: owner,
            surface,
            placement: Placement {
                position,
                ..Default::default()
            },
            latex_source: source,
            color,
            layout: FormulaLayout::default(),
        }
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && !self.document_id.is_nil()
            && self.placement.valid()
            && (MIN_FORMULA_SCALE..=MAX_FORMULA_SCALE).contains(&self.placement.scale)
            && self.placement.reflected == [false; 2]
            && self
                .placement
                .origin
                .iter()
                .chain(&self.placement.position)
                .all(|v| v.is_finite() && v.abs() <= 1e15)
            && self.latex_source.len() <= MAX_FORMULA_BYTES
            && !self.latex_source.contains('\0')
            && self.layout.valid()
    }
    /// Reflection relocates the formula center while keeping notation readable.
    pub fn transformed(&self, delta: Placement) -> Self {
        let mut next = self.clone();
        let center = delta.apply(
            self.placement
                .apply([self.layout.width * 0.5, self.layout.height * 0.5]),
        );
        next.placement.scale = self.placement.scale * delta.scale;
        next.placement.rotation = self.placement.rotation + delta.rotation;
        next.placement.origin = [0.0; 2];
        next.placement.position = [0.0; 2];
        next.placement.reflected = [false; 2];
        let half = [self.layout.width * 0.5, self.layout.height * 0.5];
        let local_center = next.placement.apply(half);
        next.placement.position = [center[0] - local_center[0], center[1] - local_center[1]];
        next
    }
    pub fn rectangle_matches(&self, bounds: ObjectBounds, crossing: bool) -> bool {
        let corners = self.world_bounds().unwrap().corners();
        let mut polygon = corners.to_vec();
        polygon.push(corners[0]);
        crate::selection::rectangle_matches(
            bounds,
            crossing,
            &polygon,
            &[],
            0.0,
            Default::default(),
        ) || (crossing
            && bounds
                .corners()
                .iter()
                .any(|p| crate::selection::inside(*p, &corners)))
    }
    pub fn lasso_matches(&self, region: &crate::selection::LassoRegion) -> bool {
        let corners = self.world_bounds().unwrap().corners();
        let mut polygon = corners.to_vec();
        polygon.push(corners[0]);
        region.encloses(&polygon, &[], 0.0, Default::default())
    }
}
impl CanvasObject for FormulaObject {
    fn object_id(&self) -> ObjectId {
        self.id
    }
    fn object_type(&self) -> ObjectType {
        ObjectType::Formula
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
        Some(ObjectBounds {
            min: [0.0; 2],
            max: [self.layout.width, self.layout.height],
        })
    }
    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        if !self.valid() || !tolerance.is_finite() || tolerance < 0.0 {
            return false;
        }
        let p = self.placement.inverse(point);
        let t = tolerance / self.placement.scale;
        p[0] >= -t && p[1] >= -t && p[0] <= self.layout.width + t && p[1] <= self.layout.height + t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocumentData, DocumentKind};
    #[test]
    fn formula_geometry_transform_and_readable_flip_preserve_exact_source() {
        let doc = DocumentData::blank(DocumentKind::Whiteboard);
        let mut f = FormulaObject::new(
            doc.id(),
            Surface::Canvas,
            [50.0, 70.0],
            r"\frac{x_1}{\sqrt{2}}".into(),
            [12, 34, 56, 255],
        );
        f.layout = FormulaLayout {
            width: 42.0,
            height: 36.0,
            baseline: 26.0,
        };
        assert!(f.valid());
        let original = f.latex_source.clone();
        let center = f.world_bounds().unwrap().center();
        let flipped = f.transformed(Placement {
            origin: center,
            position: [150.0, 120.0],
            reflected: [true, false],
            ..Default::default()
        });
        assert_eq!(flipped.latex_source, original);
        assert_eq!(flipped.placement.reflected, [false; 2]);
        assert!((flipped.world_bounds().unwrap().center()[0] - 150.0).abs() < 1e-9);
        assert!(flipped.hit_test([155.0, 120.0], 0.0));
    }
}
