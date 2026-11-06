use super::tests::{click, frame, moved, pointer, screen, setup, stroke, t};
use super::*;
use crate::{
    document::DocumentKind,
    shape::{Shape, ShapeGeometry, ShapeKind},
};
fn near(a: Point, b: Point) {
    for i in 0..2 {
        assert!((a[i] - b[i]).abs() < 1e-5, "{a:?} != {b:?}");
    }
}
fn shape(c: &Canvas) -> &Shape {
    match &c.drawing.items()[0] {
        ObjectSnapshot::Shape(s) => s,
        _ => panic!("semantic shape"),
    }
}
fn add(c: &mut Canvas, s: Surface, kind: ShapeKind, a: Point, b: Point) -> Uuid {
    let shape = Shape::create(c.drawing.owner(), s, kind, a, b, false).unwrap();
    let id = shape.id;
    assert!(c.drawing.add_shape(shape));
    id
}
#[test]
fn all_shapes_create_in_every_direction_both_layouts_and_zoom() {
    for kind in ShapeKind::ALL {
        for layout in [DocumentKind::Page, DocumentKind::Whiteboard] {
            for scale in [0.25, 1., 4.] {
                for reverse_x in [false, true] {
                    for reverse_y in [false, true] {
                        let (mut c, s) = setup(layout);
                        let ctx = egui::Context::default();
                        let view = Transform {
                            scale,
                            origin: [10.; 2],
                            center: [0.; 2],
                        };
                        let start = [
                            if reverse_x { 130. } else { 70. },
                            if reverse_y { 130. } else { 70. },
                        ];
                        let end = [
                            if reverse_x { 70. } else { 130. },
                            if reverse_y { 70. } else { 130. },
                        ];
                        c.shape_kind = kind;
                        c.select(Tool::Shape);
                        frame(
                            &ctx,
                            &mut c,
                            s,
                            view,
                            vec![
                                moved(view, start),
                                pointer(screen(view, start), true, egui::Modifiers::NONE),
                                moved(view, end),
                            ],
                            0.,
                            true,
                        );
                        assert!(c.drawing.items().is_empty());
                        assert!(c.shape_draft.is_some());
                        frame(
                            &ctx,
                            &mut c,
                            s,
                            view,
                            vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
                            0.1,
                            true,
                        );
                        assert_eq!(c.drawing.items().len(), 1);
                        assert!(c.dirty());
                        assert!(c.drawing.strokes().is_empty());
                        let object = shape(&c);
                        assert_eq!(object.kind(), kind);
                        assert!(object.valid());
                        let id = object.id;
                        assert_eq!(c.selection.ids, HashSet::from([id]));
                        assert!(c.tool == Tool::Select);
                        if let ShapeGeometry::Box { size, .. } = object.geometry {
                            near(size, [60., 60.]);
                        }
                        if let Some(ends) = object.endpoints() {
                            near(ends[0], start);
                            near(ends[1], end);
                        }
                        let created = c.drawing.items().to_vec();
                        c.undo();
                        assert!(c.drawing.items().is_empty());
                        c.redo();
                        assert_eq!(c.drawing.items(), created);
                        assert!(c.selection.ids.contains(&id));
                    }
                }
            }
        }
    }
}
#[test]
fn shift_constraints_zero_size_escape_focus_and_camera_cancel() {
    for kind in [
        ShapeKind::Rectangle,
        ShapeKind::Ellipse,
        ShapeKind::Line,
        ShapeKind::Circle,
    ] {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        c.shape_kind = kind;
        c.select(Tool::Shape);
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![
                Event::ModifiersChanged(egui::Modifiers::SHIFT),
                pointer([70., 70.], true, egui::Modifiers::SHIFT),
                moved(t(), [140., 110.]),
                pointer([140., 110.], false, egui::Modifiers::SHIFT),
            ],
            0.,
            true,
        );
        let shape = shape(&c);
        if let Some(ends) = shape.endpoints() {
            let d = [ends[1][0] - ends[0][0], ends[1][1] - ends[0][1]];
            assert!((d[0] - d[1]).abs() < 1e-5);
        } else if let ShapeGeometry::Box { size, .. } = shape.geometry {
            near(size, [70., 70.]);
        }
    }
    for cancel in 0..4 {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        c.select(Tool::Shape);
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![
                pointer([100., 100.], true, egui::Modifiers::NONE),
                moved(t(), [160., 150.]),
            ],
            0.,
            true,
        );
        match cancel {
            0 => frame(
                &ctx,
                &mut c,
                s,
                t(),
                vec![Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                0.1,
                true,
            ),
            1 => frame(&ctx, &mut c, s, t(), vec![], 0.1, false),
            2 => frame(
                &ctx,
                &mut c,
                s,
                Transform { scale: 2., ..t() },
                vec![],
                0.1,
                true,
            ),
            _ => c.select(Tool::Pen),
        }
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![pointer([160., 150.], false, egui::Modifiers::NONE)],
            0.2,
            true,
        );
        assert!(c.drawing.items().is_empty());
        assert!(!c.drawing.can_undo());
        assert!(c.shape_cache.is_empty());
    }
    let (mut c, s) = setup(DocumentKind::Page);
    let ctx = egui::Context::default();
    c.select(Tool::Shape);
    click(&ctx, &mut c, s, t(), [100., 100.], egui::Modifiers::NONE);
    assert!(c.drawing.items().is_empty());
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            pointer([100., 100.], true, egui::Modifiers::NONE),
            moved(t(), [160., 150.]),
        ],
        1.,
        true,
    );
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![pointer([160., 100.], false, egui::Modifiers::NONE)],
        1.1,
        true,
    );
    assert!(c.drawing.items().is_empty());
    assert!(c.shape_cache.is_empty());
}
#[test]
fn mixed_selection_directional_lasso_and_topmost_order() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let ink = stroke(&mut c, s, [100., 110.], [180., 110.]);
    let rect = add(&mut c, s, ShapeKind::Rectangle, [90., 90.], [200., 140.]);
    let line = add(&mut c, s, ShapeKind::Line, [260., 90.], [400., 180.]);
    c.select(Tool::Select);
    click(&ctx, &mut c, s, t(), [150., 110.], egui::Modifiers::NONE);
    assert_eq!(c.selection.ids, HashSet::from([rect]));
    click(&ctx, &mut c, s, t(), [330., 135.], egui::Modifiers::SHIFT);
    assert_eq!(c.selection.ids, HashSet::from([rect, line]));
    click(&ctx, &mut c, s, t(), [330., 135.], egui::Modifiers::ALT);
    assert_eq!(c.selection.ids, HashSet::from([rect]));
    assert!(c.drawing.set_z_order(ink, 2));
    click(&ctx, &mut c, s, t(), [150., 110.], egui::Modifiers::NONE);
    assert_eq!(c.selection.ids, HashSet::from([ink]));
    for (start, end, expected) in [
        ([50., 50.], [350., 200.], HashSet::from([ink, rect])),
        ([350., 50.], [50., 200.], HashSet::from([ink, rect, line])),
    ] {
        let ctx = egui::Context::default();
        c.selection.clear();
        frame(&ctx, &mut c, s, t(), vec![], 0.9, true);
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![
                pointer(start, true, egui::Modifiers::NONE),
                moved(t(), end),
                pointer(end, false, egui::Modifiers::NONE),
            ],
            1.,
            true,
        );
        assert_eq!(c.selection.ids, expected);
    }
    c.selection.clear();
    c.select(Tool::Lasso);
    let ctx = egui::Context::default();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            pointer([50., 50.], true, egui::Modifiers::NONE),
            moved(t(), [220., 50.]),
            moved(t(), [220., 200.]),
            moved(t(), [50., 200.]),
            pointer([50., 50.], false, egui::Modifiers::NONE),
        ],
        2.,
        true,
    );
    assert_eq!(c.selection.ids, HashSet::from([ink, rect]));
    // Bounds alone must never select a distant point beside a diagonal segment.
    assert!(!c.drawing.object(line).unwrap().hit_test([275., 170.], 4.));
}
#[test]
fn mixed_transforms_flip_clipboard_cut_delete_preserve_geometry_and_identity() {
    for kind in ShapeKind::ALL {
        for layout in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let (mut c, s) = setup(layout);
            let ctx = egui::Context::default();
            let id = add(&mut c, s, kind, [100., 100.], [180., 150.]);
            let ink = stroke(&mut c, s, [110., 180.], [140., 190.]);
            c.selection_action(SelectionCommand::SelectAll, &ctx);
            let original = c.drawing.items().to_vec();
            let ids = HashSet::from([id, ink]);
            let delta = Placement {
                position: [10., 5.],
                ..Default::default()
            };
            assert!(c.drawing.transform_objects(&ids, delta));
            for (before, after) in original.iter().zip(c.drawing.items()) {
                let a = before.as_object().geometry_bounds().unwrap();
                let b = after.as_object().geometry_bounds().unwrap();
                near(delta.apply(a.min), b.min);
                near(delta.apply(a.max), b.max);
            }
            c.drawing.undo();
            assert_eq!(c.drawing.items(), original);

            for command in [
                SelectionCommand::FlipHorizontal,
                SelectionCommand::FlipVertical,
                SelectionCommand::RotateRight,
                SelectionCommand::Larger,
            ] {
                let before = c.drawing.items().to_vec();
                let revision = c.drawing.revision();
                c.selection_action(command, &ctx);
                assert_eq!(c.drawing.revision(), revision + 1);
                assert_eq!(c.selection.ids, ids);
                let changed = c.drawing.items().to_vec();
                c.undo();
                assert_eq!(c.drawing.items(), before);
                c.redo();
                assert_eq!(c.drawing.items(), changed);
            }
            let bundle = c.selected_bundle().unwrap();
            let text = bundle.encode().unwrap();
            let decoded = ObjectBundle::decode(&text).unwrap();
            let copies = decoded.duplicate(&mut c.drawing, s);
            assert_eq!(copies.len(), 2);
            assert!(copies.iter().all(|id| !ids.contains(id)));
            c.undo();
            assert_eq!(c.drawing.items().len(), 2);
            c.selection_action(SelectionCommand::Cut, &ctx);
            assert!(c.drawing.items().is_empty());
            c.undo();
            assert_eq!(c.drawing.items().len(), 2);
            c.selection_action(SelectionCommand::Delete, &ctx);
            assert!(c.drawing.items().is_empty());
            c.undo();
            for (before, after) in original.iter().zip(c.drawing.items()) {
                assert_eq!(before.id(), after.id());
            }
        }
    }
}
#[test]
fn endpoint_and_box_handle_edits_are_one_transaction_after_rotation_and_flip() {
    for scale in [0.25, 1., 4.] {
        for kind in [
            ShapeKind::Arrow,
            ShapeKind::Line,
            ShapeKind::Ellipse,
            ShapeKind::Circle,
            ShapeKind::Rectangle,
        ] {
            let (mut c, s) = setup(DocumentKind::Whiteboard);
            let ctx = egui::Context::default();
            let view = Transform {
                scale,
                origin: [300., 250.],
                center: [240., 225.],
            };
            let id = add(&mut c, s, kind, [200., 200.], [280., 250.]);
            c.selection_action(SelectionCommand::SelectAll, &ctx);
            c.selection_action(SelectionCommand::RotateRight, &ctx);
            c.selection_action(SelectionCommand::FlipHorizontal, &ctx);
            let source = shape(&c).clone();
            let handle = source
                .endpoints()
                .map(|p| p[1])
                .unwrap_or_else(|| source.box_corners().unwrap()[2]);
            let revision = c.drawing.revision();
            click(&ctx, &mut c, s, view, handle, egui::Modifiers::NONE);
            assert_eq!(
                c.drawing.revision(),
                revision,
                "A handle click must not edit geometry"
            );
            let target = [handle[0] + 35., handle[1] + 30.];
            let before = c.drawing.items().to_vec();
            let revision = c.drawing.revision();
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, handle),
                    pointer(screen(view, handle), true, egui::Modifiers::NONE),
                    moved(view, target),
                ],
                1.,
                true,
            );
            assert_eq!(c.drawing.items(), before);
            assert!(matches!(
                c.selection_gesture,
                Some(SelectionGesture::ShapeEdit { .. })
            ));
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, target), false, egui::Modifiers::NONE)],
                1.1,
                true,
            );
            assert_eq!(c.drawing.revision(), revision + 1);
            assert_eq!(shape(&c).id, id);
            assert_eq!(shape(&c).style, source.style);
            if let Some(ends) = shape(&c).endpoints() {
                near(
                    ends[1],
                    std::array::from_fn(|i| {
                        handle[i]
                            + view.to_document(screen(view, target).map(|v| f64::from(v as f32)))[i]
                            - view.to_document(screen(view, handle).map(|v| f64::from(v as f32)))[i]
                    }),
                );
                near(ends[0], source.endpoints().unwrap()[0]);
            }
            if kind == ShapeKind::Circle
                && let ShapeGeometry::Box { size, .. } = shape(&c).geometry
            {
                near([size[0], 0.], [size[1], 0.]);
            }
            c.undo();
            assert_eq!(c.drawing.items(), before);
        }
    }
}

