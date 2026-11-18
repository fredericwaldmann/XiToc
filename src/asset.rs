//! Content-addressed original bytes shared by image and attachment objects.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub type AssetId = [u8; 32];
pub const MAX_ASSET_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ASSETS: usize = 16;
pub const MAX_DOCUMENT_ASSET_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct BinaryAsset {
    id: AssetId,
    #[serde(with = "encoded_bytes")]
    bytes: Arc<[u8]>,
}

impl BinaryAsset {
    pub fn new(bytes: Arc<[u8]>) -> Result<Arc<Self>, String> {
        if bytes.len() > MAX_ASSET_BYTES {
            return Err("File exceeds 16 MiB.".into());
        }
        Ok(Arc::new(Self {
            id: Sha256::digest(&bytes).into(),
            bytes,
        }))
    }
    pub fn id(&self) -> AssetId {
        self.id
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

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
