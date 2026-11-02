use super::tests::{frame, moved, pointer, screen, setup, stroke, t};
use super::*;
use crate::{document::DocumentKind, object::CanvasObject};

fn key(key: egui::Key, modifiers: egui::Modifiers) -> Event {
    Event::Key {
        key,
        modifiers,
        physical_key: None,
        pressed: true,
        repeat: false,
    }
}
fn modifiers_frame(
    ctx: &egui::Context,
    canvas: &mut Canvas,
    surface: Surface,
    transform: Transform,
    mut events: Vec<Event>,
    modifiers: egui::Modifiers,
    time: f64,
) {
    events.insert(0, Event::ModifiersChanged(modifiers));
    frame(ctx, canvas, surface, transform, events, time, true);
}
fn near(a: Point, b: Point) {
    for i in 0..2 {
        assert!((a[i] - b[i]).abs() < 1e-3, "{a:?} != {b:?}");
    }
}
fn pressured(canvas: &mut Canvas, surface: Surface) -> Uuid {
    let mut ink = Stroke::new(canvas.drawing.owner(), surface, [40, 70, 100, 89], 4.);
    ink.pen_type = crate::pen::PenType::Pressure;
    ink.pressure_style.enabled = true;
    ink.smoothing = drawing::Smoothing::AdaptiveV1 { strength: 75 };
    for (p, force) in [([60., 65.], 0.1), ([80., 70.], 1.), ([100., 65.], 0.4)] {
        ink.push_pressure(p, Some(force));
    }
    let id = ink.id;
    assert!(canvas.drawing.add(ink));
    id
}

#[test]
fn rotation_drag_preserves_group_pressure_and_history_with_cached_idle_preview() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.25, 1., 4.] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            let view = Transform {
                origin: [60., 50.],
                center: [50., 50.],
                scale,
            };
            let id = pressured(&mut c, s);
            stroke(&mut c, s, [65., 90.], [90., 90.]);
            c.selection_action(SelectionCommand::SelectAll, &ctx);
            frame(&ctx, &mut c, s, view, vec![], 0., true);
            let original = c.drawing.strokes().to_vec();
            let revision = c.drawing.revision();
            let base = c.cache[&id].meshes[0].clone();
            let center = c.selection.bounds.unwrap().center();
            let handle = c.rotation_point(scale).unwrap();
            let radius = center[1] - handle[1];
            let target = [center[0] + radius, center[1]];
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    pointer(screen(view, handle), true, egui::Modifiers::NONE),
                    moved(view, target),
                ],
                1.,
                true,
            );
            let delta = c.selection_preview(s).expect("rotation preview");
            assert!((delta.rotation - std::f64::consts::FRAC_PI_2).abs() < 1e-4);
            assert_eq!(c.drawing.revision(), revision);
            assert_eq!(c.drawing.strokes(), original);
            assert!(Arc::ptr_eq(&base, &c.cache[&id].meshes[0]));
            let rotated = c.cache[&id].rotation_preview.as_ref().unwrap().meshes[0].clone();
            for (a, b) in base.vertices.iter().zip(&rotated.vertices) {
                let expected = view.to_screen(
                    delta.apply(view.to_document([f64::from(a.pos.x), f64::from(a.pos.y)])),
                );
                near([f64::from(b.pos.x), f64::from(b.pos.y)], expected);
                assert_eq!(a.color, b.color);
                assert_eq!(a.uv, b.uv);
            }
            frame(&ctx, &mut c, s, view, vec![], 1.1, true);
            assert!(Arc::ptr_eq(
                &rotated,
                &c.cache[&id].rotation_preview.as_ref().unwrap().meshes[0]
            ));
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, target), false, egui::Modifiers::NONE)],
                1.2,
                true,
            );
            assert_eq!(c.drawing.revision(), revision + 1);
            let committed = c.drawing.strokes().to_vec();
            for (raw, now) in original.iter().zip(&committed) {
                assert_eq!(now.id, raw.id);
                assert_eq!(now.surface, raw.surface);
                assert_eq!(now.points, raw.points);
                assert_eq!(now.smoothing, raw.smoothing);
                assert_eq!(now.pressure_style, raw.pressure_style);
                assert_eq!(now.pen_type, raw.pen_type);
                assert_eq!(now.width, raw.width);
                assert_eq!(now.color, raw.color);
                near(
                    now.placement.apply(raw.points[0].position),
                    delta.apply(raw.placement.apply(raw.points[0].position)),
                );
                assert!(c.selection.ids.contains(&raw.id));
            }
            c.undo();
            assert_eq!(c.drawing.strokes(), original);
            c.redo();
            assert_eq!(c.drawing.strokes(), committed);
        }
    }
}

