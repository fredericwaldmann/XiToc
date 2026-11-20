//! Content-addressed original bytes shared by image and attachment objects.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub type AssetId = [u8; 32];
pub const MAX_ASSET_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ASSETS: usize = 16;
pub const MAX_DOCUMENT_ASSET_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug)]
struct ListingSource {
    text: Arc<str>,
    lines: usize,
}

#[derive(Debug, Serialize)]
pub struct BinaryAsset {
    id: AssetId,
    #[serde(with = "encoded_bytes")]
    bytes: Arc<[u8]>,
    #[serde(skip)]
    listing_text: std::sync::OnceLock<Option<ListingSource>>,
}

impl BinaryAsset {
    pub fn new(bytes: Arc<[u8]>) -> Result<Arc<Self>, String> {
        if bytes.len() > MAX_ASSET_BYTES {
            return Err("File exceeds 16 MiB.".into());
        }
        Ok(Arc::new(Self {
            id: Sha256::digest(&bytes).into(),
            bytes,
            listing_text: Default::default(),
        }))
    }
    pub fn listing_text(&self) -> bool {
        self.listing_source().is_some()
    }
    pub fn listing_source(&self) -> Option<&str> {
        self.decoded_listing().map(|source| source.text.as_ref())
    }
    pub fn listing_line_count(&self) -> usize {
        self.decoded_listing().map_or(0, |source| source.lines)
    }
    fn decoded_listing(&self) -> Option<&ListingSource> {
        self.listing_text
            .get_or_init(|| {
                let original = crate::code_listing::source(&self.bytes).ok()?;
                let text: Arc<str> = if original.contains('\r') {
                    original.replace("\r\n", "\n").replace('\r', "\n").into()
                } else {
                    original.into()
                };
                let lines = crate::code_listing::line_count(&text);
                Some(ListingSource { text, lines })
            })
            .as_ref()
    }
    pub fn id(&self) -> AssetId {
        self.id
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl PartialEq for BinaryAsset {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.bytes == other.bytes
    }
}
impl Eq for BinaryAsset {}

impl<'de> Deserialize<'de> for BinaryAsset {
    fn deserialize<D: serde::Deserializer<'de>>(input: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            id: AssetId,
            #[serde(with = "encoded_bytes")]
            bytes: Arc<[u8]>,
        }
        let wire = Wire::deserialize(input)?;
        let asset = Self::new(wire.bytes).map_err(serde::de::Error::custom)?;
        if asset.id != wire.id {
            return Err(serde::de::Error::custom("Asset hash mismatch."));
        }
        Arc::try_unwrap(asset).map_err(|_| serde::de::Error::custom("Invalid asset."))
    }
}

pub(crate) mod encoded_bytes {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    pub fn serialize<S: serde::Serializer>(
        bytes: &Arc<[u8]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(input: D) -> Result<Arc<[u8]>, D::Error> {
        let encoded = String::deserialize(input)?;
        if encoded.len() > MAX_ASSET_BYTES.div_ceil(3) * 4 {
            return Err(serde::de::Error::custom("Asset exceeds 16 MiB."));
        }
        STANDARD
            .decode(encoded)
            .map(Into::into)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_asset_id_and_wire_encoding_match_known_vector() {
        let asset = BinaryAsset::new(Arc::from(&b"abc"[..])).unwrap();
        let expected = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(asset.id(), expected);
        let wire = serde_json::json!({ "id": expected, "bytes": "YWJj" });
        assert_eq!(serde_json::to_value(&asset).unwrap(), wire);
        assert_eq!(serde_json::from_value::<BinaryAsset>(wire).unwrap(), *asset);
    }
}
