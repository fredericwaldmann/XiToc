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
fn proportional_font_scaling_preserves_runs_and_reciprocal_sizes() {
    let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    let mut t = text(&d, Surface::Canvas);
    t.content_mut().text = "Small\nMedium\nLARGE".into();
    for (range, size, family, bold, color) in [
        (0..5, 12.0, TextFamily::Sans, false, [10, 20, 30, 255]),
        (6..12, 18.0, TextFamily::Monospace, true, [40, 50, 60, 220]),
        (13..18, 30.0, TextFamily::Sans, false, [70, 80, 90, 180]),
    ] {
        t.content_mut().format(
            range,
            &TextStylePatch {
                size: Some(size),
                family: Some(family),
                bold: Some(bold),
                italic: Some(!bold),
                underline: Some(bold),
                rgb: Some([color[0], color[1], color[2]]),
                alpha: Some(color[3]),
                ..Default::default()
            },
        );
    }
    let text_len = t.content.text.chars().count();
    t.content_mut().format_paragraphs(0..text_len, |p| {
        p.alignment = Alignment::Justify;
        p.line_spacing = 1.5;
    });
    let before = t.content.clone();
    assert!(t.content_mut().scale_font_sizes(1.1));
    assert!((t.content.style_at(0).size - 13.2).abs() < 1e-12);
    assert!((t.content.style_at(6).size - 19.8).abs() < 1e-12);
    assert!((t.content.style_at(13).size - 33.0).abs() < 1e-12);
    assert_eq!(t.content.style_at(6).family, TextFamily::Monospace);
    assert!(t.content.style_at(6).bold);
    assert!(!t.content.style_at(6).italic);
    assert!(t.content.style_at(6).underline);
    assert_eq!(t.content.style_at(6).color, [40, 50, 60, 220]);
    assert!(t.content_mut().scale_font_sizes(1.0 / 1.1));
    for (index, size) in [(0, 12.0), (6, 18.0), (13, 30.0)] {
        assert!((t.content.style_at(index).size - size).abs() < 1e-12);
    }
    assert_eq!(t.content.text, before.text);
    assert_eq!(t.content.paragraphs, before.paragraphs);
    assert_eq!(t.content.style_at(6).italic, before.style_at(6).italic);
    assert_eq!(
        t.content.style_at(6).underline,
        before.style_at(6).underline
    );
    assert_eq!(t.content.style_at(6).family, before.style_at(6).family);
    assert_eq!(t.content.style_at(6).color, before.style_at(6).color);
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
    c.format_paragraphs(0..4, |p| p.alignment = Alignment::Center);
    let pasted = c.slice(4..14);
    let mut empty = TextContent::default();
    empty.replace(0..0, &pasted);
    assert_eq!(empty.paragraph(0).alignment, Alignment::Center);
    assert_eq!(empty.paragraph(1).alignment, Alignment::Right);
    assert_eq!(empty.style_at(0).family, TextFamily::Monospace);
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
        t.layout.resize = ResizeBehavior::Scale;
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
fn rich_journal_restores_full_content_paragraphs_and_resize_mode() {
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
    changed.layout.resize = ResizeBehavior::Scale;
    assert!(d.replace_texts(vec![changed.clone()], false));
    d.undo();
    assert_eq!(d.snapshot(id), Some(&t.clone().into()));
    d.redo();
    assert_eq!(d.snapshot(id), Some(&changed.into()));
}

#[test]
fn paragraph_alignment_moves_real_glyphs_and_justifies_only_word_gaps() {
    with_fonts(|ctx| {
        let source = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
        let mut c = TextContent {
            text: source.into(),
            ..Default::default()
        };
        let natural = text_layout::layout(ctx, &c, 170.);
        for alignment in [
            Alignment::Left,
            Alignment::Center,
            Alignment::Right,
            Alignment::Justify,
        ] {
            c.format_paragraphs(0..source.len(), |p| p.alignment = alignment);
            let galley = text_layout::layout(ctx, &c, 170.);
            assert_eq!(galley.rows.len(), natural.rows.len());
            for (i, (row, original)) in galley.rows.iter().zip(&natural.rows).enumerate() {
                let last = row
                    .glyphs
                    .iter()
                    .rposition(|g| !g.chr.is_whitespace())
                    .unwrap();
                let right = row.pos.x + row.glyphs[last].max_x();
                match alignment {
                    Alignment::Left => assert!(row.pos.x.abs() < 0.01),
                    Alignment::Center => {
                        assert!(
                            (row.pos.x - (170. - row.size.x) * 0.5).abs() < 0.01,
                            "row {i}: pos {} width {}",
                            row.pos.x,
                            row.size.x
                        )
                    }
                    Alignment::Right => assert!((row.pos.x + row.size.x - 170.).abs() < 0.01),
                    Alignment::Justify => {
                        assert!(row.pos.x.abs() < 0.01);
                        if i + 1 < galley.rows.len() && row.text().trim().contains(' ') {
                            assert!((right - 170.).abs() < 0.01);
                        } else {
                            assert_eq!(row.glyphs, original.glyphs);
                        }
                        for (g, before) in row.glyphs.iter().zip(&original.glyphs) {
                            if !g.chr.is_whitespace() {
                                assert_eq!(g.advance_width, before.advance_width);
                            }
                        }
                        for pair in row.glyphs.windows(2).zip(original.glyphs.windows(2)) {
                            if !pair.0[0].chr.is_whitespace() && !pair.0[1].chr.is_whitespace() {
                                assert!(
                                    ((pair.0[1].pos.x - pair.0[0].pos.x)
                                        - (pair.1[1].pos.x - pair.1[0].pos.x))
                                        .abs()
                                        < 0.001
                                );
                            }
                        }
                    }
                }
            }
        }
    });
}
#[test]
fn justified_last_lines_explicit_breaks_single_words_and_unicode_keep_natural_spacing() {
    with_fonts(|ctx| {
        for text in [
            "one\ntwo\n",
            "supercalifragilisticexpialidocious",
            "中文漢字日本語",
            "café Ελληνικά Ωmega more words for wrapping\nexplicit final line",
        ] {
            let mut c = TextContent {
                text: text.into(),
                ..Default::default()
            };
            let natural = text_layout::layout(ctx, &c, 140.);
            c.format_paragraphs(0..text.chars().count(), |p| {
                p.alignment = Alignment::Justify
            });
            let justified = text_layout::layout(ctx, &c, 140.);
            assert_eq!(natural.rows.len(), justified.rows.len());
            for (i, row) in justified.rows.iter().enumerate() {
                assert_eq!(row.text(), natural.rows[i].text());
                if row.ends_with_newline
                    || i + 1 == justified.rows.len()
                    || !row.text().trim().contains(' ')
                {
                    assert_eq!(row.glyphs, natural.rows[i].glyphs);
                }
            }
            assert_eq!(c.text, text);
        }
    });
}
#[test]
fn justified_mixed_fonts_reflow_and_scaled_caret_geometry_share_the_same_layout() {
    with_fonts(|ctx| {
        let mut c=TextContent {text:"Hello beautiful world with café Ελληνικά and more words on several lines for justified layout".into(),..Default::default()};
        c.format(
            6..15,
            &TextStylePatch {
                family: Some(TextFamily::Monospace),
                size: Some(30.),
                underline: Some(true),
                ..Default::default()
            },
        );
        let runs = c.runs.clone();
        c.format_paragraphs(0..c.text.chars().count(), |p| {
            p.alignment = Alignment::Justify
        });
        for width in [140., 220., 330.] {
            let logical = text_layout::layout(ctx, &c, width);
            for scale in [0.5, 1., 2., 4.] {
                let screen = text_layout::layout_scaled(ctx, &c, width, scale);
                for (a, b) in screen.rows.iter().zip(&logical.rows) {
                    assert_eq!(a.glyphs.len(), b.glyphs.len());
                    for (g, source) in a.glyphs.iter().zip(&b.glyphs) {
                        assert!((g.pos.x - source.pos.x * scale as f32).abs() < 0.001);
                        assert!(
                            (g.advance_width - source.advance_width * scale as f32).abs() < 0.001
                        );
                    }
                }
                let mut offset = 0;
                for row in &screen.rows {
                    if let Some((index, gap)) = row.glyphs.iter().enumerate().find(|(i, g)| {
                        g.chr == ' ' && g.advance_width > 0. && *i + 1 < row.glyphs.len()
                    }) {
                        for (fraction, expected) in
                            [(0.25, offset + index), (0.75, offset + index + 1)]
                        {
                            let point = row.pos.to_vec2()
                                + egui::vec2(
                                    gap.pos.x + gap.advance_width * fraction,
                                    row.size.y * 0.5,
                                );
                            assert_eq!(screen.cursor_from_pos(point).index.0, expected);
                        }
                    }
                    offset += row.text().chars().count() + usize::from(row.ends_with_newline);
                }
                for i in 0..=c.text.chars().count() {
                    let caret = egui::text::CCursor::new(i);
                    let a = screen.pos_from_cursor(caret);
                    let b = logical.pos_from_cursor(caret);
                    assert!((a.min.x - b.min.x * scale as f32).abs() < 0.02);
                    assert!((a.min.y - b.min.y * scale as f32).abs() < 0.02);
                }
            }
        }
        assert_eq!(c.runs, runs);
    });
}
#[test]
fn paragraph_range_scope_preserves_every_character_property_and_history() {
    let mut d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    let mut t = text(&d, Surface::Canvas);
    t.content_mut().text = "First café\nSecond Ω\nThird 中文".into();
    t.content_mut().format(
        4..17,
        &TextStylePatch {
            family: Some(TextFamily::Monospace),
            size: Some(27.),
            bold: Some(true),
            italic: Some(true),
            underline: Some(true),
            rgb: Some([80, 120, 180]),
            ..Default::default()
        },
    );
    d.replace_texts(vec![t.clone()], false);
    let original = t.clone();
    let runs = t.content.runs.clone();
    let style = t.content.style.clone();
    t.content_mut()
        .format_paragraphs(12..12, |p| p.alignment = Alignment::Justify);
    assert_eq!(t.content.paragraph(0).alignment, Alignment::Left);
    assert_eq!(t.content.paragraph(1).alignment, Alignment::Justify);
    assert_eq!(t.content.paragraph(2).alignment, Alignment::Left);
    t.content_mut().format_paragraphs(3..18, |p| {
        p.alignment = Alignment::Right;
        p.line_spacing = 1.6;
    });
    assert_eq!(t.content.paragraph(0).alignment, Alignment::Right);
    assert_eq!(t.content.paragraph(1).alignment, Alignment::Right);
    assert_eq!(t.content.paragraph(2).alignment, Alignment::Left);
    assert_eq!(t.content.runs, runs);
    assert_eq!(t.content.style, style);
    d.replace_texts(vec![t.clone()], false);
    d.undo();
    assert_eq!(d.snapshot(t.id), Some(&original.into()));
    d.redo();
    assert_eq!(d.snapshot(t.id), Some(&t.into()));
}
#[test]
fn legacy_decorated_text_import_keeps_text_identity_geometry_and_resize_without_styling() {
    let d = Drawing::new(&DocumentData::blank(DocumentKind::Whiteboard));
    let t = text(&d, Surface::Canvas);
    let mut json = serde_json::to_value(&t).unwrap();
    json.as_object_mut().unwrap().remove("layout");
    json["box_style"] = serde_json::json!({"background":[200,100,30,255],"border":{"border_enabled":true},"padding":20.,"radius":8.,"opacity":70,"resize":"Scale"});
    let imported: TextObject = serde_json::from_value(json).unwrap();
    assert_eq!(imported.id, t.id);
    assert_eq!(imported.content, t.content);
    assert_eq!(imported.placement, t.placement);
    assert_eq!(imported.width, t.width);
    assert_eq!(imported.height, t.height);
    assert_eq!(imported.layout.resize, ResizeBehavior::Scale);
    let encoded = serde_json::to_value(&imported).unwrap();
    assert!(encoded.get("box_style").is_none());
    assert_eq!(encoded["layout"], serde_json::json!({"resize":"Scale"}));
}
