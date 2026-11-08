//! Portable editable text semantics. Layout, caret and IME state are adapters.
use crate::{
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectId, ObjectType, Placement, Point},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub const MAX_TEXT_BYTES: usize = 65536;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextFamily {
    Sans,
    Monospace,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    Left,
    Center,
    Right,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextStyle {
    pub family: TextFamily,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: [u8; 4],
    pub alignment: Alignment,
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: TextFamily::Sans,
            size: 18.,
            bold: false,
            italic: false,
            underline: false,
            color: [32, 39, 49, 255],
            alignment: Alignment::Left,
        }
    }
}
impl TextStyle {
    pub fn valid(&self) -> bool {
        self.size.is_finite() && (4.0..=512.).contains(&self.size)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextContent {
    pub text: String,
    pub style: TextStyle,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextObject {
    pub id: ObjectId,
    pub document_id: Uuid,
    pub surface: Surface,
    pub placement: Placement,
    pub width: f64,
    pub height: f64,
    pub content: std::sync::Arc<TextContent>,
}
impl TextObject {
    pub fn content_mut(&mut self) -> &mut TextContent {
        std::sync::Arc::make_mut(&mut self.content)
    }
    pub fn new(owner: Uuid, surface: Surface, point: Point, width: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            document_id: owner,
            surface,
            placement: Placement {
                position: point,
                ..Default::default()
            },
            width,
            height: 26.,
            content: Default::default(),
        }
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && !self.document_id.is_nil()
            && self.placement.valid()
            && self.placement.reflected == [false; 2]
            && self
                .placement
                .position
                .iter()
                .chain(&self.placement.origin)
                .all(|v| v.abs() <= 1e15)
            && self.width.is_finite()
            && (24.0..=1e6).contains(&self.width)
            && self.height.is_finite()
            && (1.0..=1e7).contains(&self.height)
            && self.content.style.valid()
            && self.content.text.len() <= MAX_TEXT_BYTES
            && !self.content.text.contains('\0')
    }
    pub fn corners(&self) -> [Point; 4] {
        self.local_bounds()
            .unwrap()
            .corners()
            .map(|p| self.placement.apply(p))
    }
    /// Group scaling scales type uniformly. Reflection moves centers, retaining
    /// readable glyph orientation; explicit rotation rotates the text normally.
    pub fn transformed(&self, delta: Placement) -> Self {
        let mut out = self.clone();
        let center = delta.apply(self.placement.apply([self.width * 0.5, self.height * 0.5]));
        out.placement.scale *= delta.scale;
        out.placement.rotation += delta.rotation;
        out.placement.origin = [0.; 2];
        out.placement.position = [0.; 2];
        let v = out.placement.apply([out.width * 0.5, out.height * 0.5]);
        out.placement.position = std::array::from_fn(|i| center[i] - v[i]);
        out
    }
    pub fn rectangle_matches(&self, b: ObjectBounds, crossing: bool) -> bool {
        let mut p = self.corners().to_vec();
        p.push(p[0]);
        crate::selection::rectangle_matches(b, crossing, &p, &[], 0., Default::default())
            || (crossing
                && b.corners()
                    .iter()
                    .any(|p| crate::selection::inside(*p, &self.corners())))
    }
    pub fn lasso_matches(&self, region: &crate::selection::LassoRegion) -> bool {
        let mut p = self.corners().to_vec();
        p.push(p[0]);
        region.encloses(&p, &[], 0., Default::default())
    }
}
impl CanvasObject for TextObject {
    fn object_id(&self) -> ObjectId {
        self.id
    }
    fn object_type(&self) -> ObjectType {
        ObjectType::Text
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
            min: [0.; 2],
            max: [self.width, self.height],
        })
    }
    fn hit_test(&self, p: Point, tolerance: f64) -> bool {
        if !self.valid() {
            return false;
        }
        let p = self.placement.inverse(p);
        let t = tolerance / self.placement.scale;
        p[0] >= -t && p[1] >= -t && p[0] <= self.width + t && p[1] <= self.height + t
    }
}

#[derive(Default)]
pub struct TextStylePatch {
    pub family: Option<TextFamily>,
    pub size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub rgb: Option<[u8; 3]>,
    pub alpha: Option<u8>,
    pub alignment: Option<Alignment>,
}
impl TextStylePatch {
    pub fn apply(&self, style: &mut TextStyle) {
        if let Some(v) = self.family {
            style.family = v;
        }
        if let Some(v) = self.size {
            style.size = v;
        }
        if let Some(v) = self.bold {
            style.bold = v;
        }
        if let Some(v) = self.italic {
            style.italic = v;
        }
        if let Some(v) = self.underline {
            style.underline = v;
        }
        if let Some(v) = self.rgb {
            style.color[..3].copy_from_slice(&v);
        }
        if let Some(v) = self.alpha {
            style.color[3] = v;
        }
        if let Some(v) = self.alignment {
            style.alignment = v;
        }
    }
}
