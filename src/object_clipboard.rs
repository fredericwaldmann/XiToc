//! Bounded transient object interchange, never a saved native note format.
use crate::{
    drawing::{Drawing, Surface},
    object::{ObjectBounds, Placement, Point},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PREFIX: &str = "XiToc clipboard v1\n";
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_OBJECTS: usize = 512;
pub const MAX_POINTS: usize = 200_000;
use crate::object::ObjectSnapshot as ClipboardObject;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectBundle {
    version: u32,
    pub objects: Vec<ClipboardObject>,
}
impl ObjectBundle {
    #[cfg(test)]
    pub fn capture(strokes: impl Iterator<Item = crate::drawing::Stroke>) -> Result<Self, String> {
        let bundle = Self {
            version: 1,
            objects: strokes.map(Into::into).collect(),
        };
        bundle.validate()?;
        Ok(bundle)
    }
    pub fn capture_objects(objects: impl Iterator<Item = ClipboardObject>) -> Result<Self, String> {
        let bundle = Self {
            version: 1,
            objects: objects.collect(),
        };
        bundle.validate()?;
        Ok(bundle)
    }
    fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.objects.is_empty() || self.objects.len() > MAX_OBJECTS {
            return Err("Unsupported object clipboard or exceeded object limit.".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut count = 0;
        for object in &self.objects {
            if !ids.insert(object.id()) {
                return Err("Duplicate object clipboard ID.".into());
            }
            let s = match object {
                ClipboardObject::Text(t) => {
                    if !t.valid() {
                        return Err("Invalid text clipboard.".into());
                    }
                    count += t.content.text.len();
                    if count > MAX_POINTS {
                        return Err("Text clipboard limit exceeded.".into());
                    }
                    continue;
                }
                ClipboardObject::Connector(c) => {
                    if !c.valid() || c.ends().iter().any(|e| e.target().is_some_and(|id| self.objects.iter().find(|o|o.id()==id).is_some_and(|target| !matches!(target,ClipboardObject::Shape(s) if !s.kind().segment() && s.surface==c.surface && s.document_id==c.document_id)))) {return Err("Invalid connector clipboard relationship.".into());}
                    continue;
                }
                ClipboardObject::Stroke(s) => s,
                ClipboardObject::Shape(s) => {
                    if !s.valid()
                        || s.contours(0.05)
                            .iter()
                            .flatten()
                            .any(|p| p.iter().any(|v| !v.is_finite() || v.abs() > 1e15))
                    {
                        return Err("Invalid shape clipboard.".into());
                    }
                    continue;
                }
            };
            count += s.points.len();
            if s.id.is_nil()
                || s.document_id.is_nil()
                || s.points.is_empty()
                || count > MAX_POINTS
                || !s.placement.valid()
                || !s.width.is_finite()
                || !(0.1..=24.0).contains(&s.width)
                || s.pressure_style.strength > 100
                || !(1..=100).contains(&s.pressure_style.minimum)
                || matches!(s.smoothing,crate::drawing::Smoothing::AdaptiveV1{strength} if strength>100)
                || s.points.iter().any(|p| {
                    p.position
                        .iter()
                        .chain(s.placement.apply(p.position).iter())
                        .any(|v| !v.is_finite() || v.abs() > 1e15)
                        || p.pressure
                            .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
                })
            {
                return Err("Invalid or oversized object clipboard.".into());
            }
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<String, String> {
        self.validate()?;
        let text = format!(
            "{PREFIX}{}",
            serde_json::to_string(self).map_err(|e| e.to_string())?
        );
        if text.len() > MAX_BYTES {
            return Err("Object clipboard exceeds 4 MiB.".into());
        }
        Ok(text)
    }
    pub fn decode(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BYTES {
            return Err("Object clipboard exceeds 4 MiB.".into());
        }
        let json = text
            .strip_prefix(PREFIX)
            .ok_or("Clipboard does not contain XiToc objects. Use Copy on a selection first.")?;
        let bundle: Self =
            serde_json::from_str(json).map_err(|e| format!("Cannot read object clipboard: {e}"))?;
        bundle.validate()?;
        Ok(bundle)
    }
    pub fn bounds(&self) -> ObjectBounds {
        self.objects
            .iter()
            .filter_map(|o| o.as_object().world_bounds())
            .reduce(ObjectBounds::union)
            .expect("validated objects")
    }
    pub fn paste(&self, drawing: &mut Drawing, surface: Surface, target: Point) -> Vec<Uuid> {
        let bounds = self.bounds();
        let center = bounds.center();
        let mut scale: f64 = 1.0;
        let mut position = target;
        if let Some(page) = drawing.bounds(surface) {
            for (i, size) in page.into_iter().enumerate() {
                scale = scale.min((size - 2.0) / (bounds.max[i] - bounds.min[i]).max(1e-9));
            }
            for (i, size) in page.into_iter().enumerate() {
                let radius = (bounds.max[i] - bounds.min[i]) * scale * 0.5;
                position[i] = position[i].clamp(radius + 0.5, size - radius - 0.5);
            }
        }
        let delta = Placement {
            origin: center,
            position,
            scale,
            rotation: 0.0,
            reflected: [false; 2],
        };
        self.insert_placed(drawing, surface, delta)
    }
    pub fn duplicate(&self, drawing: &mut Drawing, surface: Surface) -> Vec<Uuid> {
        let delta = crate::selection::move_by(self.bounds(), [12., 12.], drawing.bounds(surface));
        self.insert_placed(drawing, surface, delta)
    }
    fn insert_placed(
        &self,
        drawing: &mut Drawing,
        surface: Surface,
        delta: Placement,
    ) -> Vec<Uuid> {
        let selected = self.objects.iter().map(|o| o.id()).collect();
        let mut objects: Vec<_> = self
            .objects
            .iter()
            .map(|o| {
                let transformed = match o {
                    ClipboardObject::Connector(c) => c.transformed(delta, &selected).into(),
                    _ => o.transformed(delta),
                };
                transformed.relocated(drawing.owner(), surface)
            })
            .collect();
        let remap = self
            .objects
            .iter()
            .zip(&objects)
            .map(|(old, new)| (old.id(), new.id()))
            .collect();
        for object in &mut objects {
            if let ClipboardObject::Connector(c) = object {
                let mut copy = (**c).clone();
                copy.remap(&remap);
                *object = copy.into();
            }
        }
        let ids = objects.iter().map(ClipboardObject::id).collect();
        if drawing.insert_snapshots(objects) {
            ids
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocumentData, DocumentKind};
    use crate::drawing::Stroke;
    #[test]
    fn clipboard_rejects_unknown_malformed_oversized_and_nonfinite_objects() {
        let doc = DocumentData::blank(DocumentKind::Whiteboard);
        let mut s = Stroke::new(doc.id(), Surface::Canvas, [1, 2, 3, 255], 2.);
        s.push([1., 2.]);
        let b = ObjectBundle::capture(std::iter::once(s.clone())).unwrap();
        let text = b.encode().unwrap();
        assert!(ObjectBundle::decode(&text).is_ok());
        assert!(ObjectBundle::decode("ordinary text").is_err());
        assert!(ObjectBundle::decode(&text.replace("\"version\":1", "\"version\":999")).is_err());
        assert!(
            ObjectBundle::decode(&text.replace("\"kind\":\"Stroke\"", "\"kind\":\"FutureShape\""))
                .is_err()
        );
        assert!(ObjectBundle::decode(&"x".repeat(MAX_BYTES + 1)).is_err());
        assert!(ObjectBundle::capture([s.clone(), s.clone()].into_iter()).is_err());
        s.placement.position = [f64::MAX; 2];
        assert!(ObjectBundle::capture(std::iter::once(s.clone())).is_err());
        s.placement = Placement::default();
        s.points[0].pressure = Some(f32::NAN);
        assert!(ObjectBundle::capture(std::iter::once(s)).is_err());
    }
}
