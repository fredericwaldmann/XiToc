//! Styling uses the same semantic snapshots/history as manipulation.
use crate::{
    document::{DocumentData, DocumentKind},
    drawing::{Drawing, Surface},
    object::{ObjectSnapshot, Placement},
    shape::*,
};
use std::collections::HashSet;
fn fixture() -> (Drawing, Vec<Shape>) {
    let data = DocumentData::blank(DocumentKind::Whiteboard);
    let mut drawing = Drawing::new(&data);
    let shapes: Vec<_> = ShapeKind::ALL
        .iter()
        .map(|&kind| {
            Shape::create(
                data.id(),
                Surface::Canvas,
                kind,
                [20., 30.],
                [120., 100.],
                false,
            )
            .unwrap()
        })
        .collect();
    for shape in &shapes {
        assert!(drawing.add_shape(shape.clone()));
    }
    (drawing, shapes)
}
#[test]
fn properties_batch_history_preserve_objects_and_skip_incompatible() {
    for patch in [
        StylePatch::Fill(Some([3, 55, 240])),
        StylePatch::Fill(None),
        StylePatch::FillOpacity(0),
        StylePatch::FillOpacity(255),
        StylePatch::Border([200, 12, 67]),
        StylePatch::BorderOpacity(64),
        StylePatch::Width(8.5),
        StylePatch::Pattern(BorderStyle::Dashed),
        StylePatch::Pattern(BorderStyle::Dotted),
        StylePatch::DashLength(12.),
        StylePatch::GapLength(6.),
        StylePatch::DotSpacing(9.),
        StylePatch::DotSize(3.),
        StylePatch::BorderEnabled(false),
        StylePatch::Heads(true, true),
        StylePatch::Heads(false, false),
        StylePatch::Radius(300.),
    ] {
        let (mut drawing, shapes) = fixture();
        let ids = shapes.iter().map(|s| s.id).collect();
        let before = drawing.items().to_vec();
        assert!(drawing.format_shapes(&ids, patch, false), "{patch:?}");
        let after = drawing.items().to_vec();
        for (object, source) in after.iter().zip(&shapes) {
            let ObjectSnapshot::Shape(shape) = object else {
                panic!()
            };
            match patch {
                StylePatch::Fill(value) => assert_eq!(
                    shape.style.fill,
                    if source.kind().segment() {
                        source.style.fill
                    } else {
                        value
                    }
                ),
                StylePatch::FillOpacity(value) => assert_eq!(
                    shape.style.fill_opacity,
                    if source.kind().segment() {
                        source.style.fill_opacity
                    } else {
                        value
                    }
                ),
                StylePatch::Border(value) => assert_eq!(&shape.style.border[..3], &value),
                StylePatch::BorderOpacity(value) => assert_eq!(shape.style.border[3], value),
                StylePatch::Width(value) => assert_eq!(shape.style.border_width, value),
                StylePatch::Pattern(value) => assert_eq!(shape.style.border_style, value),
                StylePatch::DashLength(v) => assert_eq!(shape.style.pattern.dash, v),
                StylePatch::GapLength(v) => assert_eq!(shape.style.pattern.gap, v),
                StylePatch::DotSpacing(v) => assert_eq!(shape.style.pattern.dot_spacing, v),
                StylePatch::DotSize(v) => assert_eq!(shape.style.pattern.dot_size, v),
                StylePatch::BorderEnabled(value) => assert_eq!(shape.style.border_enabled, value),
                StylePatch::Heads(start, end) => {
                    if source.kind().segment() {
                        assert_eq!(shape.style.start_head, start);
                        assert!(
                            matches!(shape.geometry, ShapeGeometry::Segment { head, .. } if head.is_some() == end)
                        );
                    } else {
                        assert_eq!(shape.geometry, source.geometry);
                    }
                }
                StylePatch::Radius(_) => {
                    if let ShapeGeometry::Box {
                        kind: ShapeKind::RoundedRectangle,
                        corner_radius,
                        size,
                    } = shape.geometry
                    {
                        assert_eq!(corner_radius, size[0].min(size[1]) * 0.5);
                    } else {
                        assert_eq!(shape.geometry, source.geometry);
                    }
                }
            }
            if !matches!(patch, StylePatch::Heads(..) | StylePatch::Radius(_)) {
                assert_eq!(shape.geometry, source.geometry);
            }
            assert_eq!(shape.id, source.id);
            assert_eq!(shape.placement, source.placement);
            assert!(shape.valid());
        }
        drawing.undo();
        assert_eq!(drawing.items(), before);
        drawing.redo();
        assert_eq!(drawing.items(), after);
        let bundle =
            crate::object_clipboard::ObjectBundle::capture_objects(drawing.items().iter().cloned())
                .unwrap();
        assert!(!bundle.encode().unwrap().is_empty());
    }
}
#[test]
fn continuous_mixed_widths_coalesce_exactly_and_reject_bad_width_atomically() {
    let (mut drawing, shapes) = fixture();
    let ids = shapes.iter().map(|s| s.id).collect();
    let first: HashSet<_> = [shapes[0].id].into_iter().collect();
    drawing.format_shapes(&first, StylePatch::Width(4.), false);
    let before = drawing.items().to_vec();
    assert!(drawing.format_shapes(&ids, StylePatch::Width(4.), false));
    for width in [5., 6., 8., 12., 24.] {
        assert!(drawing.format_shapes(&ids, StylePatch::Width(width), true));
    }
    let after = drawing.items().to_vec();
    drawing.undo();
    assert_eq!(drawing.items(), before);
    drawing.redo();
    assert_eq!(drawing.items(), after);
    for width in [f64::NAN, f64::INFINITY, -1., 0., 25.] {
        assert!(!drawing.format_shapes(&ids, StylePatch::Width(width), false));
        assert_eq!(drawing.items(), after);
    }
    drawing.undo();
    drawing.format_shapes(&ids, StylePatch::BorderOpacity(0), false);
    assert!(!drawing.can_redo());
}
#[test]
fn independent_heads_survive_transforms_endpoints_and_corner_clamping() {
    let (_, shapes) = fixture();
    for shape in shapes.iter().filter(|s| s.kind().segment()) {
        for (start, end) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut shape = shape.clone();
            StylePatch::Heads(start, end).apply(&mut shape);
            for rotation in [0., 0.7, 2.] {
                for reflected in [[false, false], [true, false], [false, true]] {
                    shape.placement = Placement {
                        rotation,
                        reflected,
                        scale: 2.,
                        ..Default::default()
                    };
                    let moved = shape.with_endpoint(1, [170., 210.], false).unwrap();
                    let paths = moved.contours(0.02);
                    assert_eq!(paths.len(), 1 + usize::from(start) + usize::from(end));
                    let endpoints = moved.endpoints().unwrap();
                    let mut i = 1;
                    if end {
                        assert_eq!(paths[i][0], endpoints[1]);
                        i += 1;
                    }
                    if start {
                        assert_eq!(paths[i][0], endpoints[0]);
                    }
                    for p in paths.into_iter().flatten() {
                        assert!(moved.geometry_bounds().expand(1e-8).contains(p));
                    }
                    let json = serde_json::to_string(&moved).unwrap();
                    assert_eq!(serde_json::from_str::<Shape>(&json).unwrap(), moved);
                }
            }
        }
    }
    let mut rounded = shapes[1].clone();
    StylePatch::Radius(-10.).apply(&mut rounded);
    if let ShapeGeometry::Box { corner_radius, .. } = rounded.geometry {
        assert_eq!(corner_radius, 0.);
    }
    StylePatch::Radius(500.).apply(&mut rounded);
    let resized = rounded.with_corner(2, [40., 45.], false).unwrap();
    assert!(resized.valid());
    assert!(resized.contours(0.01)[0].len() > 16);
}
#[test]
fn defaults_round_trip_and_do_not_change_existing_styles() {
    let mut prefs = crate::settings::Preferences::default();
    prefs.drawing.shape_defaults.style.fill = None;
    prefs.drawing.shape_defaults.style.border_style = BorderStyle::Dotted;
    prefs.drawing.shape_defaults.style.border[3] = 120;
    let mut storage = crate::settings::tests::MemoryStorage::default();
    prefs.store(&mut storage);
    assert_eq!(
        crate::settings::Preferences::load(Some(&storage)).unwrap(),
        prefs
    );
    let data = DocumentData::blank(DocumentKind::Whiteboard);
    let mut canvas = crate::canvas::Canvas::new(&data);
    let shape = Shape::create(
        data.id(),
        Surface::Canvas,
        ShapeKind::Rectangle,
        [10., 10.],
        [50., 50.],
        false,
    )
    .unwrap();
    canvas.drawing.add_shape(shape.clone());
    prefs.apply_drawing(&mut canvas);
    assert_eq!(canvas.shape_defaults, prefs.drawing.shape_defaults);
    assert_eq!(canvas.drawing.items(), &[ObjectSnapshot::from(shape)]);
    let mut invalid = serde_json::to_value(&prefs).unwrap();
    invalid["drawing"]["shape_defaults"]["style"]["border_width"] = (-1).into();
    assert!(crate::settings::Preferences::decode(&invalid.to_string()).is_err());
}

