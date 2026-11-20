//! Bounded transient object interchange, never a saved native note format.
use crate::{
    drawing::{Drawing, Surface},
    object::{ObjectBounds, Placement, Point},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PREFIX: &str = "XiToc clipboard v1\n";
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_OBJECTS: usize = 512;
pub const MAX_POINTS: usize = 200_000;
use crate::object::ObjectSnapshot as ClipboardObject;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectBundle {
    version: u32,
    pub objects: Vec<ClipboardObject>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageReference {
    id: Uuid,
    document_id: Uuid,
    surface: Surface,
    placement: Placement,
    dimensions: Point,
    opacity: u8,
    // Older readers expect this field. New readers always use the Shift modifier.
    #[serde(default)]
    preserve_aspect: bool,
    locked: bool,
    asset_id: crate::image_object::AssetId,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageBundle {
    version: u32,
    objects: Vec<serde_json::Value>,
    assets: Vec<std::sync::Arc<crate::image_object::ImageAsset>>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentReference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    listing: Option<crate::code_listing::Listing>,
    id: Uuid,
    document_id: Uuid,
    surface: Surface,
    placement: Placement,
    dimensions: Point,
    original_name: String,
    display_name: String,
    locked: bool,
    asset_id: crate::asset::AssetId,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetBundle {
    version: u32,
    objects: Vec<serde_json::Value>,
    assets: Vec<std::sync::Arc<crate::asset::BinaryAsset>>,
}

impl From<&crate::image_object::ImageObject> for ImageReference {
    fn from(image: &crate::image_object::ImageObject) -> Self {
        Self {
            id: image.id,
            document_id: image.document_id,
            surface: image.surface,
            placement: image.placement,
            dimensions: image.dimensions,
            opacity: image.opacity,
            preserve_aspect: false,
            locked: image.locked,
            asset_id: image.asset.id(),
        }
    }
}
impl ImageReference {
    fn resolve(self, asset: std::sync::Arc<crate::image_object::ImageAsset>) -> ClipboardObject {
        crate::image_object::ImageObject {
            id: self.id,
            document_id: self.document_id,
            surface: self.surface,
            placement: self.placement,
            dimensions: self.dimensions,
            opacity: self.opacity,
            locked: self.locked,
            asset,
        }
        .into()
    }
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
        if !crate::group::validate(&self.objects) {
            return Err("Invalid group ownership or cycle in clipboard.".into());
        }
        let mut ids = std::collections::HashSet::new();
        let mut count = 0;
        for object in &self.objects {
            if !ids.insert(object.id()) {
                return Err("Duplicate object clipboard ID.".into());
            }
            let s = match object {
                ClipboardObject::Attachment(a) => {
                    if !a.valid() {
                        return Err("Invalid attachment clipboard object.".into());
                    }
                    continue;
                }
                ClipboardObject::Image(i) => {
                    if !i.valid() {
                        return Err("Invalid image clipboard object.".into());
                    }
                    continue;
                }
                ClipboardObject::Group(_) => continue,
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
                ClipboardObject::Coordinate(value) => {
                    if !value.valid() {
                        return Err("Invalid scientific coordinate system.".into());
                    }
                    continue;
                }
                ClipboardObject::Formula(formula) => {
                    if !formula.valid() {
                        return Err("Invalid formula clipboard object.".into());
                    }
                    count += formula.latex_source.len();
                    if count > MAX_BYTES {
                        return Err("Formula clipboard limit exceeded.".into());
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
        let json = if self
            .objects
            .iter()
            .any(|o| matches!(o, ClipboardObject::Attachment(_)))
        {
            self.encode_assets()?
        } else if self
            .objects
            .iter()
            .any(|o| matches!(o, ClipboardObject::Image(_)))
        {
            let mut assets = std::collections::HashMap::new();
            let mut objects = Vec::new();
            for o in &self.objects {
                if let ClipboardObject::Image(i) = o {
                    assets
                        .entry(i.asset.id())
                        .or_insert_with(|| i.asset.clone());
                    let data = ImageReference::from(i.as_ref());
                    objects.push(serde_json::json!({"kind":"Image","data":data}));
                } else {
                    objects.push(serde_json::to_value(o).map_err(|e| e.to_string())?);
                }
            }
            if assets.values().map(|a| a.bytes().len()).sum::<usize>() > MAX_BYTES / 2 {
                return Err(
                    "Image copy exceeds system clipboard limit; use the XiToc session clipboard."
                        .into(),
                );
            }
            serde_json::to_string(&ImageBundle {
                version: 2,
                objects,
                assets: assets.into_values().collect(),
            })
            .map_err(|e| e.to_string())?
        } else {
            serde_json::to_string(self).map_err(|e| e.to_string())?
        };
        let text = format!("{PREFIX}{json}");
        if text.len() > MAX_BYTES {
            return Err("Object clipboard exceeds 64 MiB.".into());
        }
        Ok(text)
    }
    fn encode_assets(&self) -> Result<String, String> {
        let mut assets = std::collections::HashMap::new();
        let mut objects = Vec::with_capacity(self.objects.len());
        for object in &self.objects {
            let value = match object {
                ClipboardObject::Attachment(a) => {
                    assets
                        .entry(a.asset.id())
                        .or_insert_with(|| a.asset.clone());
                    let data = AttachmentReference {
                        listing: a.listing.clone(),
                        id: a.id,
                        document_id: a.document_id,
                        surface: a.surface,
                        placement: a.placement,
                        dimensions: a.dimensions,
                        original_name: a.original_name.clone(),
                        display_name: a.display_name.clone(),
                        locked: a.locked,
                        asset_id: a.asset.id(),
                    };
                    serde_json::json!({"kind":"Attachment","data":data})
                }
                ClipboardObject::Image(i) => {
                    assets
                        .entry(i.asset.id())
                        .or_insert_with(|| i.asset.binary.clone());
                    serde_json::json!({"kind":"Image","data":ImageReference::from(i.as_ref())})
                }
                other => serde_json::to_value(other).map_err(|error| error.to_string())?,
            };
            objects.push(value);
        }
        if assets.len() > crate::asset::MAX_ASSETS
            || assets.values().map(|a| a.bytes().len()).sum::<usize>() > MAX_BYTES / 2
        {
            return Err(
                "Copy exceeds system clipboard limit. Use the XiToc session clipboard.".into(),
            );
        }
        serde_json::to_string(&AssetBundle {
            version: 3,
            objects,
            assets: assets.into_values().collect(),
        })
        .map_err(|error| error.to_string())
    }

    fn decode_assets(value: serde_json::Value) -> Result<Self, String> {
        if value
            .get("assets")
            .and_then(|v| v.as_array())
            .is_none_or(|v| v.len() > crate::asset::MAX_ASSETS)
            || value
                .get("objects")
                .and_then(|v| v.as_array())
                .is_none_or(|v| v.len() > MAX_OBJECTS)
        {
            return Err("Asset clipboard limit exceeded.".into());
        }
        let wire: AssetBundle = serde_json::from_value(value).map_err(|error| error.to_string())?;
        let mut assets = std::collections::HashMap::new();
        for asset in wire.assets {
            if assets.insert(asset.id(), asset).is_some() {
                return Err("Duplicate clipboard asset.".into());
            }
        }
        let mut images = std::collections::HashMap::new();
        let mut objects = Vec::with_capacity(wire.objects.len());
        for value in wire.objects {
            if value.as_object().is_none_or(|fields| fields.len() != 2) {
                return Err("Invalid clipboard object fields.".into());
            }
            let object = match value["kind"].as_str() {
                Some("Attachment") => {
                    let a: AttachmentReference =
                        serde_json::from_value(value["data"].clone()).map_err(|e| e.to_string())?;
                    let asset = assets
                        .get(&a.asset_id)
                        .ok_or("Missing attachment asset.")?
                        .clone();
                    crate::attachment::Attachment {
                        id: a.id,
                        document_id: a.document_id,
                        surface: a.surface,
                        placement: a.placement,
                        dimensions: a.dimensions,
                        original_name: a.original_name,
                        listing: a.listing,
                        display_name: a.display_name,
                        locked: a.locked,
                        asset,
                    }
                    .into()
                }
                Some("Image") => {
                    let i: ImageReference =
                        serde_json::from_value(value["data"].clone()).map_err(|e| e.to_string())?;
                    let asset = match images.entry(i.asset_id) {
                        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            let bytes = assets
                                .get(&i.asset_id)
                                .ok_or("Missing image asset.")?
                                .clone();
                            entry.insert(crate::image_object::ImageAsset::from_binary(bytes)?.0)
                        }
                    };
                    i.resolve(asset.clone())
                }
                _ => serde_json::from_value(value).map_err(|e| e.to_string())?,
            };
            objects.push(object);
        }
        Ok(Self {
            version: 1,
            objects,
        })
    }

    pub fn decode(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BYTES {
            return Err("Object clipboard exceeds 64 MiB.".into());
        }
        let json = text
            .strip_prefix(PREFIX)
            .ok_or("Clipboard does not contain XiToc objects. Use Copy on a selection first.")?;
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("Cannot read object clipboard: {e}"))?;
        let version = value.get("version").and_then(|v| v.as_u64());
        let bundle: Self = if version == Some(3) {
            Self::decode_assets(value)?
        } else if version == Some(2) {
            if value
                .get("assets")
                .and_then(|v| v.as_array())
                .is_none_or(|v| v.len() > 16)
            {
                return Err("Image asset limit exceeded.".into());
            }
            let wire: ImageBundle = serde_json::from_value(value).map_err(|e| e.to_string())?;
            if wire.assets.len() > 16 || wire.objects.len() > MAX_OBJECTS {
                return Err("Image clipboard limit exceeded.".into());
            }
            let mut assets = std::collections::HashMap::new();
            for asset in wire.assets {
                if assets.insert(asset.id(), asset).is_some() {
                    return Err("Duplicate image asset.".into());
                }
            }
            let mut objects = Vec::new();
            for value in wire.objects {
                if value.get("kind").and_then(|v| v.as_str()) == Some("Image") {
                    if value.as_object().is_none_or(|v| v.len() != 2) {
                        return Err("Unknown image clipboard fields.".into());
                    }
                    let i: ImageReference =
                        serde_json::from_value(value["data"].clone()).map_err(|e| e.to_string())?;
                    let asset = assets
                        .get(&i.asset_id)
                        .ok_or("Missing clipboard image asset.")?
                        .clone();
                    objects.push(i.resolve(asset));
                } else if value["kind"] == "Attachment" {
                    return Err("Attachments require clipboard version 3.".into());
                } else {
                    objects.push(serde_json::from_value(value).map_err(|e| e.to_string())?);
                }
            }
            Self {
                version: 1,
                objects,
            }
        } else {
            if value
                .get("objects")
                .and_then(|v| v.as_array())
                .is_some_and(|objects| {
                    objects.iter().any(|o| {
                        matches!(
                            o.get("kind").and_then(|v| v.as_str()),
                            Some("Image" | "Attachment")
                        )
                    })
                })
            {
                return Err("Asset objects require a resource table.".into());
            }
            serde_json::from_value(value)
                .map_err(|e| format!("Cannot read object clipboard: {e}"))?
        };
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
        if let (Some(page), [ClipboardObject::Attachment(file)]) =
            (drawing.bounds(surface), self.objects.as_slice())
            && file.listing.is_some()
        {
            // A listing can overflow its paper. Keep the source font and frame
            // geometry, clamping its anchor when the usual centered fit fails.
            let overflows = (0..2).any(|i| bounds.max[i] - bounds.min[i] > page[i] - 2.);
            if overflows {
                let anchor = file.placement.apply([0.; 2]);
                let position = std::array::from_fn(|i| {
                    let offset = anchor[i] - center[i];
                    (target[i] + offset).clamp(0.5, page[i] - 0.5) - offset
                });
                return self.insert_placed(
                    drawing,
                    surface,
                    Placement {
                        origin: center,
                        position,
                        ..Default::default()
                    },
                );
            }
        }
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
        let remap: std::collections::HashMap<Uuid, Uuid> = self
            .objects
            .iter()
            .zip(&objects)
            .map(|(old, new)| (old.id(), new.id()))
            .collect();
        for object in &mut objects {
            if let ClipboardObject::Group(g) = object {
                std::sync::Arc::make_mut(g).children =
                    g.children.iter().map(|id| remap[id]).collect();
            }
            if let ClipboardObject::Connector(c) = object {
                let mut copy = (**c).clone();
                copy.remap(&remap);
                *object = copy.into();
            }
        }
        let children: std::collections::HashSet<_> = objects
            .iter()
            .filter_map(|o| match o {
                ClipboardObject::Group(g) => Some(&g.children),
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        let ids = objects
            .iter()
            .map(ClipboardObject::id)
            .filter(|id| !children.contains(id))
            .collect();
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
        std::sync::Arc::make_mut(&mut s.points)[0].pressure = Some(f32::NAN);
        assert!(ObjectBundle::capture(std::iter::once(s)).is_err());
    }
}
