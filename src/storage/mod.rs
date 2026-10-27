//! Platform boundaries; the UI never reads or writes filesystem bytes itself.
use crate::document::{Document, DocumentKind};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: String,
    pub kind: Option<DocumentKind>,
    pub document_id: Option<uuid::Uuid>,
    pub problem: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Folder {
    pub path: String,
    pub name: String,
    pub folders: Vec<Folder>,
    pub files: Vec<Entry>,
}

#[derive(Clone)]
pub struct Loaded {
    pub document: Document,
    pub bytes: std::sync::Arc<[u8]>,
}

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Workspace;
#[cfg(target_arch = "wasm32")]
pub mod web;