#[test]
fn formatting_leaves_ink_and_unselected_shapes_untouched() {
    use crate::drawing::Stroke;
    use std::sync::Arc;
    let (mut drawing, shapes) = fixture();
    let mut ink = Stroke::new(drawing.owner(), Surface::Canvas, [23, 45, 67, 180], 3.);
    ink.push([12., 14.]);
    ink.push([160., 180.]);
    let ink_id = ink.id;
    assert!(drawing.add(ink));
    let ids = HashSet::from([shapes[0].id, shapes[7].id, ink_id]);
    let before = drawing.items().to_vec();
    assert!(drawing.format_shapes(&ids, StylePatch::Fill(Some([0, 80, 255])), false));
    let ObjectSnapshot::Shape(rect) = &drawing.items()[0] else {
        panic!()
    };
    assert_eq!(rect.style.fill, Some([0, 80, 255]));
    assert_eq!(rect.geometry, shapes[0].geometry);
    for (a, b) in before.iter().zip(drawing.items()).skip(1) {
        assert_eq!(a, b);
        match (a, b) {
            (ObjectSnapshot::Stroke(a), ObjectSnapshot::Stroke(b)) => assert!(Arc::ptr_eq(a, b)),
            (ObjectSnapshot::Shape(a), ObjectSnapshot::Shape(b)) => assert!(Arc::ptr_eq(a, b)),
            _ => panic!(),
        }
    }
    drawing.undo();
    assert_eq!(drawing.items(), before);
    // An entirely incompatible action must not alter history/revision.
    let revision = drawing.revision();
    assert!(!drawing.format_shapes(
        &HashSet::from([shapes[7].id, ink_id]),
        StylePatch::Fill(None),
        false
    ));
    assert_eq!(drawing.revision(), revision);
}

