//! Immutable original-byte assets and portable image objects. No UI or filesystem IO.
use crate::asset::{BinaryAsset, encoded_bytes};
use crate::{
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectId, ObjectType, Placement, Point},
};
use image::ImageDecoder;
use serde::{Deserialize, Serialize};
use std::{io::Cursor, sync::Arc};
use uuid::Uuid;

pub use crate::asset::AssetId;
pub const MAX_IMAGE_BYTES: usize = crate::asset::MAX_ASSET_BYTES;
pub const MAX_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;
pub const DISPLAY_SIDE: u32 = 2048;

/// Content identity covers original compressed bytes, not a filename or display texture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImageAsset {
    #[serde(flatten)]
    pub(crate) binary: Arc<BinaryAsset>,
    dimensions: [u32; 2],
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
        if asset.id() != wire.id || asset.dimensions != wire.dimensions {
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
        self.binary.id()
    }
    pub fn bytes(&self) -> &[u8] {
        self.binary.bytes()
    }
    pub fn dimensions(&self) -> [u32; 2] {
        self.dimensions
    }
    pub fn import(bytes: Arc<[u8]>) -> Result<(Arc<Self>, DecodedImage), String> {
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            return Err("Image must be 1 byte–16 MiB.".into());
        }
        Self::from_binary(BinaryAsset::new(bytes)?)
    }
    pub(crate) fn from_binary(
        binary: Arc<BinaryAsset>,
    ) -> Result<(Arc<Self>, DecodedImage), String> {
        let (dimensions, decoded) = decode(binary.bytes())?;
        Ok((Arc::new(Self { binary, dimensions }), decoded))
    }
    pub fn decode(&self) -> Result<DecodedImage, String> {
        decode(self.bytes()).map(|(_, d)| d)
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
        crate::selection::box_rectangle_matches(self.corners(), b, crossing)
    }
    pub fn lasso_matches(&self, r: &crate::selection::LassoRegion) -> bool {
        crate::selection::box_lasso_matches(self.corners(), r)
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
    /// Resize from the captured image, keeping the opposite grip fixed.
    pub fn resized(&self, index: usize, target: Point, proportional: bool) -> Self {
        if target == self.handles()[index] {
            return self.clone();
        }
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
        let minimum = self.dimensions.map(|size| size.min(1.0));
        if proportional {
            let factor = if x && y {
                // Project the target onto the original anchor-to-corner diagonal.
                (dims[0] * w + dims[1] * h) / (w * w + h * h)
            } else if x {
                dims[0] / w
            } else {
                dims[1] / h
            };
            let min_factor = (minimum[0] / w).max(minimum[1] / h);
            let max_factor = 1e6 / w.max(h);
            let factor = factor.clamp(min_factor, max_factor);
            dims = self.dimensions.map(|size| (size * factor).clamp(0.01, 1e6));
        } else {
            dims = std::array::from_fn(|axis| dims[axis].clamp(minimum[axis], 1e6));
        }
        if dims == self.dimensions {
            return self.clone();
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
        for proportional in [true, false] {
            for index in 0..8 {
                let source = &image;
                let h = source.handles()[index];
                let next = source.resized(index, [h[0] + 17., h[1] + 13.], proportional);
                assert!(next.valid());
                assert!(Arc::ptr_eq(&next.asset, &source.asset));
                let a = source.handles()[(index + 4) % 8];
                let b = next.handles()[(index + 4) % 8];
                assert!((a[0] - b[0]).hypot(a[1] - b[1]) < 1e-8);
                if proportional {
                    assert!((next.dimensions[0] / next.dimensions[1] - 2.).abs() < 1e-9);
                }
            }
        }
        assert!(
            image
                .resized(4, [0., 0.], false)
                .dimensions
                .iter()
                .all(|n| *n > 0.)
        );
    }
    #[test]
    fn free_edges_and_corners_and_shift_use_the_current_proportions() {
        let mut image = ImageObject::new(Uuid::new_v4(), Surface::Canvas, [100., 120.], asset());
        image.dimensions = [400., 200.];
        for (index, offset, dimensions) in [
            (0, [-200., -30.], [600., 230.]),
            (1, [200., -30.], [400., 230.]),
            (2, [200., -30.], [600., 230.]),
            (3, [200., -30.], [600., 200.]),
            (4, [200., 30.], [600., 230.]),
            (5, [200., 30.], [400., 230.]),
            (6, [-200., 30.], [600., 230.]),
            (7, [-200., 30.], [600., 200.]),
        ] {
            let start = image.handles()[index];
            let target = std::array::from_fn(|axis| start[axis] + offset[axis]);
            let next = image.resized(index, target, false);
            assert_eq!(next.dimensions, dimensions);
            let anchor = (index + 4) % 8;
            assert_eq!(next.handles()[anchor], image.handles()[anchor]);
        }
        let proportional = image.resized(3, [700., 350.], true);
        assert_eq!(proportional.dimensions, [600., 300.]);
        assert_eq!(proportional.handles()[7], image.handles()[7]);
        assert_eq!(proportional.placement.position, [100., 70.]);
        let stretched = image.resized(3, [700., 350.], false);
        let scaled = stretched.resized(3, [1000., 350.], true);
        assert_eq!(scaled.dimensions, [900., 300.]);
        let shrunk = stretched.resized(4, [-1e9, -1e9], true);
        let enlarged = stretched.resized(4, [1e9, 1e9], true);
        for candidate in [shrunk, enlarged] {
            assert!(candidate.valid());
            assert!((candidate.dimensions[0] / candidate.dimensions[1] - 3.).abs() < 1e-8);
        }
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