#[test]
fn rotation_shift_snaps_and_escape_cancels_without_a_command() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        let (mut c, s) = setup(kind);
        let ctx = egui::Context::default();
        pressured(&mut c, s);
        c.selection_action(SelectionCommand::SelectAll, &ctx);
        frame(&ctx, &mut c, s, t(), vec![], 0., true);
        let original = c.drawing.strokes().to_vec();
        let revision = c.drawing.revision();
        let center = c.selection.bounds.unwrap().center();
        let handle = c.rotation_point(1.).unwrap();
        let radius = center[1] - handle[1];
        let angle = 21_f64.to_radians();
        let target = [
            center[0] + radius * angle.sin(),
            center[1] - radius * angle.cos(),
        ];
        modifiers_frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![
                pointer(handle, true, egui::Modifiers::SHIFT),
                moved(t(), target),
            ],
            egui::Modifiers::SHIFT,
            1.,
        );
        assert!((c.selection_preview(s).unwrap().rotation - 15_f64.to_radians()).abs() < 1e-10);
        modifiers_frame(&ctx, &mut c, s, t(), vec![], egui::Modifiers::NONE, 1.02);
        assert!((c.selection_preview(s).unwrap().rotation - angle).abs() < 1e-5);
        modifiers_frame(&ctx, &mut c, s, t(), vec![], egui::Modifiers::SHIFT, 1.04);
        assert!((c.selection_preview(s).unwrap().rotation - 15_f64.to_radians()).abs() < 1e-10);

        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
            1.1,
            true,
        );
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![pointer(target, false, egui::Modifiers::NONE)],
            1.2,
            true,
        );
        assert_eq!(c.drawing.strokes(), original);
        assert_eq!(c.drawing.revision(), revision);
        assert!(c.selection.ids.is_empty());
        assert!(c.cache.values().all(|v| v.rotation_preview.is_none()));
        c.selection_action(SelectionCommand::SelectAll, &ctx);
        modifiers_frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![
                pointer(handle, true, egui::Modifiers::SHIFT),
                moved(t(), target),
                pointer(target, false, egui::Modifiers::SHIFT),
            ],
            egui::Modifiers::SHIFT,
            2.,
        );
        assert!((c.drawing.strokes()[0].placement.rotation - 15_f64.to_radians()).abs() < 1e-10);
    }
}

#[test]
fn page_rotation_limits_and_keyboard_nudges_keep_ownership() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        let (mut c, s) = setup(kind);
        let ctx = egui::Context::default();
        stroke(&mut c, s, [1., 1.], [50., 1.]);
        c.selection_action(SelectionCommand::SelectAll, &ctx);
        let original = c.drawing.strokes()[0].clone();
        let revision = c.drawing.revision();
        c.selection_action(SelectionCommand::RotateRight, &ctx);
        if kind == DocumentKind::Page {
            assert_eq!(c.drawing.revision(), revision);
            assert_eq!(c.drawing.strokes()[0], original);
        } else {
            assert_eq!(c.drawing.revision(), revision + 1);
            c.undo();
        }
        modifiers_frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![key(egui::Key::ArrowDown, egui::Modifiers::SHIFT)],
            egui::Modifiers::SHIFT,
            1.,
        );
        near(c.drawing.strokes()[0].placement.apply([1., 1.]), [1., 11.]);
        modifiers_frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)],
            egui::Modifiers::NONE,
            2.,
        );
        near(c.drawing.strokes()[0].placement.apply([1., 1.]), [2., 11.]);
        c.selection_action(SelectionCommand::RotateRight, &ctx);
        assert_eq!(c.drawing.strokes()[0].surface, s);
        assert_eq!(c.drawing.strokes()[0].id, original.id);
        let changed = c.drawing.strokes()[0].clone();
        c.undo();
        c.redo();
        assert_eq!(c.drawing.strokes()[0], changed);
    }
}