#[test]
fn shape_meshes_are_cached_on_idle_and_history_keeps_mixed_order() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let id = add(&mut c, s, ShapeKind::Ellipse, [200., 200.], [280., 260.]);
    let ink = stroke(&mut c, s, [150., 220.], [320., 220.]);
    let id2 = add(&mut c, s, ShapeKind::Diamond, [300., 200.], [360., 260.]);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let mesh = c.shape_cache[&id].meshes[0].clone();
    frame(&ctx, &mut c, s, t(), vec![], 0.1, true);
    assert!(Arc::ptr_eq(&mesh, &c.shape_cache[&id].meshes[0]));
    c.drawing.erase(&HashSet::from([ink]));
    assert_eq!(
        c.drawing
            .items()
            .iter()
            .map(ObjectSnapshot::id)
            .collect::<Vec<_>>(),
        [id, id2]
    );
    c.drawing.undo();
    assert_eq!(
        c.drawing
            .items()
            .iter()
            .map(ObjectSnapshot::id)
            .collect::<Vec<_>>(),
        [id, ink, id2]
    );
    assert!(c.drawing.set_z_order(id2, 0));
    c.drawing.undo();
    assert_eq!(
        c.drawing
            .items()
            .iter()
            .map(ObjectSnapshot::id)
            .collect::<Vec<_>>(),
        [id, ink, id2]
    );
}

