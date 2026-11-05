//! Pointer-stream regressions for shape movement and affine preview rendering.
use super::tests::{frame, moved, pointer, setup, t};
use super::*;
use crate::{
    document::DocumentKind,
    shape::{Shape, ShapeKind},
};

fn near(a: Point, b: Point) {
    for i in 0..2 {
        assert!((a[i] - b[i]).abs() < 0.001, "{a:?} != {b:?}");
    }
}
fn add(c: &mut Canvas, s: Surface, kind: ShapeKind, a: Point, b: Point) -> Uuid {
    let shape = Shape::create(c.drawing.owner(), s, kind, a, b, false).unwrap();
    let id = shape.id;
    assert!(c.drawing.add_shape(shape));
    id
}
#[test]
fn body_drag_does_not_activate_hidden_axis_aligned_resize_handles() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let id = add(&mut c, s, ShapeKind::Diamond, [200., 200.], [300., 300.]);
    assert!(c.drawing.transform_objects(
        &HashSet::from([id]),
        Placement {
            origin: [250., 250.],
            position: [250., 250.],
            rotation: std::f64::consts::FRAC_PI_4,
            ..Default::default()
        }
    ));
    c.select(Tool::Select);
    c.selection.change([id], Membership::Replace, &c.drawing);
    let bounds = c.selection.bounds.unwrap();
    let start = bounds.min.map(|v| v + 3.);
    assert!(c.drawing.object(id).unwrap().hit_test(start, 0.));
    assert!(
        c.shape_handle(start, 1.).is_none(),
        "This body point is not a visible local shape handle"
    );
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            moved(t(), start),
            pointer(start, true, egui::Modifiers::NONE),
            moved(t(), [start[0] + 24., start[1] + 16.]),
        ],
        0.1,
        true,
    );
    assert!(
        matches!(
            c.selection_gesture,
            Some(SelectionGesture::Transform { corner: None, .. })
        ),
        "Dragging visible shape content must move, not scale via a hidden AABB handle"
    );
    let delta = c.selection_preview(s).unwrap();
    assert_eq!(delta.scale, 1.);
    assert_eq!(delta.rotation, 0.);
}
#[test]
fn shape_handle_drag_preserves_grab_offset_within_hit_radius() {
    for kind in [ShapeKind::Arrow, ShapeKind::Rectangle] {
        for scale in [0.5, 1., 2., 4.] {
            let (mut c, s) = setup(DocumentKind::Whiteboard);
            let ctx = egui::Context::default();
            let id = add(&mut c, s, kind, [200., 200.], [300., 250.]);
            c.select(Tool::Select);
            c.selection.change([id], Membership::Replace, &c.drawing);
            let view = Transform {
                origin: [350., 260.],
                center: [300., 250.],
                scale,
            };
            let start = [353., 259.];
            let end = [373., 269.];
            frame(&ctx, &mut c, s, view, vec![], 0., true);
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    pointer(start, true, egui::Modifiers::NONE),
                    Event::PointerMoved(egui::pos2(end[0] as f32, end[1] as f32)),
                ],
                0.1,
                true,
            );
            let preview = c.shape_edit_preview().unwrap();
            let handle = preview
                .endpoints()
                .map(|p| p[1])
                .unwrap_or_else(|| preview.box_corners().unwrap()[2]);
            near(handle, [300. + 20. / scale, 250. + 10. / scale]);
        }
    }
}
#[test]
fn translation_preview_reuses_original_shape_meshes() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let id = add(&mut c, s, ShapeKind::Ellipse, [200., 200.], [320., 280.]);
    c.select(Tool::Select);
    c.selection.change([id], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let base = c.shape_cache[&id].meshes.clone();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            moved(t(), [245., 230.]),
            pointer([245., 230.], true, egui::Modifiers::NONE),
        ],
        0.1,
        true,
    );
    for (i, point) in [[260., 240.], [265.25, 248.5], [251., 235.], [260., 240.]]
        .into_iter()
        .enumerate()
    {
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![moved(t(), point)],
            0.2 + i as f64 * 0.1,
            true,
        );
        near(c.selection_preview(s).unwrap().apply([245., 230.]), point);
        assert!(
            Arc::ptr_eq(&base[0], &c.shape_cache[&id].meshes[0]),
            "A position-only preview must not rebuild contours and tessellate the shape on each pointer update"
        );
    }
}