#[test]
fn off_axis_handwriting_resize_is_uniform_and_has_a_minimum_extent() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for shift in [false, true] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            stroke(&mut c, s, [50., 50.], [100., 80.]);
            c.selection_action(SelectionCommand::SelectAll, &ctx);
            frame(&ctx, &mut c, s, t(), vec![], 0., true);
            let original = c.drawing.strokes()[0].clone();
            let b = c.selection.bounds.unwrap();
            let corner = b.max;
            let target = [corner[0] + 30., corner[1]];
            let mods = if shift {
                egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::NONE
            };
            modifiers_frame(
                &ctx,
                &mut c,
                s,
                t(),
                vec![
                    pointer(corner, true, mods),
                    moved(t(), target),
                    pointer(target, false, mods),
                ],
                mods,
                1.,
            );
            let changed = &c.drawing.strokes()[0];
            let a = changed.placement.apply([50., 50.]);
            let z = changed.placement.apply([100., 80.]);
            assert!((z[0] - a[0]) / 50. > 1.);
            assert!(((z[0] - a[0]) / 50. - (z[1] - a[1]) / 30.).abs() < 1e-10);
            assert_eq!(changed.points, original.points);
            assert_eq!(changed.width, original.width);
            let corner = c.selection.bounds.unwrap().max;
            let anchor = c.selection.bounds.unwrap().min;
            modifiers_frame(
                &ctx,
                &mut c,
                s,
                t(),
                vec![
                    pointer(corner, true, mods),
                    moved(t(), anchor),
                    pointer(anchor, false, mods),
                ],
                mods,
                2.,
            );
            let b = c.drawing.strokes()[0].world_bounds().unwrap();
            assert!((b.max[0] - b.min[0]).max(b.max[1] - b.min[1]) >= 1. - 1e-8);
            assert!(c.drawing.strokes()[0].placement.valid());
        }
    }
}

#[test]
fn cut_paste_cross_document_fallback_preserves_style_and_exact_undo() {
    for source_kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for target_kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let (mut source, s) = setup(source_kind);
            let ctx = egui::Context::default();
            pressured(&mut source, s);
            stroke(&mut source, s, [65., 90.], [90., 90.]);
            source.selection_action(SelectionCommand::SelectAll, &ctx);
            source.selection_action(SelectionCommand::RotateRight, &ctx);
            let original = source.drawing.strokes().to_vec();
            let revision = source.drawing.revision();
            frame(
                &ctx,
                &mut source,
                s,
                t(),
                vec![Event::Cut, key(egui::Key::X, egui::Modifiers::COMMAND)],
                1.,
                true,
            );
            assert!(source.drawing.strokes().is_empty());
            assert!(source.selection.ids.is_empty());
            assert_eq!(
                source.drawing.revision(),
                revision + 1,
                "native Cut and shortcut must not run twice"
            );
            source.undo();
            assert_eq!(source.drawing.strokes(), original);
            source.redo();
            assert!(source.drawing.strokes().is_empty());
            let (mut target, scope) = setup(target_kind);
            frame(&ctx, &mut target, scope, t(), vec![], 2., true);
            target.selection_action(SelectionCommand::Paste, &ctx);
            frame(&ctx, &mut target, scope, t(), vec![], 3., true);
            let pasted = target.drawing.strokes().to_vec();
            assert_eq!(pasted.len(), original.len());
            for (raw, now) in original.iter().zip(&pasted) {
                assert_ne!(now.id, raw.id);
                assert_eq!(now.document_id, target.drawing.owner());
                assert_eq!(now.surface, scope);
                assert_eq!(now.points, raw.points);
                assert_eq!(now.color, raw.color);
                assert_eq!(now.width, raw.width);
                assert_eq!(now.pressure_style, raw.pressure_style);
                assert_eq!(now.smoothing, raw.smoothing);
                assert_eq!(now.pen_type, raw.pen_type);
                assert_eq!(now.placement.scale, raw.placement.scale);
                assert_eq!(now.placement.rotation, raw.placement.rotation);
                assert!(target.selection.ids.contains(&now.id));
            }
            let before_delta = [
                original[1].path()[0][0] - original[0].path()[0][0],
                original[1].path()[0][1] - original[0].path()[0][1],
            ];
            let after_delta = [
                pasted[1].path()[0][0] - pasted[0].path()[0][0],
                pasted[1].path()[0][1] - pasted[0].path()[0][1],
            ];
            near(before_delta, after_delta);
            target.undo();
            assert!(target.drawing.strokes().is_empty());
            target.redo();
            assert_eq!(target.drawing.strokes(), pasted);
            target.selection_action(SelectionCommand::Delete, &ctx);
            assert!(target.drawing.strokes().is_empty());
            target.undo();
            assert_eq!(target.drawing.strokes(), pasted);
        }
    }
}

