use super::*;
use crate::document::DocumentKind;
use crate::object::CanvasObject;

fn pointer(point: Point, pressed: bool, modifiers: egui::Modifiers) -> Event {
    Event::PointerButton {
        pos: egui::pos2(point[0] as f32, point[1] as f32),
        button: PointerButton::Primary,
        pressed,
        modifiers,
    }
}
fn screen(t: Transform, p: Point) -> Point {
    t.to_screen(p)
}
fn moved(t: Transform, p: Point) -> Event {
    let p = t.to_screen(p);
    Event::PointerMoved(egui::pos2(p[0] as f32, p[1] as f32))
}
fn frame(
    ctx: &egui::Context,
    canvas: &mut Canvas,
    surface: Surface,
    transform: Transform,
    events: Vec<Event>,
    time: f64,
    focused: bool,
) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.))),
            events,
            time: Some(time),
            focused,
            ..Default::default()
        },
        |ui| {
            canvas.begin_frame(ui);
            canvas.surface(
                ui,
                Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.)),
                surface,
                transform,
            );
            canvas.end_frame(ui);
        },
    );
    output.textures_delta.clear();
}
fn click(
    ctx: &egui::Context,
    c: &mut Canvas,
    s: Surface,
    t: Transform,
    p: Point,
    modifiers: egui::Modifiers,
) {
    frame(
        ctx,
        c,
        s,
        t,
        vec![moved(t, p), pointer(screen(t, p), true, modifiers)],
        0.,
        true,
    );
    frame(
        ctx,
        c,
        s,
        t,
        vec![pointer(screen(t, p), false, modifiers)],
        0.1,
        true,
    );
}
fn t() -> Transform {
    Transform {
        origin: [0.; 2],
        center: [0.; 2],
        scale: 1.,
    }
}
fn stroke(c: &mut Canvas, s: Surface, a: Point, b: Point) -> Uuid {
    let mut stroke = Stroke::new(c.drawing.owner(), s, [37, 99, 200, 255], 2.);
    stroke.smoothing = drawing::Smoothing::PolylineV1;
    stroke.push(a);
    stroke.push(b);
    let id = stroke.id;
    assert!(c.drawing.add(stroke));
    id
}
fn setup(kind: DocumentKind) -> (Canvas, Surface) {
    let data = DocumentData::blank(kind);
    let s = if kind == DocumentKind::Page {
        Surface::Page(0)
    } else {
        Surface::Canvas
    };
    (Canvas::new(&data), s)
}
#[test]
fn single_multi_topmost_selection_scope_and_clear() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        let (mut c, s) = setup(kind);
        let ctx = egui::Context::default();
        let t = t();
        let a = stroke(&mut c, s, [30., 100.], [90., 100.]);
        let b = stroke(&mut c, s, [60., 100.], [120., 100.]);
        let other = stroke(&mut c, s, [30., 180.], [120., 180.]);
        c.select(Tool::Select);
        frame(&ctx, &mut c, s, t, vec![], 0., true);
        click(&ctx, &mut c, s, t, [70., 100.], egui::Modifiers::NONE);
        assert_eq!(c.selection.ids, HashSet::from([b]));
        click(&ctx, &mut c, s, t, [40., 100.], egui::Modifiers::SHIFT);
        assert_eq!(c.selection.ids, HashSet::from([a, b]));
        click(&ctx, &mut c, s, t, [40., 100.], egui::Modifiers::ALT);
        assert_eq!(c.selection.ids, HashSet::from([b]));
        c.selection_action(SelectionCommand::SelectAll, &ctx);
        assert_eq!(c.selection.ids, HashSet::from([a, b, other]));
        click(&ctx, &mut c, s, t, [400., 400.], egui::Modifiers::NONE);
        assert!(c.selection.ids.is_empty());
        assert_eq!(c.drawing.strokes().len(), 3);
    }
}
#[test]
fn lasso_closes_selects_and_adds_subtracts_under_zoom() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.25, 1., 4.] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            let t = Transform {
                origin: [5.; 2],
                center: [0.; 2],
                scale,
            };
            let a = stroke(&mut c, s, [30., 60.], [90., 60.]);
            let b = stroke(&mut c, s, [30., 110.], [90., 110.]);
            c.select(Tool::Lasso);
            let lasso = |c: &mut Canvas, polygon: &[Point], mods| {
                c.select(Tool::Lasso);
                let mut events = vec![
                    moved(t, polygon[0]),
                    pointer(screen(t, polygon[0]), true, mods),
                ];
                events.extend(polygon.iter().skip(1).map(|p| moved(t, *p)));
                events.push(pointer(screen(t, *polygon.last().unwrap()), false, mods));
                frame(&ctx, c, s, t, events, 0., true);
            };
            lasso(
                &mut c,
                &[[20., 40.], [100., 40.], [100., 80.], [20., 80.]],
                egui::Modifiers::NONE,
            );
            assert_eq!(c.selection.ids, HashSet::from([a]));
            lasso(
                &mut c,
                &[[20., 90.], [100., 90.], [100., 130.], [20., 130.]],
                egui::Modifiers::SHIFT,
            );
            assert_eq!(c.selection.ids, HashSet::from([a, b]));
            lasso(
                &mut c,
                &[[20., 40.], [100., 40.], [100., 80.], [20., 80.]],
                egui::Modifiers::ALT,
            );
            assert_eq!(c.selection.ids, HashSet::from([b]));
            lasso(&mut c, &[[150., 40.], [151., 41.]], egui::Modifiers::NONE);
            assert_eq!(c.selection.ids, HashSet::from([b]));
            assert_eq!(c.drawing.strokes().len(), 2);
        }
    }
}
#[test]
fn moving_is_one_transaction_cancels_and_reuses_meshes() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.25, 1., 4.] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            let t = Transform {
                origin: [10.; 2],
                center: [0.; 2],
                scale,
            };
            let id = stroke(&mut c, s, [30., 60.], [90., 60.]);
            c.select(Tool::Select);
            click(&ctx, &mut c, s, t, [60., 60.], egui::Modifiers::NONE);
            let original = c.drawing.strokes()[0].clone();
            let revision = c.drawing.revision();
            let mesh = c.cache[&id].meshes[0].clone();
            frame(
                &ctx,
                &mut c,
                s,
                t,
                vec![
                    pointer(screen(t, [60., 60.]), true, egui::Modifiers::NONE),
                    moved(t, [100., 100.]),
                ],
                1.,
                true,
            );
            assert_eq!(
                c.drawing.revision(),
                revision,
                "Preview must not mutate the document"
            );
            assert!(Arc::ptr_eq(&mesh, &c.cache[&id].meshes[0]));
            frame(
                &ctx,
                &mut c,
                s,
                t,
                vec![pointer(
                    screen(t, [100., 100.]),
                    false,
                    egui::Modifiers::NONE,
                )],
                1.1,
                true,
            );
            assert_eq!(c.drawing.revision(), revision + 1);
            let changed = c.drawing.strokes()[0].clone();
            assert_eq!(changed.id, id);
            assert_eq!(changed.points, original.points);
            assert_eq!(
                changed.placement.apply(original.points[0].position),
                [70., 100.]
            );
            c.undo();
            assert_eq!(c.drawing.strokes()[0], original);
            c.redo();
            assert_eq!(c.drawing.strokes()[0], changed);
            frame(
                &ctx,
                &mut c,
                s,
                t,
                vec![
                    pointer(screen(t, [100., 100.]), true, egui::Modifiers::NONE),
                    moved(t, [120., 100.]),
                ],
                2.,
                true,
            );
            frame(
                &ctx,
                &mut c,
                s,
                t,
                vec![Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                2.1,
                true,
            );
            frame(
                &ctx,
                &mut c,
                s,
                t,
                vec![pointer(
                    screen(t, [120., 100.]),
                    false,
                    egui::Modifiers::NONE,
                )],
                2.2,
                true,
            );
            assert_eq!(c.drawing.strokes()[0], changed);
        }
    }
}
#[test]
fn resizing_preserves_raw_style_and_is_proportional_and_page_safe() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        let (mut c, s) = setup(kind);
        let ctx = egui::Context::default();
        let t = t();
        let id = stroke(&mut c, s, [40., 100.], [100., 130.]);
        c.select(Tool::Select);
        click(&ctx, &mut c, s, t, [70., 115.], egui::Modifiers::NONE);
        let original = c.drawing.strokes()[0].clone();
        let bounds = c.selection.bounds.unwrap();
        let corner = bounds.max;
        let anchor = bounds.min;
        let target = std::array::from_fn(|i| anchor[i] + (corner[i] - anchor[i]) * 2.);
        frame(
            &ctx,
            &mut c,
            s,
            t,
            vec![
                pointer(corner, true, egui::Modifiers::NONE),
                moved(t, target),
                pointer(target, false, egui::Modifiers::NONE),
            ],
            0.,
            true,
        );
        let resized = c.drawing.strokes()[0].clone();
        assert_eq!(resized.placement.scale, 2.);
        assert_eq!(resized.points, original.points);
        assert_eq!(resized.id, id);
        assert_eq!(resized.width, original.width);
        assert_eq!(resized.pressure_style, original.pressure_style);
        assert_eq!(resized.color, original.color);
        c.undo();
        assert_eq!(c.drawing.strokes()[0], original);
        c.redo();
        assert_eq!(c.drawing.strokes()[0], resized);
        if kind == DocumentKind::Page {
            let b = c.selection.bounds.unwrap();
            let corner = b.max;
            frame(
                &ctx,
                &mut c,
                s,
                t,
                vec![
                    pointer(corner, true, egui::Modifiers::NONE),
                    moved(t, [10000.; 2]),
                    pointer([10000.; 2], false, egui::Modifiers::NONE),
                ],
                1.,
                true,
            );
            for point in c.drawing.strokes()[0].path() {
                assert!(c.drawing.contains(s, point));
            }
        }
    }
}
#[test]
fn duplicate_delete_copy_paste_are_atomic_across_documents_and_owners() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let t = t();
    stroke(&mut c, s, [-100., 10.], [40., 30.]);
    stroke(&mut c, s, [30., 80.], [40., 90.]);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    let original = c.drawing.strokes().to_vec();
    c.selection_action(SelectionCommand::Copy, &ctx);
    c.selection_action(SelectionCommand::Duplicate, &ctx);
    assert_eq!(c.drawing.strokes().len(), 4);
    let dup = c.drawing.strokes()[2..].to_vec();
    assert_eq!(dup[0].points, original[0].points);
    assert_ne!(dup[0].id, original[0].id);
    c.undo();
    assert_eq!(c.drawing.strokes(), original);
    c.redo();
    assert_eq!(&c.drawing.strokes()[2..], dup);
    c.selection_action(SelectionCommand::Delete, &ctx);
    assert_eq!(c.drawing.strokes(), original);
    c.undo();
    assert_eq!(&c.drawing.strokes()[2..], dup);
    let (mut target, scope) = setup(DocumentKind::Page);
    target.selection_action(SelectionCommand::Paste, &ctx);
    frame(&ctx, &mut target, scope, t, vec![], 10., true);
    assert_eq!(target.drawing.strokes().len(), 2);
    let pasted = target.drawing.strokes().to_vec();
    for stroke in &pasted {
        assert_eq!(stroke.document_id, target.drawing.owner());
        assert_eq!(stroke.surface, scope);
        assert!(!original.iter().any(|o| o.id == stroke.id));
        for point in stroke.path() {
            assert!(target.drawing.contains(scope, point));
        }
    }
    target.undo();
    assert!(target.drawing.strokes().is_empty());
    target.redo();
    assert_eq!(target.drawing.strokes(), pasted);
}
#[test]
fn transformed_ink_remains_erasable_in_both_modes_with_exact_history() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for rotation in [0.0, std::f64::consts::FRAC_PI_2] {
            for mode in [EraserMode::Stroke, EraserMode::Partial] {
                let (mut c, s) = setup(kind);
                let ctx = egui::Context::default();
                let t = t();
                stroke(&mut c, s, [30., 100.], [130., 100.]);
                c.selection_action(SelectionCommand::SelectAll, &ctx);
                let ids = c.selection.ids.clone();
                assert!(c.drawing.transform_objects(
                    &ids,
                    Placement {
                        origin: [30., 100.],
                        position: [60., 120.],
                        scale: 2.,
                        rotation,
                    }
                ));
                c.prune_cache();
                frame(&ctx, &mut c, s, t, vec![], 0., true);
                let source = c.drawing.strokes()[0].clone();
                c.select(Tool::Eraser);
                c.eraser_radius = 4.;
                c.eraser_mode = mode;
                let midpoint = source.placement.apply([80., 100.]);
                click(&ctx, &mut c, s, t, midpoint, egui::Modifiers::NONE);
                if mode == EraserMode::Partial {
                    assert_eq!(c.drawing.strokes().len(), 2);
                    for p in c.drawing.strokes() {
                        assert_eq!(p.placement, source.placement);
                        assert_eq!(p.width, source.width);
                    }
                } else {
                    assert!(c.drawing.strokes().is_empty());
                }
                let erased = c.drawing.strokes().to_vec();
                c.undo();
                assert_eq!(c.drawing.strokes(), &[source]);
                c.redo();
                assert_eq!(c.drawing.strokes(), erased);
            }
        }
    }
}
#[test]
fn far_origin_scope_and_focus_loss_do_not_mutate_selection_or_presets() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let origin = [1e12, -1e12];
    let id = stroke(
        &mut c,
        s,
        [origin[0] + 30., origin[1] + 100.],
        [origin[0] + 100., origin[1] + 100.],
    );
    let t = Transform {
        origin: [0.; 2],
        center: origin,
        scale: 2.,
    };
    c.select(Tool::Select);
    let start = [origin[0] + 60., origin[1] + 100.];
    click(&ctx, &mut c, s, t, start, egui::Modifiers::NONE);
    assert!(c.selection.ids.contains(&id));
    let original = c.drawing.strokes()[0].clone();
    let pen = (c.color, c.width, c.smoothing_strength, c.pressure_style);
    frame(
        &ctx,
        &mut c,
        s,
        t,
        vec![
            pointer(screen(t, start), true, egui::Modifiers::NONE),
            moved(t, [start[0] + 20., start[1] + 30.]),
        ],
        1.,
        true,
    );
    frame(&ctx, &mut c, s, t, vec![], 1.1, false);
    assert_eq!(c.drawing.strokes()[0], original);
    assert_eq!(
        pen,
        (c.color, c.width, c.smoothing_strength, c.pressure_style)
    );
    c.selection.scope(Surface::Page(1));
    assert!(c.selection.ids.is_empty());
    assert_eq!(original.object_id(), id);
}

