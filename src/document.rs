//! Platform-neutral semantic data. Paths are locators, never document identity.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentKind {
    Page,
    Whiteboard,
}

impl DocumentKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Page => "Page Note",
            Self::Whiteboard => "Whiteboard",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageGeometry {
    pub width_mm: f64,
    pub height_mm: f64,
}

impl Default for PageGeometry {
    fn default() -> Self {
        Self {
            width_mm: 210.0,
            height_mm: 297.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Viewport {
    pub center: [f64; 2],
    pub zoom: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            center: [0.0; 2],
            zoom: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Layout {
    Page { pages: Vec<PageGeometry> },
    Whiteboard { viewport: Viewport, grid: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentData {
    schema_version: u32,
    document_id: Uuid,
    layout: Layout,
}

impl DocumentData {
    pub fn blank(kind: DocumentKind) -> Self {
        Self {
            schema_version: 1,
            document_id: Uuid::new_v4(),
            layout: match kind {
                DocumentKind::Page => Layout::Page {
                    pages: vec![PageGeometry::default()],
                },
                DocumentKind::Whiteboard => Layout::Whiteboard {
                    viewport: Viewport::default(),
                    grid: true,
                },
            },
        }
    }

    pub fn id(&self) -> Uuid {
        self.document_id
    }
    pub fn layout(&self) -> &Layout {
        &self.layout
    }
    pub fn kind(&self) -> DocumentKind {
        match self.layout {
            Layout::Page { .. } => DocumentKind::Page,
            Layout::Whiteboard { .. } => DocumentKind::Whiteboard,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.document_id.is_nil() {
            return Err("Unsupported XiToc schema or missing document identity".into());
        }
        match &self.layout {
            Layout::Page { pages } if pages.len() == 1 && pages[0] == PageGeometry::default() => {
                Ok(())
            }
            Layout::Whiteboard { viewport, .. }
                if viewport.center.iter().all(|v| v.is_finite())
                    && viewport.zoom.is_finite()
                    && (0.1..=8.0).contains(&viewport.zoom) =>
            {
                Ok(())
            }
            _ => Err("Unsupported or invalid blank-document geometry/settings".into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location(String);

impl Location {
    pub fn new(path: String) -> Self {
        Self(path)
    }
    pub fn name(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    pub data: DocumentData,
    pub location: Location,
    pub external: bool,
}

/// Single-component filename, shared by native and browser creation dialogs.
pub fn pdf_filename(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty()
        || name.len() > 180
        || name.starts_with('.')
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "/\\<>:\"|?*".contains(c))
    {
        return Err("Use a simple filename without path separators or reserved characters".into());
    }
    let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL"].contains(&base.as_str())
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
    {
        return Err("This filename is reserved on Windows".into());
    }
    if name.to_ascii_lowercase().ends_with(".pdf") {
        Ok(name.into())
    } else {
        Ok(format!("{name}.pdf"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kinds_have_stable_distinct_identities_and_matching_layouts() {
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let doc = DocumentData::blank(kind);
            assert_eq!(doc.kind(), kind);
            assert_ne!(doc.id(), DocumentData::blank(kind).id());
            doc.validate().unwrap();
        }
    }
    #[test]
    fn names_cannot_escape_a_folder_or_use_reserved_windows_names() {
        for bad in [
            "", "../note", "a/b", "a\\b", "CON.pdf", "LPT1", "note:", ".hidden",
        ] {
            assert!(pdf_filename(bad).is_err(), "{bad}");
        }
        assert_eq!(pdf_filename("Notes").unwrap(), "Notes.pdf");
        assert_eq!(pdf_filename("Notes.PDF").unwrap(), "Notes.PDF");
    }
}