#[test]
fn failed_cut_preserves_objects_history_and_previous_clipboard() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    stroke(&mut c, s, [10., 10.], [20., 10.]);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    c.selection_action(SelectionCommand::Copy, &ctx);
    let clipboard = ctx
        .data(|d| d.get_temp::<SessionClipboard>(clipboard_id()))
        .unwrap()
        .bundle;
    for i in 0..crate::object_clipboard::MAX_OBJECTS {
        stroke(&mut c, s, [i as f64, 20.], [i as f64, 20.]);
    }
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    let revision = c.drawing.revision();
    let original = c.drawing.strokes().to_vec();
    c.selection_action(SelectionCommand::Cut, &ctx);
    assert_eq!(c.drawing.revision(), revision);
    assert_eq!(c.drawing.strokes(), original);
    assert_eq!(c.selection.ids.len(), original.len());
    let retained = ctx
        .data(|d| d.get_temp::<SessionClipboard>(clipboard_id()))
        .unwrap()
        .bundle;
    assert!(Arc::ptr_eq(&clipboard, &retained));
    assert!(c.notice.as_ref().unwrap().contains("limit"));
}

#[test]
fn cut_and_nudges_respect_text_focus_modifiers_and_active_gestures() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    pressured(&mut c, s);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    let original = c.drawing.strokes().to_vec();
    let revision = c.drawing.revision();
    modifiers_frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![key(egui::Key::ArrowRight, egui::Modifiers::COMMAND)],
        egui::Modifiers::COMMAND,
        0.,
    );
    assert_eq!(c.drawing.revision(), revision);
    // A text widget consumes Cut and arrow input before the canvas shortcuts run.
    let mut text = "Preset name".to_string();
    let mut out = ctx.run_ui(
        egui::RawInput {
            focused: true,
            ..Default::default()
        },
        |ui| {
            ui.text_edit_singleline(&mut text).request_focus();
            c.begin_frame(ui);
        },
    );
    out.textures_delta.clear();
    let mut out = ctx.run_ui(
        egui::RawInput {
            focused: true,
            events: vec![
                Event::Cut,
                key(egui::Key::X, egui::Modifiers::COMMAND),
                key(egui::Key::ArrowRight, egui::Modifiers::NONE),
            ],
            ..Default::default()
        },
        |ui| {
            ui.text_edit_singleline(&mut text);
            c.begin_frame(ui);
        },
    );
    out.textures_delta.clear();
    assert_eq!(c.drawing.strokes(), original);
    assert_eq!(c.drawing.revision(), revision);
    ctx.memory_mut(|m| {
        if let Some(focus) = m.focused() {
            m.surrender_focus(focus);
        }
    });
    modifiers_frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![Event::ModifiersChanged(egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        1.,
    );
    let handle = c.rotation_point(1.).unwrap();
    let center = c.selection.bounds.unwrap().center();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            pointer(handle, true, egui::Modifiers::NONE),
            moved(t(), [center[0] + 40., center[1]]),
        ],
        2.,
        true,
    );
    assert!(matches!(
        c.selection_gesture,
        Some(SelectionGesture::Rotate { .. })
    ));
    modifiers_frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        2.1,
    );
    assert_eq!(c.drawing.strokes(), original);
    assert_eq!(c.drawing.revision(), revision);
}