#[test]
fn dot_selection_moves_and_proportional_handles_remain_reachable() {
    let (mut c, s) = setup(DocumentKind::Page);
    let ctx = egui::Context::default();
    let t = t();
    stroke(&mut c, s, [100., 100.], [100., 100.]);
    c.select(Tool::Select);
    click(&ctx, &mut c, s, t, [100., 100.], egui::Modifiers::NONE);
    frame(
        &ctx,
        &mut c,
        s,
        t,
        vec![
            pointer([100., 100.], true, egui::Modifiers::NONE),
            moved(t, [120., 120.]),
            pointer([120., 120.], false, egui::Modifiers::NONE),
        ],
        1.,
        true,
    );
    assert_eq!(c.drawing.strokes()[0].path(), vec![[120., 120.]]);
    let display = Canvas::display_bounds(c.selection.bounds.unwrap(), t.scale);
    let corner = display.max;
    frame(
        &ctx,
        &mut c,
        s,
        t,
        vec![
            pointer(corner, true, egui::Modifiers::NONE),
            moved(t, [corner[0] + 11., corner[1] + 11.]),
            pointer(
                [corner[0] + 11., corner[1] + 11.],
                false,
                egui::Modifiers::NONE,
            ),
        ],
        2.,
        true,
    );
    assert_eq!(c.drawing.strokes()[0].placement.scale, 1.5);
}
#[test]
fn changing_view_cancels_transform_and_never_commits_a_jump() {
    let (mut c, s) = setup(DocumentKind::Page);
    let ctx = egui::Context::default();
    let t = t();
    stroke(&mut c, s, [30., 100.], [130., 100.]);
    c.select(Tool::Select);
    click(&ctx, &mut c, s, t, [80., 100.], egui::Modifiers::NONE);
    let original = c.drawing.strokes()[0].clone();
    frame(
        &ctx,
        &mut c,
        s,
        t,
        vec![
            pointer([80., 100.], true, egui::Modifiers::NONE),
            moved(t, [120., 120.]),
        ],
        1.,
        true,
    );
    frame(
        &ctx,
        &mut c,
        s,
        Transform {
            origin: [0., -20.],
            ..t
        },
        vec![],
        1.1,
        true,
    );
    frame(
        &ctx,
        &mut c,
        s,
        Transform {
            origin: [0., -20.],
            ..t
        },
        vec![pointer([120., 120.], false, egui::Modifiers::NONE)],
        1.2,
        true,
    );
    assert_eq!(c.drawing.strokes()[0], original);
}
#[test]
fn placement_preserves_all_pen_types_pressure_and_fragment_appearance() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for pen_type in crate::pen::PenType::ALL {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            let mut ink = Stroke::new(c.drawing.owner(), s, [100, 30, 200, 89], 4.);
            ink.pen_type = pen_type;
            ink.smoothing = drawing::Smoothing::AdaptiveV1 { strength: 75 };
            ink.pressure_style = drawing::PressureStyle {
                enabled: pen_type == crate::pen::PenType::Pressure,
                strength: 80,
                minimum: 25,
            };
            ink.push_pressure([30., 100.], Some(0.1));
            ink.push_pressure([80., 100.], Some(0.5));
            ink.push_pressure([130., 100.], Some(1.));
            let original = ink.clone();
            assert!(c.drawing.add(ink));
            c.selection_action(SelectionCommand::SelectAll, &ctx);
            c.selection_action(SelectionCommand::Larger, &ctx);
            let transformed = c.drawing.strokes()[0].clone();
            assert_eq!(transformed.points, original.points);
            assert_eq!(transformed.pressure_style, original.pressure_style);
            assert_eq!(transformed.pen_type, pen_type);
            assert_eq!(transformed.smoothing, original.smoothing);
            assert_eq!(transformed.width, original.width);
            let center = transformed.placement.apply([80., 100.]);
            c.select(Tool::Eraser);
            c.eraser_mode = EraserMode::Partial;
            c.eraser_radius = 4.;
            click(&ctx, &mut c, s, t(), center, egui::Modifiers::NONE);
            assert_eq!(c.drawing.strokes().len(), 2);
            for piece in c.drawing.strokes() {
                assert_eq!(piece.placement, transformed.placement);
                assert_eq!(piece.pressure_style, original.pressure_style);
                assert_eq!(piece.pen_type, pen_type);
                assert_eq!(piece.width, original.width);
                assert_eq!(piece.color, original.color);
                assert!(piece.points.iter().all(|p| p.pressure.is_some()));
            }
            c.undo();
            assert_eq!(c.drawing.strokes()[0], transformed);
            c.undo();
            assert_eq!(*c.drawing.strokes()[0], original);
        }
    }
}
#[test]
fn keyboard_commands_respect_text_focus_and_system_payloads() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    stroke(&mut c, s, [30., 100.], [130., 100.]);
    let key = |key| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    frame(&ctx, &mut c, s, t(), vec![key(egui::Key::A)], 0., true);
    assert_eq!(c.selection.ids.len(), 1);
    frame(&ctx, &mut c, s, t(), vec![key(egui::Key::D)], 0.1, true);
    assert_eq!(c.drawing.strokes().len(), 2);
    let bundle = ObjectBundle::capture(c.drawing.strokes().iter().map(|s| (**s).clone())).unwrap();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![Event::Paste(bundle.encode().unwrap())],
        0.2,
        true,
    );
    assert_eq!(c.drawing.strokes().len(), 4);
    let count = c.drawing.strokes().len();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![Event::Paste("ordinary text".into())],
        0.3,
        true,
    );
    assert_eq!(c.drawing.strokes().len(), count);
    let mut text = "Name".to_string();
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.))),
            events: vec![],
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
            events: vec![Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            focused: true,
            ..Default::default()
        },
        |ui| {
            ui.text_edit_singleline(&mut text);
            c.begin_frame(ui);
        },
    );
    out.textures_delta.clear();
    assert_eq!(
        c.drawing.strokes().len(),
        count,
        "Delete inside text input must not remove objects"
    );
}