#[test]
fn all_shapes_follow_pointer_snapshot_under_zoom_pan_rotation_and_multi_selection() {
    for kind in ShapeKind::ALL {
        for layout in [DocumentKind::Page, DocumentKind::Whiteboard] {
            for zoom in [0.5, 1., 2., 4.] {
                for pan in [false, true] {
                    for rotated in [false, true] {
                        for group in [false, true] {
                            let (mut c, s) = setup(layout);
                            let ctx = egui::Context::default();
                            let id = add(&mut c, s, kind, [100., 100.], [180., 160.]);
                            let other = add(
                                &mut c,
                                s,
                                ShapeKind::RoundedRectangle,
                                [250., 210.],
                                [300., 250.],
                            );
                            let mut ids = HashSet::from([id]);
                            if group {
                                ids.insert(other);
                            }
                            let placement = Placement {
                                origin: [140., 130.],
                                position: [140., 130.],
                                rotation: if rotated { 0.47 } else { 0. },
                                ..Default::default()
                            };
                            if rotated {
                                assert!(c.drawing.transform_objects(&ids, placement));
                            }
                            c.select(Tool::Select);
                            c.selection
                                .change(ids.clone(), Membership::Replace, &c.drawing);
                            let view = Transform {
                                origin: [350., 260.],
                                center: if pan { [170., 110.] } else { [140., 130.] },
                                scale: zoom,
                            };
                            let grab = placement.apply(if kind.segment() {
                                [140., 130.]
                            } else {
                                [135., 135.]
                            });
                            let start = view.to_screen(grab).map(|v| f64::from(v as f32));
                            let pointer_start = view.to_document(start);
                            frame(&ctx, &mut c, s, view, vec![], 0., true);
                            let base = c.shape_cache[&id].meshes.clone();
                            let original = c.drawing.items().to_vec();
                            let revision = c.drawing.revision();
                            frame(
                                &ctx,
                                &mut c,
                                s,
                                view,
                                vec![pointer(start, true, egui::Modifiers::NONE)],
                                0.1,
                                true,
                            );
                            assert!(
                                matches!(
                                    c.selection_gesture,
                                    Some(SelectionGesture::Transform {
                                        corner: None,
                                        moved: false,
                                        ..
                                    })
                                ),
                                "{kind:?} zoom={zoom} rotated={rotated} group={group}"
                            );
                            assert!(c.selection_preview(s).is_none());
                            assert_eq!(c.selection.ids, ids);
                            for (i, offset) in [
                                [4., 2.],
                                [7.25, -5.5],
                                [25., 11.],
                                [7.25, -5.5],
                                [0., 0.],
                                [40., 20.],
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                let target: Point = std::array::from_fn(|axis| {
                                    f64::from((start[axis] + offset[axis]) as f32)
                                });
                                frame(
                                    &ctx,
                                    &mut c,
                                    s,
                                    view,
                                    vec![Event::PointerMoved(egui::pos2(
                                        target[0] as f32,
                                        target[1] as f32,
                                    ))],
                                    0.2 + i as f64 * 0.1,
                                    true,
                                );
                                let preview = c.selection_preview(s).unwrap();
                                near(preview.apply(pointer_start), view.to_document(target));
                                assert_eq!(preview.scale, 1.);
                                assert_eq!(preview.rotation, 0.);
                                assert_eq!(preview.reflected, [false; 2]);
                                assert_eq!(c.drawing.items(), original);
                                assert_eq!(c.drawing.revision(), revision);
                                assert_eq!(c.selection.ids, ids);
                                assert!(Arc::ptr_eq(&base[0], &c.shape_cache[&id].meshes[0]));
                                // A stationary repaint must retain the same preview, not alternate
                                // between identity and movement as the shared graphics layer resets.
                                frame(&ctx, &mut c, s, view, vec![], 0.25 + i as f64 * 0.1, true);
                                assert_eq!(c.selection_preview(s), Some(preview));
                                if !preview.is_identity() {
                                    let layer = ctx
                                        .layer_transform_to_global(preview_layer_id(
                                            c.drawing.owner(),
                                        ))
                                        .unwrap();
                                    let rendered =
                                        layer * egui::pos2(start[0] as f32, start[1] as f32);
                                    near([rendered.x as f64, rendered.y as f64], target);
                                }
                            }
                            let end: Point =
                                std::array::from_fn(|axis| start[axis] + [40., 20.][axis]);
                            frame(
                                &ctx,
                                &mut c,
                                s,
                                view,
                                vec![pointer(end, false, egui::Modifiers::NONE)],
                                1.,
                                true,
                            );
                            assert_eq!(c.drawing.revision(), revision + 1);
                            assert_eq!(c.selection.ids, ids);
                            let final_objects = c.drawing.items().to_vec();
                            for (before, after) in original.iter().zip(&final_objects) {
                                if ids.contains(&before.id()) {
                                    let before_shape = match before {
                                        ObjectSnapshot::Shape(s) => s,
                                        _ => unreachable!(),
                                    };
                                    let after_shape = match after {
                                        ObjectSnapshot::Shape(s) => s,
                                        _ => unreachable!(),
                                    };
                                    assert_eq!(before_shape.geometry, after_shape.geometry);
                                    assert_eq!(before_shape.style, after_shape.style);
                                    assert_eq!(
                                        before_shape.placement.rotation,
                                        after_shape.placement.rotation
                                    );
                                    assert_eq!(
                                        before_shape.placement.scale,
                                        after_shape.placement.scale
                                    );
                                    let a = before_shape.geometry_bounds();
                                    let b = after_shape.geometry_bounds();
                                    near(
                                        [b.min[0] - a.min[0], b.min[1] - a.min[1]],
                                        [40. / zoom, 20. / zoom],
                                    );
                                } else {
                                    assert_eq!(before, after);
                                }
                            }
                            c.undo();
                            assert_eq!(c.drawing.items(), original);
                            c.redo();
                            assert_eq!(c.drawing.items(), final_objects);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn click_cancel_tool_handoff_and_return_to_start_never_create_move_history() {
    for mode in [Tool::Select, Tool::Lasso] {
        for cancel in 0..5 {
            let (mut c, s) = setup(DocumentKind::Whiteboard);
            let ctx = egui::Context::default();
            let id = add(&mut c, s, ShapeKind::Rectangle, [200., 200.], [320., 280.]);
            c.select(mode);
            c.selection.change([id], Membership::Replace, &c.drawing);
            let original = c.drawing.items().to_vec();
            let revision = c.drawing.revision();
            frame(&ctx, &mut c, s, t(), vec![], 0., true);
            frame(
                &ctx,
                &mut c,
                s,
                t(),
                vec![pointer([245., 230.], true, egui::Modifiers::NONE)],
                0.1,
                true,
            );
            if cancel != 0 {
                frame(
                    &ctx,
                    &mut c,
                    s,
                    t(),
                    vec![moved(t(), [270., 245.])],
                    0.2,
                    true,
                );
            }
            match cancel {
                1 => {
                    frame(
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
                        0.3,
                        true,
                    );
                }
                2 => c.select(Tool::Pen),
                3 => frame(&ctx, &mut c, s, t(), vec![], 0.3, false),
                4 => frame(
                    &ctx,
                    &mut c,
                    s,
                    t(),
                    vec![moved(t(), [245., 230.])],
                    0.3,
                    true,
                ),
                _ => {}
            }
            frame(
                &ctx,
                &mut c,
                s,
                t(),
                vec![pointer([245., 230.], false, egui::Modifiers::NONE)],
                0.4,
                true,
            );
            assert_eq!(c.drawing.items(), original);
            assert_eq!(c.drawing.revision(), revision);
            c.select(mode);
            frame(&ctx, &mut c, s, t(), vec![], 0.5, true);
            assert_eq!(c.drawing.items(), original);
            assert!(
                ctx.layer_transform_to_global(preview_layer_id(c.drawing.owner()))
                    .is_none()
            );
        }
    }
}

#[test]
fn shared_ink_and_shape_drag_reuses_both_meshes_and_commits_once() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let shape = add(&mut c, s, ShapeKind::Ellipse, [200., 200.], [320., 280.]);
    let ink = super::tests::stroke(&mut c, s, [350., 220.], [390., 250.]);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let before = c.drawing.items().to_vec();
    let revision = c.drawing.revision();
    let shape_mesh = c.shape_cache[&shape].meshes[0].clone();
    let ink_mesh = c.cache[&ink].meshes[0].clone();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            pointer([245., 230.], true, egui::Modifiers::NONE),
            moved(t(), [275., 250.]),
        ],
        0.1,
        true,
    );
    assert_eq!(c.selection.ids, HashSet::from([shape, ink]));
    assert_eq!(c.drawing.items(), before);
    assert!(Arc::ptr_eq(&shape_mesh, &c.shape_cache[&shape].meshes[0]));
    assert!(Arc::ptr_eq(&ink_mesh, &c.cache[&ink].meshes[0]));
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![pointer([275., 250.], false, egui::Modifiers::NONE)],
        0.2,
        true,
    );
    assert_eq!(c.drawing.revision(), revision + 1);
    c.undo();
    assert_eq!(c.drawing.items(), before);
}

#[test]
#[ignore = "manual release-mode shape movement preview benchmark"]
fn shape_drag_preview_performance() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let objects = (0..1000)
        .map(|i| {
            let start = [20. + f64::from(i % 40) * 14., 20. + f64::from(i / 40) * 18.];
            ObjectSnapshot::from(
                Shape::create(
                    c.drawing.owner(),
                    s,
                    ShapeKind::Ellipse,
                    start,
                    [start[0] + 8., start[1] + 8.],
                    false,
                )
                .unwrap(),
            )
        })
        .collect();
    assert!(c.drawing.insert_snapshots(objects));
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let meshes: Vec<_> = c
        .shape_cache
        .iter()
        .map(|(id, c)| (*id, c.meshes[0].clone()))
        .collect();
    assert_eq!(meshes.len(), 1000);
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![pointer([24., 24.], true, egui::Modifiers::NONE)],
        0.1,
        true,
    );
    let revision = c.drawing.revision();
    let start = std::time::Instant::now();
    for i in 0..40 {
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![moved(
                t(),
                [35. + f64::from(i % 10), 32. + f64::from(i % 5)],
            )],
            0.2 + f64::from(i) * 0.01,
            true,
        );
    }
    eprintln!(
        "1,000 ellipses / 40 changed movement preview frames: {:?}",
        start.elapsed()
    );
    assert_eq!(c.drawing.revision(), revision);
    assert!(
        meshes
            .iter()
            .all(|(id, m)| Arc::ptr_eq(m, &c.shape_cache[id].meshes[0]))
    );
}