#[test]
fn rotation_unwraps_full_turn_far_from_origin_and_view_change_cancels() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let far = 1e12;
    stroke(&mut c, s, [far - 20., far - 10.], [far + 20., far - 10.]);
    stroke(&mut c, s, [far - 10., far + 10.], [far + 10., far + 10.]);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    let view = Transform {
        origin: [400., 300.],
        center: [far, far],
        scale: 4.,
    };
    frame(&ctx, &mut c, s, view, vec![], 0., true);
    let original = c.drawing.strokes().to_vec();
    let center = c.selection.bounds.unwrap().center();
    let start = c.rotation_point(view.scale).unwrap();
    let radius = center[1] - start[1];
    let mut events = vec![pointer(screen(view, start), true, egui::Modifiers::NONE)];
    for i in 1..=24 {
        let a = i as f64 * std::f64::consts::TAU / 24.;
        events.push(moved(
            view,
            [center[0] + radius * a.sin(), center[1] - radius * a.cos()],
        ));
    }
    frame(&ctx, &mut c, s, view, events, 1., true);
    assert!((c.selection_preview(s).unwrap().rotation - std::f64::consts::TAU).abs() < 1e-6);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![pointer(screen(view, start), false, egui::Modifiers::NONE)],
        1.1,
        true,
    );
    for (raw, now) in original.iter().zip(c.drawing.strokes()) {
        near(raw.path()[0], now.path()[0]);
        assert_eq!(raw.points, now.points);
    }
    c.undo();
    assert_eq!(c.drawing.strokes(), original);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            pointer(screen(view, start), true, egui::Modifiers::NONE),
            moved(view, [center[0] + radius, center[1]]),
        ],
        2.,
        true,
    );
    assert!(c.selection_preview(s).is_some());
    let changed_view = Transform {
        center: [far + 10., far],
        ..view
    };
    frame(&ctx, &mut c, s, changed_view, vec![], 2.1, true);
    frame(
        &ctx,
        &mut c,
        s,
        changed_view,
        vec![pointer(
            screen(changed_view, start),
            false,
            egui::Modifiers::NONE,
        )],
        2.2,
        true,
    );
    assert!(c.selection_gesture.is_none());
    assert_eq!(c.drawing.strokes(), original);
}

#[test]
#[ignore = "manual release-mode multi-stroke rotation preview benchmark"]
fn rotation_preview_performance() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    for i in 0..2000 {
        let mut ink = Stroke::new(c.drawing.owner(), s, [40, 70, 100, 255], 2.);
        ink.smoothing = drawing::Smoothing::PolylineV1;
        let x = 10. + (i % 50) as f64 * 12.;
        let y = 10. + (i / 50) as f64 * 8.;
        for j in 0..20 {
            ink.push([x + j as f64 * 0.2, y + (j as f64 * 0.2).sin()]);
        }
        assert!(c.drawing.add(ink));
    }
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    let view = Transform {
        origin: [400., 300.],
        center: [300., 170.],
        scale: 0.8,
    };
    frame(&ctx, &mut c, s, view, vec![], 0., true);
    let start = c.rotation_point(view.scale).unwrap();
    let center = c.selection.bounds.unwrap().center();
    let radius = center[1] - start[1];
    let now = std::time::Instant::now();
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            pointer(screen(view, start), true, egui::Modifiers::NONE),
            moved(
                view,
                [center[0] + radius * 0.3, center[1] - radius * 0.9539392014],
            ),
        ],
        1.,
        true,
    );
    eprintln!(
        "2,000 strokes / 40,000 samples rotation preview frame: {:?}",
        now.elapsed()
    );
    assert!(c.selection_preview(s).is_some());
    let id = c.drawing.strokes()[0].id;
    let mesh = c.cache[&id].rotation_preview.as_ref().unwrap().meshes[0].clone();
    let now = std::time::Instant::now();
    frame(&ctx, &mut c, s, view, vec![], 1.1, true);
    eprintln!(
        "2,000-stroke unchanged rotation preview frame: {:?}",
        now.elapsed()
    );
    assert!(Arc::ptr_eq(
        &mesh,
        &c.cache[&id].rotation_preview.as_ref().unwrap().meshes[0]
    ));
    let now = std::time::Instant::now();
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![moved(
            view,
            [center[0] + radius * 0.4, center[1] - radius * 0.916515139],
        )],
        1.2,
        true,
    );
    eprintln!(
        "2,000-stroke changed rotation preview frame: {:?}",
        now.elapsed()
    );
}

#[test]
fn rotation_handle_hit_target_is_clear_of_context_controls() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    stroke(&mut c, s, [150., 200.], [250., 200.]);
    stroke(&mut c, s, [150., 300.], [250., 300.]);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let handle = c.rotation_point(1.).unwrap();
    let hit = [handle[0], handle[1] - 4.5];
    let center = c.selection.bounds.unwrap().center();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            pointer(hit, true, egui::Modifiers::NONE),
            moved(t(), [center[0] + 80., center[1]]),
        ],
        1.,
        true,
    );
    assert!(
        matches!(c.selection_gesture, Some(SelectionGesture::Rotate { .. })),
        "the complete rotation hit target must be outside the contextual toolbar"
    );
}
