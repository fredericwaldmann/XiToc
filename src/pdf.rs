//! Blank PDF 1.7 only: standard A4 page plus a named, embedded edit payload.
use crate::document::{Document, DocumentData, DocumentKind, Location};
use lopdf::{Document as PdfDocument, Object};
use pdf_writer::{Finish, Name, Pdf, Rect, Ref, Str, TextStr};

pub const MAX_PDF_BYTES: usize = 16 * 1024 * 1024;
const MAX_PAYLOAD: usize = 64 * 1024;
const PAYLOAD_NAME: &[u8] = b"XiToc/document.json";

pub fn encode(data: &DocumentData) -> Result<Vec<u8>, String> {
    data.validate()?;
    let payload = serde_json::to_vec(data).map_err(|e| e.to_string())?;
    let mut pdf = Pdf::new();
    pdf.set_version(1, 7);
    pdf.catalog(Ref::new(1))
        .pages(Ref::new(2))
        .names()
        .embedded_files()
        .names()
        .insert(Str(PAYLOAD_NAME), Ref::new(5));
    pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
    pdf.page(Ref::new(3))
        .parent(Ref::new(2))
        .media_box(Rect::new(0.0, 0.0, 595.276, 841.89))
        .contents(Ref::new(4))
        .resources()
        .finish();
    pdf.stream(Ref::new(4), b"").finish();
    pdf.file_spec(Ref::new(5))
        .path(Str(PAYLOAD_NAME))
        .unic_file(TextStr("XiToc/document.json"))
        .embedded_file_with_unicode(Ref::new(6))
        .finish();
    pdf.embedded_file(Ref::new(6), &payload)
        .subtype(Name(b"application/vnd.XiToc.document+json"))
        .finish();
    Ok(pdf.finish())
}

fn resolve<'a>(pdf: &'a PdfDocument, object: &'a Object) -> Result<&'a Object, String> {
    pdf.dereference(object)
        .map(|(_, value)| value)
        .map_err(|e| e.to_string())
}

fn find_payload<'a>(
    pdf: &'a PdfDocument,
    node: &'a Object,
    budget: &mut usize,
    depth: usize,
    found: &mut Option<&'a Object>,
) -> Result<(), String> {
    if depth > 16 || *budget == 0 {
        return Err("PDF attachment tree exceeds limits".into());
    }
    *budget -= 1;
    let dict = resolve(pdf, node)?.as_dict().map_err(|e| e.to_string())?;
    if let Ok(names) = dict.get(b"Names") {
        let names = resolve(pdf, names)?.as_array().map_err(|e| e.to_string())?;
        if names.len() % 2 != 0 {
            return Err("Invalid attachment name tree".into());
        }
        for pair in names.as_chunks::<2>().0 {
            if pair[0].as_str().map_err(|e| e.to_string())? == PAYLOAD_NAME {
                if found.is_some() {
                    return Err("Duplicate XiToc edit payloads".into());
                }
                *found = Some(&pair[1]);
            }
        }
    }
    if let Ok(kids) = dict.get(b"Kids") {
        for kid in resolve(pdf, kids)?.as_array().map_err(|e| e.to_string())? {
            find_payload(pdf, kid, budget, depth + 1, found)?;
        }
    }
    Ok(())
}