#[test]
fn translated_preview_clip_remains_the_viewport_for_large_jumps() {
    let ctx = egui::Context::default();
    let viewport = Rect::from_min_max(egui::pos2(100., 80.), egui::pos2(800., 600.));
    for displacement in [[50., 20.], [700., 420.], [-900., -600.]] {
        let delta = Placement {
            position: displacement,
            ..Default::default()
        };
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.painter().with_clip_rect(viewport);
            let selected = selection_preview_painter(ui, &painter, t(), delta, Uuid::nil());
            let layer = ui
                .ctx()
                .layer_transform_to_global(selected.layer_id())
                .unwrap();
            assert_eq!(
                layer * selected.clip_rect(),
                painter.clip_rect(),
                "visual clipping must not shrink as a selection moves"
            );
        });
        output.textures_delta.clear();
    }
}

#[test]
fn hover_loss_during_held_drag_does_not_cancel_selection_session() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let ids = [
        add(&mut c, s, ShapeKind::Rectangle, [100., 100.], [300., 300.]),
        add(&mut c, s, ShapeKind::Arrow, [400., 150.], [1100., 250.]),
    ];
    c.select(Tool::Select);
    c.selection.change(ids, Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![pointer([150., 150.], true, egui::Modifiers::NONE)],
        0.1,
        true,
    );
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![moved(t(), [450., 300.]), Event::PointerGone],
        0.2,
        true,
    );
    assert!(
        c.selection_gesture.is_some(),
        "hover loss is not pointer cancellation while held"
    );
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![moved(t(), [1200., 700.])],
        0.3,
        true,
    );
    let delta = c.selection_preview(s).unwrap();
    near(delta.apply([150., 150.]), [1200., 700.]);
    assert_eq!(c.selection.ids, HashSet::from(ids));
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![pointer([1200., 700.], false, egui::Modifiers::NONE)],
        0.4,
        true,
    );
    assert!(c.selection_gesture.is_none());
    for id in ids {
        assert!(c.drawing.object(id).is_some());
    }
}