#[test]
fn configured_defaults_are_captured_on_creation_at_all_zoom_levels() {
    use crate::shape::BorderStyle;
    for layout in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.5, 1., 2., 4.] {
            let (mut c, s) = setup(layout);
            let ctx = egui::Context::default();
            let view = Transform {
                scale,
                origin: [10.; 2],
                center: [0.; 2],
            };
            c.shape_defaults.style.fill = None;
            c.shape_defaults.style.border_style = BorderStyle::Dotted;
            c.shape_defaults.style.border_width = 7.;
            c.shape_defaults.style.border[3] = 123;
            let defaults = c.shape_defaults.clone();
            c.select(Tool::Shape);
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, [40., 40.]),
                    pointer(screen(view, [40., 40.]), true, egui::Modifiers::NONE),
                ],
                0.,
                true,
            );
            c.shape_defaults = Default::default();
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, [100., 100.]),
                    pointer(screen(view, [100., 100.]), false, egui::Modifiers::NONE),
                ],
                0.1,
                true,
            );
            assert_eq!(shape(&c).style, defaults.style);
            let created = c.drawing.items().to_vec();
            c.undo();
            assert!(c.drawing.items().is_empty());
            c.redo();
            assert_eq!(c.drawing.items(), created);
        }
    }
}

#[test]
fn formatted_rendering_uses_alpha_patterns_and_no_border_without_geometry_changes() {
    use crate::shape::{BorderStyle, StylePatch};
    for scale in [0.5, 1., 2., 4.] {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        let view = Transform {
            scale,
            origin: [0.; 2],
            center: [0.; 2],
        };
        let id = add(&mut c, s, ShapeKind::Rectangle, [40., 40.], [120., 100.]);
        let ids = HashSet::from([id]);
        let geometry = shape(&c).geometry.clone();
        c.drawing.format_shapes(&ids, StylePatch::Fill(None), false);
        c.drawing
            .format_shapes(&ids, StylePatch::BorderEnabled(false), false);
        frame(&ctx, &mut c, s, view, vec![], 0., true);
        assert!(
            c.shape_cache[&id].meshes.is_empty(),
            "No-fill/no-border must paint nothing"
        );
        c.drawing
            .format_shapes(&ids, StylePatch::Fill(Some([21, 80, 230])), false);
        c.drawing
            .format_shapes(&ids, StylePatch::FillOpacity(128), false);
        frame(&ctx, &mut c, s, view, vec![], 0.1, true);
        let color = egui::Color32::from_rgba_unmultiplied(21, 80, 230, 128);
        assert!(
            c.shape_cache[&id]
                .meshes
                .iter()
                .flat_map(|m| &m.vertices)
                .any(|v| v.color == color)
        );
        c.drawing
            .format_shapes(&ids, StylePatch::FillOpacity(0), false);
        frame(&ctx, &mut c, s, view, vec![], 0.2, true);
        assert!(c.shape_cache[&id].meshes.is_empty());
        c.drawing
            .format_shapes(&ids, StylePatch::BorderEnabled(true), false);
        for pattern in [BorderStyle::Solid, BorderStyle::Dashed, BorderStyle::Dotted] {
            c.drawing
                .format_shapes(&ids, StylePatch::Pattern(pattern), false);
            frame(&ctx, &mut c, s, view, vec![], 0.3, true);
            assert!(!c.shape_cache[&id].meshes.is_empty());
            assert_eq!(shape(&c).geometry, geometry);
        }
    }
}