pub fn decode(bytes: &[u8], location: Location) -> Result<Document, String> {
    if bytes.len() > MAX_PDF_BYTES || !bytes.starts_with(b"%PDF-") {
        return Err("Not a supported PDF (maximum size 16 MiB)".into());
    }
    let pdf = PdfDocument::load_mem_with_options(
        bytes,
        lopdf::LoadOptions {
            strict: true,
            max_decompressed_size: Some(MAX_PDF_BYTES),
            ..Default::default()
        },
    )
    .map_err(|e| format!("Cannot parse PDF: {e}"))?;
    if pdf.is_encrypted() {
        return Err("Encrypted PDFs are not supported".into());
    }
    let catalog = pdf.catalog().map_err(|e| e.to_string())?;
    // Validate that a page tree exists before classifying an external document.
    if pdf.get_pages().is_empty() {
        return Err("PDF has no readable pages".into());
    }
    let mut payload = None;
    if let Ok(names) = catalog.get(b"Names") {
        let names = resolve(&pdf, names)?.as_dict().map_err(|e| e.to_string())?;
        if let Ok(files) = names.get(b"EmbeddedFiles") {
            find_payload(&pdf, files, &mut 256, 0, &mut payload)?;
        }
    }
    let Some(spec) = payload else {
        return Ok(Document {
            data: DocumentData::blank(DocumentKind::Page),
            location,
            external: true,
        });
    };
    let spec = resolve(&pdf, spec)?.as_dict().map_err(|e| e.to_string())?;
    let ef = resolve(&pdf, spec.get(b"EF").map_err(|e| e.to_string())?)?
        .as_dict()
        .map_err(|e| e.to_string())?;
    let stream = resolve(&pdf, ef.get(b"F").map_err(|e| e.to_string())?)?
        .as_stream()
        .map_err(|e| e.to_string())?;
    let payload = stream
        .decompressed_content_with_limit(MAX_PAYLOAD)
        .map_err(|e| e.to_string())?;
    let data: DocumentData = serde_json::from_slice(&payload).map_err(|e| {
        format!("Invalid/unsupported XiToc edit data; original PDF is unchanged: {e}")
    })?;
    data.validate()?;
    // Recognized metadata does not imply externally edited PDF graphics are
    // represented by this blank-only codec. Never display such content as blank.
    let pages = pdf.get_pages();
    let blank = if pages.len() == 1 {
        let page = pdf
            .get_dictionary(*pages.values().next().unwrap())
            .map_err(|e| e.to_string())?;
        let empty_contents = page
            .get(b"Contents")
            .ok()
            .and_then(|value| resolve(&pdf, value).ok())
            .and_then(|value| value.as_stream().ok())
            .is_some_and(|stream| stream.content.is_empty());
        empty_contents && !page.has(b"Annots")
    } else {
        false
    };
    Ok(Document {
        data,
        location,
        external: !blank,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn location() -> Location {
        Location::new("note.pdf".into())
    }
    #[test]
    fn blank_types_round_trip_with_identity_settings_and_standard_page() {
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let data = DocumentData::blank(kind);
            let bytes = encode(&data).unwrap();
            // Optional fixtures for manual independent-viewer validation.
            if let Some(directory) = std::env::var_os("XITOC_PDF_REVIEW_DIR") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join(format!("{}.pdf", kind.label())), &bytes).unwrap();
            }
            assert!(bytes.starts_with(b"%PDF-1.7"));
            assert_eq!(decode(&bytes, location()).unwrap().data, data);
            let parsed = PdfDocument::load_mem(&bytes).unwrap();
            assert_eq!(parsed.get_pages().len(), 1);
            let page = parsed
                .get_dictionary(*parsed.get_pages().values().next().unwrap())
                .unwrap();
            let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
            assert!((media[2].as_float().unwrap() - 595.276).abs() < 0.01);
        }
    }
    #[test]
    fn nondefault_viewport_settings_round_trip() {
        let mut value =
            serde_json::to_value(DocumentData::blank(DocumentKind::Whiteboard)).unwrap();
        value["layout"]["viewport"]["center"] = serde_json::json!([-1e10, 2e10]);
        value["layout"]["viewport"]["zoom"] = serde_json::json!(2.5);
        value["layout"]["grid"] = serde_json::json!(false);
        let data: DocumentData = serde_json::from_value(value).unwrap();
        assert_eq!(
            decode(&encode(&data).unwrap(), location()).unwrap().data,
            data
        );
    }

    #[test]
    fn ordinary_pdf_is_page_and_read_only() {
        let bytes = encode(&DocumentData::blank(DocumentKind::Whiteboard)).unwrap();
        let mut parsed = PdfDocument::load_mem(&bytes).unwrap();
        let root = parsed.trailer.get(b"Root").unwrap().as_reference().unwrap();
        parsed
            .get_object_mut(root)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .remove(b"Names");
        let mut ordinary = Vec::new();
        parsed.save_to(&mut ordinary).unwrap();
        let doc = decode(&ordinary, location()).unwrap();
        assert_eq!(doc.data.kind(), DocumentKind::Page);
        assert!(doc.external);
    }
    #[test]
    fn corrupted_or_future_payload_fails_instead_of_becoming_a_page() {
        let bytes = encode(&DocumentData::blank(DocumentKind::Whiteboard)).unwrap();
        let mut parsed = PdfDocument::load_mem(&bytes).unwrap();
        let stream = parsed
            .get_object_mut((6, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap();
        stream.set_content(b"{\"schema_version\":999}".to_vec());
        let mut bad = Vec::new();
        parsed.save_to(&mut bad).unwrap();
        assert!(decode(&bad, location()).is_err());
        assert!(decode(b"%PDF-broken", location()).is_err());
    }
    #[test]
    fn external_graphics_with_recognized_metadata_remain_read_only_and_keep_kind() {
        let bytes = encode(&DocumentData::blank(DocumentKind::Whiteboard)).unwrap();
        let mut parsed = PdfDocument::load_mem(&bytes).unwrap();
        parsed
            .get_object_mut((4, 0))
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_content(b"0 0 100 100 re f".to_vec());
        let mut edited = Vec::new();
        parsed.save_to(&mut edited).unwrap();
        let reopened = decode(&edited, location()).unwrap();
        assert!(reopened.external);
        assert_eq!(reopened.data.kind(), DocumentKind::Whiteboard);
    }

    #[test]
    fn duplicate_payloads_are_rejected() {
        let bytes = encode(&DocumentData::blank(DocumentKind::Page)).unwrap();
        let mut parsed = PdfDocument::load_mem(&bytes).unwrap();
        let root = parsed.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = parsed.get_object_mut(root).unwrap().as_dict_mut().unwrap();
        let pairs = catalog
            .get_mut(b"Names")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"EmbeddedFiles")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"Names")
            .unwrap()
            .as_array_mut()
            .unwrap();
        pairs.extend(pairs.clone());
        let mut duplicate = Vec::new();
        parsed.save_to(&mut duplicate).unwrap();
        assert!(
            decode(&duplicate, location())
                .unwrap_err()
                .contains("Duplicate")
        );
    }

    #[test]
    fn moving_locator_cannot_change_identity_or_kind() {
        let bytes = encode(&DocumentData::blank(DocumentKind::Whiteboard)).unwrap();
        let a = decode(&bytes, Location::new("one/note.pdf".into())).unwrap();
        let b = decode(&bytes, Location::new("two/renamed.pdf".into())).unwrap();
        assert_eq!(a.data, b.data);
        assert_ne!(a.location, b.location);
    }
}