#[test]
fn fast_group_drag_keeps_frozen_sources_through_offscreen_frames_and_one_history_entry() {
    for zoom in [0.5, 1., 2., 4., 8.] {
        for pan in [false, true] {
            let (mut c, s) = setup(DocumentKind::Whiteboard);
            let ctx = egui::Context::default();
            let mut ids = HashSet::new();
            ids.insert(add(
                &mut c,
                s,
                ShapeKind::Rectangle,
                [-1000., -1000.],
                [1600., 1200.],
            ));
            for (i, kind) in [
                ShapeKind::Ellipse,
                ShapeKind::Diamond,
                ShapeKind::Triangle,
                ShapeKind::Line,
                ShapeKind::Arrow,
            ]
            .into_iter()
            .enumerate()
            {
                let x = 300. + i as f64 * 80.;
                ids.insert(add(&mut c, s, kind, [x, 300.], [x + 60., 360.]));
            }
            // Completely offscreen members belong to the same session.
            ids.insert(add(
                &mut c,
                s,
                ShapeKind::Arrow,
                [5000., 5000.],
                [5100., 5100.],
            ));
            c.select(Tool::Select);
            c.selection
                .change(ids.clone(), Membership::Replace, &c.drawing);
            let view = Transform {
                origin: [200., 200.],
                center: if pan { [210., 205.] } else { [200., 200.] },
                scale: zoom,
            };
            let start = view.to_screen([200., 200.]);
            let original = c.drawing.items().to_vec();
            let revision = c.drawing.revision();
            frame(&ctx, &mut c, s, view, vec![], 0., true);
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(start, true, egui::Modifiers::NONE)],
                0.1,
                true,
            );
            for (i, target) in [
                [250., 220.],
                [450., 300.],
                [1200., 700.],
                [-50000., -50000.],
                [800., 500.],
                [250., 220.],
            ]
            .into_iter()
            .enumerate()
            {
                frame(
                    &ctx,
                    &mut c,
                    s,
                    view,
                    vec![
                        Event::PointerMoved(egui::pos2(151., 121.)),
                        Event::PointerMoved(egui::pos2(target[0] as f32, target[1] as f32)),
                        Event::PointerGone,
                    ],
                    0.2 + i as f64 * 0.2,
                    true,
                );
                let delta = c.selection_preview(s).unwrap();
                near(delta.apply([200., 200.]), view.to_document(target));
                assert_eq!(c.selection.ids, ids);
                assert_eq!(c.drawing.items(), original);
                assert_eq!(c.drawing.revision(), revision);
                let Some(SelectionGesture::Transform { session, .. }) = &c.selection_gesture else {
                    panic!("lost session")
                };
                assert_eq!(session.ids, ids);
                assert_eq!(session.sources, original);
                assert_eq!(session.view, view);
                frame(&ctx, &mut c, s, view, vec![], 0.3 + i as f64 * 0.2, true);
                assert_eq!(c.selection_preview(s), Some(delta));
            }
            // Coalesced hover-loss/release must commit, regardless of final button state.
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    Event::PointerGone,
                    pointer([800., 500.], false, egui::Modifiers::NONE),
                ],
                2.,
                true,
            );
            assert!(c.selection_gesture.is_none());
            assert_eq!(c.selection.ids, ids);
            assert_eq!(c.drawing.revision(), revision + 1);
            let after = c.drawing.items().to_vec();
            let movement: Point =
                std::array::from_fn(|i| view.to_document([800., 500.])[i] - [200., 200.][i]);
            for (before, after) in original.iter().zip(&after) {
                let a = before.as_object();
                let b = after.as_object();
                near(
                    b.placement().position,
                    std::array::from_fn(|i| a.placement().position[i] + movement[i]),
                );
                assert_eq!(a.placement().rotation, b.placement().rotation);
                assert_eq!(a.placement().scale, b.placement().scale);
                assert_eq!(before.id(), after.id());
            }
            c.undo();
            assert_eq!(c.drawing.items(), original);
            c.redo();
            assert_eq!(c.drawing.items(), after);
        }
    }
}