#[test]
fn paste_into_a_fresh_pen_scope_uses_visible_view_center() {
    let (mut source, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    stroke(&mut source, s, [30., 100.], [130., 100.]);
    source.selection_action(SelectionCommand::SelectAll, &ctx);
    source.selection_action(SelectionCommand::Copy, &ctx);
    let (mut target, scope) = setup(DocumentKind::Page);
    frame(&ctx, &mut target, scope, t(), vec![], 0., true);
    target.selection_action(SelectionCommand::Paste, &ctx);
    frame(&ctx, &mut target, scope, t(), vec![], 1., true);
    assert_eq!(
        target.drawing.strokes()[0].world_bounds().unwrap().center(),
        [400., 300.]
    );
}

#[test]
fn page_edge_duplicate_never_rescales_original_geometry() {
    let (mut c, s) = setup(DocumentKind::Page);
    let ctx = egui::Context::default();
    let width = c.drawing.bounds(s).unwrap()[0];
    stroke(&mut c, s, [0., 100.], [width, 100.]);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    c.selection_action(SelectionCommand::Duplicate, &ctx);
    assert_eq!(c.drawing.strokes().len(), 2);
    let a = &c.drawing.strokes()[0];
    let b = &c.drawing.strokes()[1];
    assert_eq!(a.placement.scale, b.placement.scale);
    assert_eq!(a.width, b.width);
    assert_eq!(a.points, b.points);
    assert_eq!(b.path()[0], [0., 112.]);
    assert_eq!(b.path().last(), Some(&[width, 112.]));
}

#[test]
fn over_complex_lasso_preserves_previous_selection() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let id = stroke(&mut c, s, [30., 100.], [130., 100.]);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    c.select(Tool::Lasso);
    let mut events = vec![pointer([10., 30.], true, egui::Modifiers::NONE)];
    events.extend(
        (0..4100).map(|i| moved(t(), [10. + (i % 500) as f64, 30. + (i % 2) as f64 * 10.])),
    );
    events.push(pointer([10., 30.], false, egui::Modifiers::NONE));
    frame(&ctx, &mut c, s, t(), events, 1., true);
    assert_eq!(c.selection.ids, HashSet::from([id]));
    assert!(
        c.notice
            .as_ref()
            .is_some_and(|text| text.contains("too complex"))
    );
}

