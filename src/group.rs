//! Semantic organization: ordered ownership, with derived world bounds.
use crate::{
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectId, ObjectType, Placement, Point},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: ObjectId,
    pub document_id: Uuid,
    pub surface: Surface,
    pub children: Vec<ObjectId>,
    #[serde(default)]
    pub locked: bool,
    /// Orientation of the container. Descendants retain baked world placements.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: f64,
    /// Rebuilt from descendants at transaction boundaries, never saved content.
    #[serde(skip)]
    pub bounds: Option<ObjectBounds>,
}
impl CanvasObject for Group {
    fn object_id(&self) -> ObjectId {
        self.id
    }
    fn object_type(&self) -> ObjectType {
        ObjectType::Group
    }
    fn document_id(&self) -> Uuid {
        self.document_id
    }
    fn surface(&self) -> Surface {
        self.surface
    }
    fn placement(&self) -> Placement {
        Placement::default()
    }
    fn local_bounds(&self) -> Option<ObjectBounds> {
        self.bounds
    }
    // Picking uses descendant geometry in Drawing, never this broad-phase box.
    fn hit_test(&self, _: Point, _: f64) -> bool {
        false
    }
}

fn is_zero(angle: &f64) -> bool {
    *angle == 0.0
}

/// Validate a bounded, single-owner forest. Iterative walks reject cycles and
/// missing children before any clipboard or document insertion is committed.
pub fn validate(objects: &[crate::object::ObjectSnapshot]) -> bool {
    use std::collections::{HashMap, HashSet};
    let slots: HashMap<_, _> = objects.iter().map(|o| (o.id(), o)).collect();
    if slots.len() != objects.len() {
        return false;
    }
    let mut parents = HashMap::new();
    for object in objects {
        if let crate::object::ObjectSnapshot::Group(g) = object {
            if g.id.is_nil()
                || g.document_id.is_nil()
                || g.children.is_empty()
                || !g.rotation.is_finite()
            {
                return false;
            }
            for child in &g.children {
                let Some(o) = slots.get(child) else {
                    return false;
                };
                if parents.insert(*child, g.id).is_some()
                    || o.as_object().surface() != g.surface
                    || o.as_object().document_id() != g.document_id
                {
                    return false;
                }
            }
        }
    }
    for id in parents.keys() {
        let mut visited = HashSet::new();
        let mut current = *id;
        while let Some(parent) = parents.get(&current) {
            if !visited.insert(current) || visited.len() > 64 {
                return false;
            }
            current = *parent;
        }
    }
    true
}