#[test]
fn legacy_style_defaults_and_default_geometry_are_validated() {
    let (_, shapes) = fixture();
    let mut value = serde_json::to_value(&shapes[7]).unwrap();
    value["style"]
        .as_object_mut()
        .unwrap()
        .remove("border_enabled");
    value["style"].as_object_mut().unwrap().remove("start_head");
    let legacy: Shape = serde_json::from_value(value).unwrap();
    assert!(legacy.style.border_enabled);
    assert!(!legacy.style.start_head);
    let mut defaults = ShapeDefaults {
        corner_radius: Some(500.),
        arrow_end: false,
        ..Default::default()
    };
    defaults.style.start_head = true;
    let mut rounded = shapes[1].clone();
    defaults.apply(&mut rounded);
    let ShapeGeometry::Box {
        corner_radius,
        size,
        ..
    } = rounded.geometry
    else {
        panic!()
    };
    assert_eq!(corner_radius, size[0].min(size[1]) * 0.5);
    let mut arrow = shapes[7].clone();
    defaults.apply(&mut arrow);
    assert_eq!(arrow.contours(0.01).len(), 2);
    assert!(arrow.style.start_head);
    assert!(matches!(
        arrow.geometry,
        ShapeGeometry::Segment { head: None, .. }
    ));
    assert_eq!(
        serde_json::from_str::<ShapeDefaults>(&serde_json::to_string(&defaults).unwrap()).unwrap(),
        defaults
    );
    defaults.corner_radius = Some(-1.);
    assert!(!defaults.valid());
    defaults.corner_radius = Some(f64::NAN);
    assert!(!defaults.valid());
}