#[test]
fn escape_clears_idle_and_active_lasso_selection_without_editing_ink() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for active_lasso in [false, true] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            let id = stroke(&mut c, s, [30., 60.], [90., 60.]);
            stroke(&mut c, s, [30., 110.], [90., 110.]);
            c.selection_action(SelectionCommand::SelectAll, &ctx);
            frame(&ctx, &mut c, s, t(), vec![], 0., true);
            let original = c.drawing.strokes().to_vec();
            let revision = c.drawing.revision();
            let mesh = c.cache[&id].meshes[0].clone();
            if active_lasso {
                c.select(Tool::Lasso);
                frame(
                    &ctx,
                    &mut c,
                    s,
                    t(),
                    vec![
                        pointer([20., 90.], true, egui::Modifiers::SHIFT),
                        moved(t(), [100., 90.]),
                        moved(t(), [100., 130.]),
                        moved(t(), [20., 130.]),
                    ],
                    1.,
                    true,
                );
                assert!(matches!(
                    c.selection_gesture,
                    Some(SelectionGesture::Lasso { .. })
                ));
            }
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
                2.,
                true,
            );
            assert!(
                c.selection.ids.is_empty(),
                "Escape must clear even during a lasso"
            );
            assert!(c.selection.bounds.is_none());
            assert!(c.selection.geometry.is_none());
            assert!(c.selection.objects.is_empty());
            assert!(c.selection_gesture.is_none());
            frame(
                &ctx,
                &mut c,
                s,
                t(),
                vec![pointer([20., 130.], false, egui::Modifiers::NONE)],
                2.1,
                true,
            );
            assert!(
                c.selection.ids.is_empty(),
                "Release must not commit the cancelled lasso"
            );
            assert_eq!(c.drawing.revision(), revision);
            assert_eq!(c.drawing.strokes(), original);
            assert!(!c.drawing.can_redo());
            assert!(Arc::ptr_eq(&mesh, &c.cache[&id].meshes[0]));
            c.undo();
            assert_eq!(
                c.drawing.strokes(),
                &original[..1],
                "Escape must not add a history entry"
            );
        }
    }
}

