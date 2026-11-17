//! Immutable original-byte assets and portable image objects. No UI or filesystem IO.
use crate::{
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectId, ObjectType, Placement, Point},
};
use image::ImageDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Cursor, sync::Arc};
use uuid::Uuid;

pub type AssetId = [u8; 32];
pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;
pub const DISPLAY_SIDE: u32 = 2048;

/// Content identity covers original compressed bytes, not a filename or display texture.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ImageAsset {
    id: AssetId,
    #[serde(with = "encoded_bytes")]
    bytes: Arc<[u8]>,
    dimensions: [u32; 2],
}
mod encoded_bytes {
    use super::*;
    use base64::Engine;
    pub fn serialize<S: serde::Serializer>(
        bytes: &Arc<[u8]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<[u8]>, D::Error> {
        let s = String::deserialize(deserializer)?;
        if s.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
            return Err(serde::de::Error::custom("Image asset exceeds 16 MiB."));
        }
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map(Into::into)
            .map_err(serde::de::Error::custom)
    }
}
impl<'de> Deserialize<'de> for ImageAsset {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            id: AssetId,
            #[serde(with = "encoded_bytes")]
            bytes: Arc<[u8]>,
            dimensions: [u32; 2],
        }
        let wire = Wire::deserialize(d)?;
        let (asset, _) = Self::import(wire.bytes).map_err(serde::de::Error::custom)?;
        if asset.id != wire.id || asset.dimensions != wire.dimensions {
            return Err(serde::de::Error::custom(
                "Image asset identity/dimensions mismatch.",
            ));
        }
        Arc::try_unwrap(asset).map_err(|_| serde::de::Error::custom("Invalid image asset"))
    }
}
/// Derived, disposable RGBA preview. Original asset is retained independently.
pub struct DecodedImage {
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
}
impl ImageAsset {
    pub fn id(&self) -> AssetId {
        self.id
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn dimensions(&self) -> [u32; 2] {
        self.dimensions
    }
    pub fn import(bytes: Arc<[u8]>) -> Result<(Arc<Self>, DecodedImage), String> {
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            return Err("Image must be 1 byte–16 MiB.".into());
        }
        let (dimensions, decoded) = decode(&bytes)?;
        Ok((
            Arc::new(Self {
                id: Sha256::digest(&bytes).into(),
                bytes,
                dimensions,
            }),
            decoded,
        ))
    }
    pub fn decode(&self) -> Result<DecodedImage, String> {
        decode(&self.bytes).map(|(_, d)| d)
    }
}
fn decode(bytes: &[u8]) -> Result<([u32; 2], DecodedImage), String> {
    let format =
        image::guess_format(bytes).map_err(|_| "Unsupported image. Choose PNG, JPEG or WebP.")?;
    if !matches!(
        format,
        image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP
    ) {
        return Err("Unsupported image. Choose PNG, JPEG or WebP.".into());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("Cannot decode image: {e}"))?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_IMAGE_PIXELS {
        return Err("Image exceeds 32 megapixels or has invalid dimensions.".into());
    }
    let orientation = decoder
        .orientation()
        .map_err(|e| format!("Invalid image orientation: {e}"))?;
    let mut decoded =
        image::DynamicImage::from_decoder(decoder).map_err(|e| format!("Corrupted image: {e}"))?;
    decoded.apply_orientation(orientation);
    let dimensions = [decoded.width(), decoded.height()];
    if decoded.width() > DISPLAY_SIDE || decoded.height() > DISPLAY_SIDE {
        decoded = decoded.resize(
            DISPLAY_SIDE,
            DISPLAY_SIDE,
            image::imageops::FilterType::Triangle,
        );
    }
    let rgba = decoded.into_rgba8();
    Ok((
        dimensions,
        DecodedImage {
            size: [rgba.width() as usize, rgba.height() as usize],
            rgba: rgba.into_raw(),
        },
    ))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageObject {
    pub id: ObjectId,
    pub document_id: Uuid,
    pub surface: Surface,
    pub placement: Placement,
    pub dimensions: Point,
    pub opacity: u8,
    pub preserve_aspect: bool,
    pub locked: bool,
    /// Shared immutable asset handle. History/duplicates never clone original bytes.
    pub asset: Arc<ImageAsset>,
}
impl ImageObject {
    pub fn new(owner: Uuid, surface: Surface, position: Point, asset: Arc<ImageAsset>) -> Self {
        let dims = asset.dimensions();
        let factor = (320.0 / f64::from(dims[0].max(dims[1]))).min(1.0);
        Self {
            id: Uuid::new_v4(),
            document_id: owner,
            surface,
            placement: Placement {
                position,
                ..Default::default()
            },
            dimensions: dims.map(|n| f64::from(n) * factor),
            opacity: 255,
            preserve_aspect: true,
            locked: false,
            asset,
        }
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && !self.document_id.is_nil()
            && self.placement.valid()
            && self
                .dimensions
                .iter()
                .all(|n| n.is_finite() && (0.01..=1e6).contains(n))
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
            .map(|p| self.placement.apply(p))
    }
    pub fn transformed(&self, delta: Placement) -> Self {
        let mut n = self.clone();
        n.placement = self.placement.then(delta, [0.; 2]);
        n
    }
    pub fn rectangle_matches(&self, b: ObjectBounds, crossing: bool) -> bool {
        let corners = self.corners();
        let mut polygon = corners.to_vec();
        polygon.push(corners[0]);
        crate::selection::rectangle_matches(b, crossing, &polygon, &[], 0., Default::default())
            || (crossing
                && b.corners()
                    .iter()
                    .any(|p| crate::selection::inside(*p, &corners)))
    }
    pub fn lasso_matches(&self, r: &crate::selection::LassoRegion) -> bool {
        let corners = self.corners();
        let mut p = corners.to_vec();
        p.push(corners[0]);
        r.encloses(&p, &[], 0., Default::default())
    }
    pub fn handles(&self) -> [Point; 8] {
        let [w, h] = self.dimensions;
        [
            [0., 0.],
            [w * 0.5, 0.],
            [w, 0.],
            [w, h * 0.5],
            [w, h],
            [w * 0.5, h],
            [0., h],
            [0., h * 0.5],
        ]
        .map(|p| self.placement.apply(p))
    }
    /// Captured local resize; keeps the opposite handle fixed, including rotated/flipped objects.
    pub fn resized(&self, index: usize, target: Point) -> Self {
        let local = self.placement.inverse(target);
        let [w, h] = self.dimensions;
        let left = matches!(index, 0 | 6 | 7);
        let top = matches!(index, 0..=2);
        let x = !matches!(index, 1 | 5);
        let y = !matches!(index, 3 | 7);
        let mut dims = [
            if x {
                if left { w - local[0] } else { local[0] }
            } else {
                w
            },
            if y {
                if top { h - local[1] } else { local[1] }
            } else {
                h
            },
        ];
        for v in &mut dims {
            *v = v.clamp(1., 1e6);
        }
        if self.preserve_aspect {
            let factor = if x && y {
                ((dims[0] / w) + (dims[1] / h)) * 0.5
            } else if x {
                dims[0] / w
            } else {
                dims[1] / h
            };
            dims = [w * factor, h * factor];
        }
        let opposite = self.handles()[(index + 4) % 8];
        let normalized = [
            [0., 0.],
            [0.5, 0.],
            [1., 0.],
            [1., 0.5],
            [1., 1.],
            [0.5, 1.],
            [0., 1.],
            [0., 0.5],
        ][(index + 4) % 8];
        let mut next = self.clone();
        next.dimensions = dims;
        next.placement.origin = [0.; 2];
        next.placement.position = [0.; 2];
        let offset = next
            .placement
            .apply([dims[0] * normalized[0], dims[1] * normalized[1]]);
        next.placement.position = std::array::from_fn(|i| opposite[i] - offset[i]);
        next
    }
}
impl CanvasObject for ImageObject {
    fn object_id(&self) -> ObjectId {
        self.id
    }
    fn object_type(&self) -> ObjectType {
        ObjectType::Image
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
    fn hit_test(&self, p: Point, t: f64) -> bool {
        t.is_finite()
            && t >= 0.
            && self
                .local_bounds()
                .unwrap()
                .expand(t / self.placement.scale)
                .contains(self.placement.inverse(p))
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    pub fn bytes(format: image::ImageFormat) -> Arc<[u8]> {
        let rgba = image::RgbaImage::from_fn(40, 20, |x, y| {
            image::Rgba([
                x as u8 * 5,
                y as u8 * 10,
                100,
                if x < 10 { 100 } else { 255 },
            ])
        });
        let image = if format == image::ImageFormat::Jpeg {
            image::DynamicImage::ImageRgb8(image::DynamicImage::ImageRgba8(rgba).into_rgb8())
        } else {
            image::DynamicImage::ImageRgba8(rgba)
        };
        let mut data = Cursor::new(Vec::new());
        image.write_to(&mut data, format).unwrap();
        data.into_inner().into()
    }
    pub fn asset() -> Arc<ImageAsset> {
        ImageAsset::import(bytes(image::ImageFormat::Png))
            .unwrap()
            .0
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use test_support::*;
    #[test]
    fn image_formats_original_bytes_alpha_identity_and_errors() {
        for format in [
            image::ImageFormat::Png,
            image::ImageFormat::Jpeg,
            image::ImageFormat::WebP,
        ] {
            let bytes = bytes(format);
            let (asset, decoded) = ImageAsset::import(bytes.clone()).unwrap();
            assert_eq!(asset.bytes(), bytes.as_ref());
            assert_eq!(asset.dimensions(), [40, 20]);
            assert_eq!(decoded.size, [40, 20]);
            assert_eq!(ImageAsset::import(bytes).unwrap().0.id(), asset.id());
            if format != image::ImageFormat::Jpeg {
                assert_eq!(decoded.rgba[3], 100);
            }
        }
        for bytes in [
            b"broken image".as_slice(),
            b"<svg><script>alert(1)</script></svg>".as_slice(),
            b"\x89PNG\r\n\x1a\n".as_slice(),
        ] {
            assert!(ImageAsset::import(bytes.into()).is_err());
        }
        assert!(ImageAsset::import(vec![0; MAX_IMAGE_BYTES + 1].into()).is_err());
    }
    #[test]
    fn jpeg_exif_orientation_is_applied_to_geometry_and_display() {
        let jpeg = bytes(image::ImageFormat::Jpeg);
        // Little-endian TIFF IFD: Orientation tag 0x112, SHORT value 6 (90 degrees).
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut data = vec![0xff, 0xd8, 0xff, 0xe1];
        data.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        data.extend_from_slice(exif);
        data.extend_from_slice(&jpeg[2..]);
        let (asset, decoded) = ImageAsset::import(data.into()).unwrap();
        assert_eq!(asset.dimensions(), [20, 40]);
        assert_eq!(decoded.size, [20, 40]);
    }
    #[test]
    fn all_eight_resize_handles_keep_opposite_anchor_and_shared_asset() {
        let image = ImageObject::new(Uuid::new_v4(), Surface::Canvas, [100., 120.], asset())
            .transformed(Placement {
                origin: [0.; 2],
                position: [0.; 2],
                scale: 2.,
                rotation: 0.4,
                reflected: [true, false],
            });
        for locked in [true, false] {
            for index in 0..8 {
                let mut source = image.clone();
                source.preserve_aspect = locked;
                let h = source.handles()[index];
                let next = source.resized(index, [h[0] + 17., h[1] + 13.]);
                assert!(next.valid());
                assert!(Arc::ptr_eq(&next.asset, &source.asset));
                let a = source.handles()[(index + 4) % 8];
                let b = next.handles()[(index + 4) % 8];
                assert!((a[0] - b[0]).hypot(a[1] - b[1]) < 1e-8);
                if locked {
                    assert!((next.dimensions[0] / next.dimensions[1] - 2.).abs() < 1e-9);
                }
            }
        }
        let mut unlocked = image.clone();
        unlocked.preserve_aspect = false;
        assert!(
            unlocked
                .resized(4, [0., 0.])
                .dimensions
                .iter()
                .all(|n| *n > 0.)
        );
    }
    #[test]
    fn rotated_image_selection_uses_polygon_not_axis_aligned_bounds() {
        let source = ImageObject::new(Uuid::new_v4(), Surface::Canvas, [100., 120.], asset());
        let source = source.transformed(Placement {
            origin: [120., 130.],
            position: [120., 130.],
            rotation: 0.7,
            ..Default::default()
        });
        let b = source.world_bounds().unwrap();
        assert!(!source.hit_test(b.min, 0.));
        assert!(source.hit_test(b.center(), 0.));
        let empty = ObjectBounds {
            min: b.min,
            max: b.min.map(|v| v + 0.01),
        };
        assert!(!source.rectangle_matches(empty, true));
        assert!(source.rectangle_matches(b.expand(1.), false));
        let r = crate::selection::LassoRegion::new(b.expand(1.).corners().to_vec()).unwrap();
        assert!(source.lasso_matches(&r));
    }
}