#[test]
fn final_slider_sample_on_release_merges_into_one_mixed_selection_undo() {
    use crate::shape::StylePatch;
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, ShapeKind::Rectangle, [40., 40.], [100., 100.]);
    let mut second = Shape::create(
        c.drawing.owner(),
        s,
        ShapeKind::Rectangle,
        [140., 40.],
        [200., 100.],
        false,
    )
    .unwrap();
    second.style.border_width = 8.;
    let b = second.id;
    c.drawing.add_shape(second);
    c.select(Tool::Select);
    c.selection.change([a, b], Membership::Replace, &c.drawing);
    let before = c.drawing.items().to_vec();
    let ctx = egui::Context::default();
    let id = egui::Id::unique("continuous-width");
    for (events, width) in [
        (vec![pointer([700., 500.], true, egui::Modifiers::NONE)], 4.),
        (vec![Event::PointerMoved(egui::pos2(710., 500.))], 6.),
        (
            vec![pointer([720., 500.], false, egui::Modifiers::NONE)],
            12.,
        ),
    ] {
        crate::ui::settings::tests::render(&ctx, events, |ui| {
            c.begin_frame(ui);
            c.selection_controls(
                ui,
                Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.)),
                s,
                t(),
            );
            assert!(c.apply_shape_style(
                StylePatch::Width(width),
                id,
                ui.input(|i| i.pointer.primary_down())
            ));
            c.end_frame(ui);
        });
    }
    let after = c.drawing.items().to_vec();
    c.undo();
    assert_eq!(c.drawing.items(), before);
    c.redo();
    assert_eq!(c.drawing.items(), after);
}
