//! An editable file card. Names and layout belong to the object, not its content hash.
use crate::{
    asset::BinaryAsset,
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectId, ObjectType, Placement, Point},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    pub id: ObjectId,
    pub document_id: Uuid,
    pub surface: Surface,
    pub placement: Placement,
    pub dimensions: Point,
    pub original_name: String,
    pub display_name: String,
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listing: Option<crate::code_listing::Listing>,
    pub asset: Arc<BinaryAsset>,
}

pub fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= 1024
        && !name.chars().any(|c| {
            c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
}

pub fn original_name(name: &str) -> Option<&str> {
    let basename = name.rsplit(['/', '\\']).next()?;
    (valid_name(basename) && !matches!(basename, "." | "..")).then_some(basename)
}

impl Attachment {
    pub fn new(
        owner: Uuid,
        surface: Surface,
        position: Point,
        name: String,
        asset: Arc<BinaryAsset>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            document_id: owner,
            surface,
            placement: Placement {
                position,
                ..Default::default()
            },
            dimensions: [176., 36.],
            display_name: name.clone(),
            original_name: name,
            locked: false,
            listing: None,
            asset,
        }
    }

    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && !self.document_id.is_nil()
            && self.placement.valid()
            && original_name(&self.original_name) == Some(self.original_name.as_str())
            && valid_name(&self.display_name)
            && self
                .listing
                .as_ref()
                .is_none_or(|listing| listing.valid() && self.asset.listing_text())
            && self
                .dimensions
                .iter()
                .all(|n| n.is_finite() && (16.0..=1e6).contains(n))
            && self
                .corners()
                .iter()
                .flatten()
                .all(|n| n.is_finite() && n.abs() <= 1e15)
    }

    pub fn corners(&self) -> [Point; 4] {
        self.local_bounds()
            .unwrap()
            .corners()
            .map(|point| self.placement.apply(point))
    }

    pub fn handles(&self) -> [Point; 8] {
        let bounds = self.local_bounds().unwrap();
        let corners = bounds.corners();
        let edges = bounds.edge_midpoints();
        std::array::from_fn(|i| {
            self.placement.apply(if i.is_multiple_of(2) {
                corners[i / 2]
            } else {
                edges[i / 2]
            })
        })
    }

    pub fn resized(&self, index: usize, point: Point) -> Self {
        let fixed = self.placement.inverse(self.handles()[(index + 4) % 8]);
        let initial = self.placement.inverse(self.handles()[index]);
        let target = self.placement.inverse(point);
        let mut out = self.clone();
        let mut min = [0.; 2];
        for axis in 0..2 {
            let direction = initial[axis] - fixed[axis];
            if direction.abs() > 1e-7 {
                let extent = ((target[axis] - fixed[axis]) * direction.signum()).clamp(
                    if self.listing.is_some() {
                        if axis == 0 { 100. } else { 64. }
                    } else if axis == 0 {
                        48.
                    } else {
                        20.
                    },
                    1e6,
                );
                out.dimensions[axis] = extent;
                if direction < 0. {
                    min[axis] = fixed[axis] - extent;
                }
            }
        }
        out.placement.origin = [0.; 2];
        out.placement.position = self.placement.apply(min);
        out
    }

    pub fn extension(&self) -> String {
        self.original_name
            .rsplit_once('.')
            .map_or(String::new(), |(_, ext)| ext.to_ascii_lowercase())
    }
}

impl CanvasObject for Attachment {
    fn object_id(&self) -> ObjectId {
        self.id
    }
    fn object_type(&self) -> ObjectType {
        if self.listing.is_some() {
            ObjectType::CodeListing
        } else {
            ObjectType::Attachment
        }
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
            max: self.dimensions,
        })
    }
    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        self.local_bounds()
            .unwrap()
            .expand(tolerance / self.placement.scale)
            .contains(self.placement.inverse(point))
    }
}