#[test]
fn click_selection_uses_geometry_and_screen_tolerance_under_zoom_and_pan() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.25, 1., 4.] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            let t = Transform {
                origin: [50., 40.],
                center: [10., 20.],
                scale,
            };
            let a = stroke(&mut c, s, [40., 60.], [100., 120.]);
            let b = stroke(&mut c, s, [40., 145.], [100., 145.]);
            let original = c.drawing.strokes().to_vec();
            let revision = c.drawing.revision();
            c.select(Tool::Select);
            click(&ctx, &mut c, s, t, [70., 90.], egui::Modifiers::NONE);
            assert_eq!(c.selection.ids, HashSet::from([a]));
            click(&ctx, &mut c, s, t, [70., 145.], egui::Modifiers::SHIFT);
            assert_eq!(c.selection.ids, HashSet::from([a, b]));
            // Inside the diagonal's AABB but outside its visible envelope/tolerance.
            click(&ctx, &mut c, s, t, [95., 85.], egui::Modifiers::NONE);
            assert!(c.selection.ids.is_empty());
            // Three screen points from the centerline is within click tolerance.
            click(
                &ctx,
                &mut c,
                s,
                t,
                [70., 145. + 3. / scale],
                egui::Modifiers::NONE,
            );
            assert_eq!(c.selection.ids, HashSet::from([b]));
            // Four screen points of tolerance plus ink half-width, then two outside.
            click(
                &ctx,
                &mut c,
                s,
                t,
                [70., 145. + (4. + scale + 2.) / scale],
                egui::Modifiers::NONE,
            );
            assert!(c.selection.ids.is_empty());
            assert_eq!(c.drawing.revision(), revision);
            for (before, after) in original.iter().zip(c.drawing.strokes()) {
                assert!(Arc::ptr_eq(before, after));
            }
        }
    }
}

