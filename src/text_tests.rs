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
                t.content_mut().style.family = family.clone();
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
                    runs: Vec::new(),
                    paragraphs: Vec::new(),
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

#[test]
fn rich_ranges_preserve_other_properties_and_outside_words() {
    let mut c = TextContent {
        text: "Hello beautiful world".into(),
        ..Default::default()
    };
    c.format(
        6..15,
        &TextStylePatch {
            family: Some(TextFamily::Monospace),
            size: Some(42.),
            rgb: Some([210, 30, 70]),
            ..Default::default()
        },
    );
    assert_eq!(c.style_at(0), &TextStyle::default());
    assert_eq!(c.style_at(16), &TextStyle::default());
    assert_eq!(c.style_at(8).family, TextFamily::Monospace);
    assert_eq!(c.style_at(8).size, 42.);
    c.format(
        6..10,
        &TextStylePatch {
            bold: Some(true),
            ..Default::default()
        },
    );
    c.format(
        0..21,
        &TextStylePatch {
            rgb: Some([0, 100, 0]),
            ..Default::default()
        },
    );
    assert!(c.style_at(8).bold);
    assert!(!c.style_at(12).bold);
    assert_eq!(c.style_at(8).size, 42.);
    assert_eq!(c.style_at(18).size, 18.);
    assert!(c.valid());
    let copy: TextContent = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(c, copy);
}
#[test]
fn rich_unicode_ranges_snap_to_whole_extended_graphemes() {
    let mut c = TextContent {
        text: "Ae\u{301}👩‍💻中Ω!".into(),
        ..Default::default()
    };
    c.format(
        2..3,
        &TextStylePatch {
            underline: Some(true),
            ..Default::default()
        },
    );
    assert!(c.style_at(1).underline && c.style_at(2).underline);
    assert!(!c.style_at(0).underline && !c.style_at(3).underline);
    c.format(
        4..5,
        &TextStylePatch {
            size: Some(50.),
            ..Default::default()
        },
    );
    for i in 3..6 {
        assert_eq!(c.style_at(i).size, 50.);
    }
    c.replace(4..5, &TextContent::default());
    assert_eq!(c.text, "Ae\u{301}中Ω!");
    assert!(c.valid());
}
#[test]
fn insert_delete_and_rich_clipboard_keep_runs_and_paragraphs() {
    let mut c = TextContent {
        text: "leftRIGHT\nlast".into(),
        ..Default::default()
    };
    c.format(
        4..9,
        &TextStylePatch {
            bold: Some(true),
            family: Some(TextFamily::Monospace),
            ..Default::default()
        },
    );
    c.format_paragraphs(10..14, |p| {
        p.alignment = Alignment::Right;
        p.spacing = 12.;
    });
    let pasted = c.slice(4..14);
    c.replace(
        4..4,
        &TextContent {
            text: "✨".into(),
            style: TextStyle {
                italic: true,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert_eq!(c.text, "left✨RIGHT\nlast");
    assert!(c.style_at(4).italic && c.style_at(5).bold && !c.style_at(3).bold);
    c.replace(2..7, &TextContent::default());
    assert_eq!(c.text, "leGHT\nlast");
    assert_eq!(c.paragraph(1).alignment, Alignment::Right);
    c.replace(c.text.chars().count()..c.text.chars().count(), &pasted);
    assert!(c.text.ends_with("RIGHT\nlast"));
    let n = c.text.chars().count();
    assert!(c.style_at(n - 6).bold);
    assert_eq!(c.paragraph(2).spacing, 12.);
    assert!(c.valid());
}
#[test]
fn eight_handles_anchor_rotated_boxes_and_scaling_preserves_mixed_sizes() {
    let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    for rotation in [0., 0.7, -1.8] {
        let mut t = text(&d, Surface::Canvas);
        t.height = 100.;
        t.placement.rotation = rotation;
        t.placement.scale = 2.;
        t.content_mut().format(
            0..5,
            &TextStylePatch {
                size: Some(36.),
                ..Default::default()
            },
        );
        for (i, h) in TEXT_HANDLES.iter().enumerate() {
            let start = t
                .placement
                .apply([t.width * h[0] + 1., t.height * h[1] + 2.]);
            let target = t
                .placement
                .apply([t.width * h[0] + 11., t.height * h[1] + 12.]);
            let resized = t.resized(i, start, target);
            assert_ne!((resized.width, resized.height), (t.width, t.height));
            let anchor = [
                if h[0] == 0. { 1. } else { 0. },
                if h[1] == 0. { 1. } else { 0. },
            ];
            let before = t
                .placement
                .apply([anchor[0] * t.width, anchor[1] * t.height]);
            let after = resized
                .placement
                .apply([anchor[0] * resized.width, anchor[1] * resized.height]);
            assert!((before[0] - after[0]).hypot(before[1] - after[1]) < 1e-8);
            assert_eq!(resized.content, t.content);
            let stationary = t.resized(i, start, start);
            assert_eq!(stationary, t);
        }
        t.box_style.resize = ResizeBehavior::Scale;
        let start = t.placement.apply([t.width, t.height]);
        let target = t.placement.apply([t.width * 1.5, t.height * 1.5]);
        let scaled = t.resized(3, start, target);
        assert!((scaled.width / scaled.height - t.width / t.height).abs() < 1e-8);
        assert_eq!(scaled.content.style_at(1).size, 54.);
        assert_eq!(scaled.content.style_at(7).size, 27.);
        assert_eq!(scaled.content.text, t.content.text);
        let edge = t.resized(5, start, target);
        assert_eq!(edge.content, t.content);
    }
}
#[test]
fn rich_layout_retains_fonts_sizes_paragraphs_and_caret_metrics_at_all_zooms() {
    with_fonts(|ctx| {
        let mut c = TextContent {
            text: "Hello beautiful world\nCentered second paragraph\nRight".into(),
            ..Default::default()
        };
        c.format(
            6..15,
            &TextStylePatch {
                family: Some(TextFamily::Monospace),
                size: Some(36.),
                bold: Some(true),
                ..Default::default()
            },
        );
        c.format_paragraphs(22..47, |p| {
            p.alignment = Alignment::Center;
            p.line_spacing = 1.6;
            p.spacing = 15.;
        });
        c.format_paragraphs(49..54, |p| p.alignment = Alignment::Right);
        let logical = text_layout::layout(ctx, &c, 220.);
        assert_eq!(logical.text(), c.text);
        let job = text_layout::job(&c, 220.);
        assert!(job.sections.iter().any(|s| s.format.font_id.size == 36.));
        assert!(job.sections.iter().any(|s| s.format.font_id.size == 18.));
        for zoom in [0.5, 1., 2., 4.] {
            let screen = text_layout::layout_scaled(ctx, &c, 220., zoom);
            assert_eq!(screen.rows.len(), logical.rows.len());
            for (a, b) in screen.rows.iter().zip(&logical.rows) {
                assert_eq!(a.text(), b.text());
                assert!((a.pos.y - b.pos.y * zoom as f32).abs() < 0.01);
                assert_eq!(a.glyphs.len(), b.glyphs.len());
            }
        }
    });
}
#[test]
fn installed_font_catalog_is_unique_and_missing_fonts_fall_back() {
    let families = crate::text_fonts::families();
    let mut names: Vec<_> = families.iter().map(|(n, _)| n.to_lowercase()).collect();
    let count = names.len();
    names.sort();
    names.dedup();
    assert_eq!(count, names.len());
    #[cfg(not(target_arch = "wasm32"))]
    assert!(
        families.len() > 2,
        "This native test requires at least one system font fixture"
    );
    with_fonts(|ctx| {
        let mut c = TextContent {
            text: "Fallback café 中文 Ω".into(),
            ..Default::default()
        };
        c.style.family = TextFamily::Installed("XiToc nonexistent font fixture".into());
        assert!(!text_layout::layout(ctx, &c, 240.).is_empty());
        for (_, family) in families.iter().take(5) {
            c.style.family = family.clone();
            assert!(!text_layout::layout(ctx, &c, 240.).is_empty());
        }
    });
}
#[test]
fn rich_journal_restores_full_content_and_box_styles() {
    let mut d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    let t = text(&d, Surface::Canvas);
    let id = t.id;
    assert!(d.replace_texts(vec![t.clone()], false));
    let mut changed = t.clone();
    changed.content_mut().format(
        0..5,
        &TextStylePatch {
            size: Some(40.),
            family: Some(TextFamily::Monospace),
            ..Default::default()
        },
    );
    changed
        .content_mut()
        .format_paragraphs(0..5, |p| p.alignment = Alignment::Center);
    changed.box_style.background = Some([200, 100, 0, 123]);
    changed.box_style.padding = 12.;
    changed.box_style.resize = ResizeBehavior::Scale;
    assert!(d.replace_texts(vec![changed.clone()], false));
    d.undo();
    assert_eq!(d.snapshot(id), Some(&t.clone().into()));
    d.redo();
    assert_eq!(d.snapshot(id), Some(&changed.into()));
}