#[test]
fn mostly_offscreen_shape_still_emits_visible_preview_mesh_after_crossing_viewport() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let id = add(
        &mut c,
        s,
        ShapeKind::Rectangle,
        [-1600., -1200.],
        [200., 200.],
    );
    c.select(Tool::Select);
    c.selection.change([id], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![pointer([100., 100.], true, egui::Modifiers::NONE)],
        0.1,
        true,
    );
    let viewport = Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.));
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(viewport),
            events: vec![Event::PointerMoved(egui::pos2(900., 650.))],
            focused: true,
            ..Default::default()
        },
        |ui| {
            c.begin_frame(ui);
            c.surface(ui, viewport, s, t());
            c.end_frame(ui);
        },
    );
    assert!(c.selection_gesture.is_some());
    assert!(
        output
            .shapes
            .iter()
            .any(|clipped| matches!(&clipped.shape,egui::Shape::Mesh(mesh)
        if mesh.calc_bounds().width()>1700. && clipped.clip_rect.intersects(viewport))),
        "the visible portion of the large selected shape must actually be submitted for rendering"
    );
    output.textures_delta.clear();
}

#[test]
fn group_drag_cancels_safely_on_explicit_cancellation_or_view_change() {
    for reason in 0..4 {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        let ids = [
            add(&mut c, s, ShapeKind::Rectangle, [100., 100.], [300., 300.]),
            add(
                &mut c,
                s,
                ShapeKind::Ellipse,
                [1000., 1000.],
                [1200., 1200.],
            ),
        ];
        c.select(Tool::Select);
        c.selection.change(ids, Membership::Replace, &c.drawing);
        let original = c.drawing.items().to_vec();
        let revision = c.drawing.revision();
        frame(&ctx, &mut c, s, t(), vec![], 0., true);
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![
                pointer([150., 150.], true, egui::Modifiers::NONE),
                moved(t(), [1200., 700.]),
            ],
            0.1,
            true,
        );
        assert!(c.selection_preview(s).is_some());
        let events = match reason {
            0 => vec![Event::Touch {
                device_id: egui::TouchDeviceId(0),
                id: egui::TouchId(0),
                phase: egui::TouchPhase::Cancel,
                pos: egui::pos2(1200., 700.),
                force: None,
            }],
            1 => vec![Event::WindowFocused(false)],
            2 => vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            _ => vec![],
        };
        let view = if reason == 3 {
            Transform { scale: 4., ..t() }
        } else {
            t()
        };
        frame(&ctx, &mut c, s, view, events, 0.2, true);
        assert!(c.selection_gesture.is_none());
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![pointer([1200., 700.], false, egui::Modifiers::NONE)],
            0.3,
            true,
        );
        assert_eq!(c.drawing.items(), original);
        assert_eq!(c.drawing.revision(), revision);
        if reason != 2 {
            assert_eq!(c.selection.ids, HashSet::from(ids));
        }
    }
}