#[test]
fn page_selection_rejects_outside_paper_while_whiteboard_has_no_paper_boundary() {
    let edge = 210. * drawing::POINTS_PER_MM;
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        let (mut c, s) = setup(kind);
        let ctx = egui::Context::default();
        let id = stroke(&mut c, s, [edge - 20., 100.], [edge, 100.]);
        let original = c.drawing.strokes().to_vec();
        let revision = c.drawing.revision();
        c.select(Tool::Select);
        // Outside paper, but close enough to the ink to pass its hit tolerance.
        click(
            &ctx,
            &mut c,
            s,
            t(),
            [edge + 3., 100.],
            egui::Modifiers::NONE,
        );
        assert_eq!(
            c.selection.ids.contains(&id),
            kind == DocumentKind::Whiteboard
        );
        click(
            &ctx,
            &mut c,
            s,
            t(),
            [edge - 10., 100.],
            egui::Modifiers::NONE,
        );
        assert_eq!(c.selection.ids, HashSet::from([id]));
        c.selection.clear();
        c.select(Tool::Lasso);
        frame(
            &ctx,
            &mut c,
            s,
            t(),
            vec![
                pointer([edge + 10., 80.], true, egui::Modifiers::NONE),
                moved(t(), [edge + 10., 120.]),
                moved(t(), [edge - 30., 120.]),
                moved(t(), [edge - 30., 80.]),
                pointer([edge - 30., 80.], false, egui::Modifiers::NONE),
            ],
            1.,
            true,
        );
        assert_eq!(
            c.selection.ids.contains(&id),
            kind == DocumentKind::Whiteboard
        );
        assert_eq!(c.drawing.revision(), revision);
        assert_eq!(c.drawing.strokes(), original);
        assert_eq!(c.drawing.strokes()[0].surface(), s);
    }
}

