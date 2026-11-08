use crate::{
    document::{DocumentData, DocumentKind},
    drawing::{Drawing, Surface},
    object::{CanvasObject, ObjectBounds, ObjectSnapshot, Placement},
    object_clipboard::ObjectBundle,
    text::*,
    text_layout,
};
use std::collections::HashSet;
fn with_fonts(test: impl FnOnce(&egui::Context)) {
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let mut test = Some(test);
    let mut output = ctx.run_ui(Default::default(), |_| test.take().unwrap()(&ctx));
    output.textures_delta.clear();
}
fn text(d: &Drawing, s: Surface) -> TextObject {
    let mut t = TextObject::new(d.owner(), s, [40., 60.], 160.);
    t.content_mut().text = "Hello café\nΕλληνικά e\u{301} 🌍".into();
    t
}
#[test]
fn semantic_unicode_identity_style_and_validation_roundtrip() {
    let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    let mut t = text(&d, Surface::Canvas);
    assert!(t.valid());
    let copy: TextObject = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
    assert_eq!(copy, t);
    t.content_mut().style.size = f64::NAN;
    assert!(!t.valid());
    t.content_mut().style.size = 18.;
    t.content_mut().text.push('\0');
    assert!(!t.valid());
}
#[test]
fn portable_faces_wrapping_explicit_breaks_and_reflow_preserve_source() {
    with_fonts(|ctx| {
        let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
        let mut t = text(&d, Surface::Canvas);
        t.content_mut().text ="A long paragraph that must wrap across many rows without changing the source.\nExplicit break\n".into();
        let source = t.content.text.clone();
        text_layout::measure(ctx, &mut t);
        let wide = t.height;
        t.width = 65.;
        text_layout::measure(ctx, &mut t);
        assert!(t.height > wide);
        assert_eq!(source, t.content.text);
        let rows = text_layout::layout(ctx, &t.content, t.width);
        assert!(rows.rows.len() > 4);
        for family in [TextFamily::Sans, TextFamily::Monospace] {
            for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
                t.content_mut().style.family = family;
                t.content_mut().style.bold = bold;
                t.content_mut().style.italic = italic;
                assert!(!text_layout::layout(ctx, &t.content, t.width).is_empty());
            }
        }
    });
}
#[test]
fn formatting_keeps_independent_color_alpha_and_layout_metrics() {
    with_fonts(|ctx| {
        let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
        let mut t = text(&d, Surface::Canvas);
        let original = t.content.style.clone();
        t.content_mut().style.color[3] = 91;
        TextStylePatch {
            rgb: Some([200, 40, 50]),
            bold: Some(true),
            underline: Some(true),
            alignment: Some(Alignment::Right),
            ..Default::default()
        }
        .apply(&mut t.content_mut().style);
        assert_eq!(t.content.style.color, [200, 40, 50, 91]);
        assert_eq!(t.content.style.family, original.family);
        text_layout::measure(ctx, &mut t);
        let small = t.height;
        t.content_mut().style.size = 36.;
        text_layout::measure(ctx, &mut t);
        assert!(t.height > small);
    });
}
#[test]
fn shared_journal_typing_coalescing_geometry_clipboard_and_exact_history() {
    with_fonts(|ctx| {
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let mut d = Drawing::new(&DocumentData::blank(kind));
            let s = if kind == DocumentKind::Page {
                Surface::Page(0)
            } else {
                Surface::Canvas
            };
            let mut t = text(&d, s);
            text_layout::measure(ctx, &mut t);
            let id = t.id;
            assert!(d.replace_texts(vec![t.clone()], false));
            for suffix in ["a", "b", "c"] {
                t.content_mut().text.push_str(suffix);
                text_layout::measure(ctx, &mut t);
                assert!(d.replace_texts(vec![t.clone()], true));
            }
            let typed = d.items().to_vec();
            d.undo();
            assert!(d.items().is_empty());
            d.redo();
            assert_eq!(typed, d.items());
            let delta = Placement {
                position: [20., 10.],
                rotation: 0.1,
                ..Default::default()
            };
            assert!(d.transform_objects(&HashSet::from([id]), delta));
            let moved = d.items().to_vec();
            d.undo();
            assert_eq!(d.items(), typed);
            d.redo();
            assert_eq!(d.items(), moved);
            let bundle = ObjectBundle::capture_objects(d.items().iter().cloned()).unwrap();
            let encoded = bundle.encode().unwrap();
            let bundle = ObjectBundle::decode(&encoded).unwrap();
            let ids = bundle.duplicate(&mut d, s);
            assert_eq!(ids.len(), 1);
            let Some(ObjectSnapshot::Text(copy)) = d.snapshot(ids[0]) else {
                panic!()
            };
            assert_ne!(copy.id, id);
            assert_eq!(copy.content, t.content);
            let before = d.items().to_vec();
            d.erase(&HashSet::from([id]));
            d.undo();
            assert_eq!(before, d.items());
        }
    });
}
#[test]
fn flips_relocate_boxes_without_mirroring_unicode_and_rotation_is_reversible() {
    let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    let t = text(&d, Surface::Canvas);
    for reflected in [[true, false], [false, true]] {
        let delta = Placement {
            origin: [200., 200.],
            position: [200., 200.],
            reflected,
            ..Default::default()
        };
        let flipped = t.transformed(delta);
        assert_eq!(flipped.content, t.content);
        assert_eq!(flipped.placement.reflected, [false; 2]);
        let restored = flipped.transformed(delta);
        for (a, b) in restored
            .corners()
            .iter()
            .flatten()
            .zip(t.corners().iter().flatten())
        {
            assert!((a - b).abs() < 1e-8);
        }
    }
    let r = t.transformed(Placement {
        rotation: 0.7,
        ..Default::default()
    });
    assert!((r.placement.rotation - 0.7).abs() < 1e-8);
}
#[test]
fn text_box_geometry_handles_rotated_hits_directional_and_lasso_selection() {
    let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    let mut t = text(&d, Surface::Canvas);
    t.placement.rotation = 0.4;
    assert!(t.hit_test(t.placement.apply([80., 10.]), 0.));
    assert!(!t.hit_test(t.placement.apply([180., 50.]), 0.));
    let b = t.world_bounds().unwrap().expand(5.);
    assert!(t.rectangle_matches(b, false));
    assert!(!t.rectangle_matches(
        ObjectBounds {
            min: b.min,
            max: b.center()
        },
        false
    ));
    assert!(t.rectangle_matches(
        ObjectBounds {
            min: t.placement.apply([79., 9.]),
            max: t.placement.apply([81., 11.])
        },
        true
    ));
}
#[test]
fn page_height_overflow_preserves_editable_source_and_fits_model_bounds() {
    with_fonts(|ctx| {
        let mut d = Drawing::new(&DocumentData::blank(DocumentKind::Page));
        let mut t = text(&d, Surface::Page(0));
        t.placement.position = [60., 760.];
        t.content_mut().text = "Long line\n".repeat(100);
        text_layout::measure(ctx, &mut t);
        assert!(t.height > 1000.);
        d.fit_text(&mut t);
        assert!(t.height < 100.);
        assert!(d.replace_texts(vec![t.clone()], false));
        assert_eq!(t.content.text.lines().count(), 100);
    });
}
#[test]
fn malformed_clipboard_text_and_ownership_fail_atomically() {
    let mut d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    let mut t = text(&d, Surface::Canvas);
    assert!(d.replace_texts(vec![t.clone()], false));
    let before = d.items().to_vec();
    t.document_id = uuid::Uuid::new_v4();
    assert!(!d.replace_texts(vec![t], false));
    assert_eq!(d.items(), before);
}
#[test]
fn bundled_fallback_has_latin_greek_cjk_and_combining_glyphs() {
    with_fonts(|ctx| {
        let content = TextContent::default();
        let font = text_layout::font(&content);
        ctx.fonts_mut(|f| {
            for glyph in ['é', 'Ω', '中', '日', '\u{301}'] {
                assert!(f.has_glyph(&font, glyph), "missing {glyph}");
            }
        });
    });
}
#[test]
fn zoomed_layout_keeps_rows_alignment_and_explicit_breaks() {
    with_fonts(|ctx| {
        for width in [80., 120., 240.] {
            for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
                let content = TextContent {
                    text: "Words wrap at box width.\nAn explicit line break.\n中文 café".into(),
                    style: TextStyle {
                        alignment,
                        ..Default::default()
                    },
                };
                let rows = text_layout::layout(ctx, &content, width).rows.len();
                for zoom in [0.5, 1., 2., 4.] {
                    let layout = text_layout::layout_scaled(ctx, &content, width, zoom);
                    assert_eq!(layout.rows.len(), rows, "zoom {zoom}, width {width}");
                }
            }
        }
    });
}
#[test]
fn transforms_share_immutable_content_and_dpi_never_changes_document_metrics() {
    let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    let original = text(&d, Surface::Canvas);
    let moved = original.transformed(Placement {
        position: [400., -900.],
        ..Default::default()
    });
    assert!(std::sync::Arc::ptr_eq(&original.content, &moved.content));
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let mut results = Vec::new();
    for zoom in [1., 2., 1.5, 1.] {
        ctx.set_zoom_factor(zoom);
        let mut output = ctx.run_ui(Default::default(), |_| {
            let mut t = original.clone();
            text_layout::measure(&ctx, &mut t);
            results.push((
                t.width,
                t.height,
                text_layout::layout(&ctx, &t.content, t.width)
                    .rows
                    .iter()
                    .map(|r| r.text())
                    .collect::<Vec<_>>(),
            ));
        });
        output.textures_delta.clear();
    }
    for result in &results[1..] {
        assert_eq!(result, &results[0]);
    }
}
#[test]
fn a_box_never_clips_a_single_wide_glyph_when_resized() {
    with_fonts(|ctx| {
        let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
        let mut t = text(&d, Surface::Canvas);
        t.width = 24.;
        t.content_mut().text = "WWW".into();
        t.content_mut().style.size = 80.;
        text_layout::measure(ctx, &mut t);
        let layout = text_layout::layout(ctx, &t.content, t.width);
        assert!(
            layout
                .rows
                .iter()
                .all(|r| f64::from(r.size.x) <= t.width + 0.001)
        );
    });
}