#[test]
fn custom_patterns_survive_transform_clipboard_history_and_ignore_thickness() {
    let (mut drawing, shapes) = fixture();
    let ids = shapes.iter().map(|s| s.id).collect();
    for patch in [
        StylePatch::DashLength(12.),
        StylePatch::GapLength(6.),
        StylePatch::DotSpacing(10.),
        StylePatch::DotSize(3.),
    ] {
        assert!(drawing.format_shapes(&ids, patch, false));
    }
    let styled = drawing.items().to_vec();
    for width in [1., 2., 12., 24.] {
        drawing.format_shapes(&ids, StylePatch::Width(width), false);
        for object in drawing.items() {
            let ObjectSnapshot::Shape(s) = object else {
                panic!()
            };
            assert_eq!(
                s.style.pattern,
                crate::stroke_pattern::PatternSettings {
                    dash: 12.,
                    gap: 6.,
                    dot_spacing: 10.,
                    dot_size: 3.
                }
            );
        }
    }
    for object in &styled {
        let ObjectSnapshot::Shape(source) = object else {
            panic!()
        };
        let transformed = object.transformed(Placement {
            rotation: 0.6,
            reflected: [true, false],
            scale: 2.,
            position: [20., 30.],
            ..Default::default()
        });
        let ObjectSnapshot::Shape(target) = transformed else {
            panic!()
        };
        assert_eq!(source.style, target.style);
    }
    let bundle =
        crate::object_clipboard::ObjectBundle::capture_objects(styled.iter().cloned()).unwrap();
    let bundle = crate::object_clipboard::ObjectBundle::decode(&bundle.encode().unwrap()).unwrap();
    let duplicate = bundle.duplicate(&mut drawing, Surface::Canvas);
    assert_eq!(duplicate.len(), shapes.len());
    for source in drawing
        .items()
        .iter()
        .filter(|s| duplicate.contains(&s.id()))
    {
        let ObjectSnapshot::Shape(s) = source else {
            panic!()
        };
        assert_eq!(s.style.pattern.dash, 12.);
        assert_eq!(s.style.pattern.gap, 6.);
    }
    drawing.undo();
    assert_eq!(drawing.items().len(), shapes.len());
    drawing.redo();
    assert_eq!(drawing.items().len(), shapes.len() * 2);
    let pasted = bundle.paste(&mut drawing, Surface::Canvas, [500., 500.]);
    assert_eq!(pasted.len(), shapes.len());
    let mut legacy = serde_json::to_value(&shapes[0]).unwrap();
    legacy["style"].as_object_mut().unwrap().remove("pattern");
    let legacy: Shape = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.style.pattern, Default::default());
    for invalid in [f64::NAN, f64::INFINITY, 0., -1., 257.] {
        assert!(!drawing.format_shapes(&ids, StylePatch::DashLength(invalid), false));
    }
}