#[test]
fn selection_tool_handoff_preserves_pen_and_both_erasers() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for mode in [EraserMode::Stroke, EraserMode::Partial] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            let id = stroke(&mut c, s, [30., 100.], [130., 100.]);
            let original = c.drawing.strokes()[0].clone();
            c.selection_action(SelectionCommand::SelectAll, &ctx);
            c.select(Tool::Lasso);
            frame(
                &ctx,
                &mut c,
                s,
                t(),
                vec![
                    pointer([20., 80.], true, egui::Modifiers::NONE),
                    moved(t(), [140., 80.]),
                ],
                1.,
                true,
            );
            c.select(Tool::Pen);
            assert!(c.selection.ids.is_empty());
            frame(
                &ctx,
                &mut c,
                s,
                t(),
                vec![pointer([140., 80.], false, egui::Modifiers::NONE)],
                1.1,
                true,
            );
            assert_eq!(c.drawing.strokes(), std::slice::from_ref(&original));
            click(&ctx, &mut c, s, t(), [200., 200.], egui::Modifiers::NONE);
            assert_eq!(c.drawing.strokes().len(), 2);
            let dot = c.drawing.strokes()[1].clone();
            c.select(Tool::Select);
            click(&ctx, &mut c, s, t(), [200., 200.], egui::Modifiers::NONE);
            assert_eq!(c.selection.ids, HashSet::from([dot.id]));
            c.select(Tool::Eraser);
            c.eraser_mode = mode;
            c.eraser_radius = 4.;
            click(&ctx, &mut c, s, t(), [200., 200.], egui::Modifiers::NONE);
            assert_eq!(c.drawing.strokes(), std::slice::from_ref(&original));
            c.undo();
            assert_eq!(c.drawing.strokes()[1], dot);
            c.select(Tool::Pan);
            click(&ctx, &mut c, s, t(), [200., 200.], egui::Modifiers::NONE);
            assert_eq!(c.drawing.strokes().len(), 2);
            assert_eq!(c.drawing.strokes()[0].id, id);
        }
    }
}
