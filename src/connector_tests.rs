//! Real ordered pointer/controller regression tests for magnetic connections.
use super::tests::{frame, moved, pointer, screen, setup, t};
use super::*;
use crate::{
    connector::{Connector, Endpoint, Port},
    document::DocumentKind,
    shape::{Shape, ShapeKind},
};
fn add_shape(c: &mut Canvas, s: Surface, kind: ShapeKind, a: Point, b: Point) -> Shape {
    let shape = Shape::create(c.drawing.owner(), s, kind, a, b, false).unwrap();
    assert!(c.drawing.add_shape(shape.clone()));
    shape
}
fn port_handle(shape: &Shape, port: Port, scale: f64) -> Point {
    let p = port.point(shape).unwrap();
    let center = shape.geometry_bounds().center();
    let d = [p[0] - center[0], p[1] - center[1]];
    let n = d[0].hypot(d[1]);
    std::array::from_fn(|i| p[i] + d[i] / n * 9. / scale)
}
fn edge(c: &Canvas) -> &Connector {
    c.drawing
        .items()
        .iter()
        .find_map(|o| {
            if let ObjectSnapshot::Connector(c) = o {
                Some(c.as_ref())
            } else {
                None
            }
        })
        .unwrap()
}
#[test]
fn handle_drag_snaps_detaches_reconnects_and_cancels_at_every_zoom_on_both_layouts() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.5, 1., 2., 4.] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            let view = Transform {
                scale,
                origin: [20.; 2],
                center: [0.; 2],
            };
            let a = add_shape(&mut c, s, ShapeKind::Rectangle, [25., 30.], [65., 70.]);
            let b = add_shape(&mut c, s, ShapeKind::Ellipse, [110., 30.], [150., 70.]);
            c.select(Tool::Select);
            c.selection.change([a.id], Membership::Replace, &c.drawing);
            let start = port_handle(&a, Port::Right, scale);
            let target = Port::Left.point(&b).unwrap();
            let before = c.drawing.items().to_vec();
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, start),
                    pointer(screen(view, start), true, egui::Modifiers::NONE),
                    moved(view, target),
                ],
                0.,
                true,
            );
            assert_eq!(c.drawing.items(), before);
            assert!(c.connection_gesture.is_some());
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, target), false, egui::Modifiers::NONE)],
                0.1,
                true,
            );
            assert_eq!(edge(&c).source.target(), Some(a.id));
            assert_eq!(edge(&c).target.target(), Some(b.id));
            let after = c.drawing.items().to_vec();
            c.undo();
            assert_eq!(c.drawing.items(), before);
            c.redo();
            assert_eq!(c.drawing.items(), after);
            let id = edge(&c).id;
            let free = [175., 100.];
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, target),
                    pointer(screen(view, target), true, egui::Modifiers::NONE),
                    moved(view, free),
                ],
                0.2,
                true,
            );
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, free), false, egui::Modifiers::NONE)],
                0.3,
                true,
            );
            assert!(edge(&c).target.target().is_none());
            assert_eq!(edge(&c).id, id);
            c.undo();
            assert_eq!(c.drawing.items(), after);
            c.selection.change([a.id], Membership::Replace, &c.drawing);
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, start),
                    pointer(screen(view, start), true, egui::Modifiers::NONE),
                    moved(view, target),
                ],
                0.4,
                true,
            );
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                    pointer(screen(view, target), false, egui::Modifiers::NONE),
                ],
                0.5,
                true,
            );
            assert_eq!(c.drawing.items(), after);
            assert!(c.connection_gesture.is_none());
        }
    }
}
#[test]
fn linked_routes_follow_frozen_move_preview_and_shape_edits_without_committing_or_cloning_unrelated_meshes()
 {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let view = t();
    let a = add_shape(&mut c, s, ShapeKind::Rectangle, [50., 100.], [150., 180.]);
    let b = add_shape(&mut c, s, ShapeKind::Rectangle, [300., 100.], [400., 180.]);
    let link = Connector::new(
        c.drawing.owner(),
        s,
        Endpoint::attached(&a, Port::Right).unwrap(),
        Endpoint::attached(&b, Port::Left).unwrap(),
        Default::default(),
    );
    let id = link.id;
    assert!(c.drawing.add_connector(link));
    c.select(Tool::Select);
    c.selection.change([a.id], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, view, vec![], 0., true);
    let before = c.drawing.items().to_vec();
    let start = [90., 140.];
    let end = [190., 240.];
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            moved(view, start),
            pointer(start, true, egui::Modifiers::NONE),
            moved(view, end),
        ],
        0.1,
        true,
    );
    assert_eq!(c.drawing.items(), before);
    let preview = crate::canvas::connectors_ui::preview_shape(
        edge(&c),
        &c.drawing,
        &c.selection.ids,
        c.selection_preview(s),
        c.shape_edit_preview(),
    );
    assert_eq!(preview.endpoints().unwrap()[0], [250., 240.]);
    assert_eq!(preview.endpoints().unwrap()[1], [300., 140.]);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![pointer(end, false, egui::Modifiers::NONE)],
        0.2,
        true,
    );
    let Some(ObjectSnapshot::Connector(link)) = c.drawing.snapshot(id) else {
        panic!()
    };
    assert_eq!(link.source.position(), [250., 240.]);
    c.undo();
    assert_eq!(c.drawing.items(), before);
    assert_eq!(
        c.selection.ids,
        HashSet::from([a.id]),
        "Undo must not select incidental connector updates"
    );
}

#[test]
fn connector_selection_priority_region_modes_keyboard_creation_and_noop_click() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let a = add_shape(&mut c, s, ShapeKind::Rectangle, [50., 100.], [150., 180.]);
    let b = add_shape(&mut c, s, ShapeKind::Rectangle, [300., 100.], [400., 180.]);
    c.select(Tool::Select);
    c.selection
        .change([a.id, b.id], Membership::Replace, &c.drawing);
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![Event::Key {
            key: egui::Key::K,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        }],
        0.,
        true,
    );
    let id = edge(&c).id;
    assert_eq!(c.pick(s, [220., 140.], 4.), Some(id));
    assert_eq!(c.pick(s, [150., 140.], 4.), Some(a.id));
    let bounds = ObjectBounds::from_points([[190., 120.], [250., 160.]]).unwrap();
    assert!(edge(&c).as_shape().rectangle_matches(bounds, true));
    assert!(!edge(&c).as_shape().rectangle_matches(bounds, false));
    let region =
        selection::LassoRegion::new(vec![[140., 120.], [310., 120.], [310., 160.], [140., 160.]])
            .unwrap();
    assert!(edge(&c).as_shape().lasso_matches(&region));
    c.selection.change([a.id], Membership::Replace, &c.drawing);
    super::tests::click(&ctx, &mut c, s, t(), [220., 140.], egui::Modifiers::SHIFT);
    assert!(c.selection.ids.contains(&id) && c.selection.ids.contains(&a.id));
    super::tests::click(&ctx, &mut c, s, t(), [220., 140.], egui::Modifiers::ALT);
    assert!(!c.selection.ids.contains(&id));
    let before = c.drawing.items().to_vec();
    let start = port_handle(&a, Port::Top, 1.);
    super::tests::click(&ctx, &mut c, s, t(), start, egui::Modifiers::NONE);
    assert_eq!(c.drawing.items(), before);
}
#[test]
fn free_endpoint_gesture_survives_hover_loss_and_offscreen_samples_but_cancels_on_focus_change() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let view = t();
    let a = add_shape(&mut c, s, ShapeKind::Rectangle, [50., 100.], [150., 180.]);
    c.select(Tool::Select);
    c.selection.change([a.id], Membership::Replace, &c.drawing);
    let start = port_handle(&a, Port::Right, 1.);
    let free = [2000., -1000.];
    let before = c.drawing.items().to_vec();
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            moved(view, start),
            pointer(start, true, egui::Modifiers::NONE),
            Event::PointerGone,
            moved(view, free),
        ],
        0.,
        true,
    );
    assert!(c.connection_gesture.is_some());
    assert_eq!(c.drawing.items(), before);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![pointer(free, false, egui::Modifiers::NONE)],
        0.1,
        true,
    );
    assert_eq!(edge(&c).target, Endpoint::Free { position: free });
    c.undo();
    assert_eq!(c.drawing.items(), before);
    c.selection.change([a.id], Membership::Replace, &c.drawing);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            moved(view, start),
            pointer(start, true, egui::Modifiers::NONE),
            moved(view, free),
        ],
        0.2,
        true,
    );
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![Event::WindowFocused(false)],
        0.3,
        false,
    );
    assert!(c.connection_gesture.is_none());
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![pointer(free, false, egui::Modifiers::NONE)],
        0.4,
        true,
    );
    assert_eq!(c.drawing.items(), before);
}

#[test]
fn page_margin_ports_remain_clickable_and_zero_length_formatting_keeps_heads() {
    let (mut c, s) = setup(DocumentKind::Page);
    let ctx = egui::Context::default();
    let view = Transform {
        scale: 1.,
        origin: [20., 20.],
        center: [0.; 2],
    };
    let a = add_shape(&mut c, s, ShapeKind::Rectangle, [0., 100.], [80., 160.]);
    c.select(Tool::Select);
    c.selection.change([a.id], Membership::Replace, &c.drawing);
    let start = port_handle(&a, Port::Left, 1.);
    let target = [180., 200.];
    let mut render = |events| {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.))),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| {
                c.begin_frame(ui);
                c.surface(
                    ui,
                    Rect::from_min_size(egui::pos2(20., 20.), egui::vec2(595., 842.)),
                    s,
                    view,
                );
                c.end_frame(ui);
            },
        );
        out.textures_delta.clear();
    };
    render(vec![
        moved(view, start),
        pointer(screen(view, start), true, egui::Modifiers::NONE),
        moved(view, target),
    ]);
    render(vec![pointer(
        screen(view, target),
        false,
        egui::Modifiers::NONE,
    )]);
    drop(render);
    assert_eq!(edge(&c).source.target(), Some(a.id));
    let mut zero = Connector::new(
        c.drawing.owner(),
        s,
        Endpoint::Free {
            position: [50., 50.],
        },
        Endpoint::Free {
            position: [50., 50.],
        },
        Default::default(),
    );
    zero.style.start_head = true;
    let head = zero.end_head;
    let id = zero.id;
    assert!(c.drawing.add_connector(zero));
    assert!(c.drawing.format_shapes(
        &HashSet::from([id]),
        crate::shape::StylePatch::Border([100, 20, 30]),
        false
    ));
    let Some(ObjectSnapshot::Connector(zero)) = c.drawing.snapshot(id) else {
        panic!()
    };
    assert_eq!(zero.end_head, head);
    assert!(zero.style.start_head);
}

#[test]
fn advanced_route_handle_drags_preview_commit_cancel_and_undo_once_across_views() {
    use crate::connector::Route;
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.5, 1., 2., 4.] {
            for route in [
                Route::Orthogonal { rail: None },
                Route::Curved { controls: None },
            ] {
                let (mut c, s) = setup(kind);
                let ctx = egui::Context::default();
                let view = Transform {
                    scale,
                    origin: [25., 30.],
                    center: [10., 15.],
                };
                let a = add_shape(&mut c, s, ShapeKind::Rectangle, [25., 60.], [65., 90.]);
                let b = add_shape(&mut c, s, ShapeKind::Rectangle, [140., 140.], [180., 170.]);
                let mut link = Connector::new(
                    c.drawing.owner(),
                    s,
                    Endpoint::attached(&a, Port::Right).unwrap(),
                    Endpoint::attached(&b, Port::Left).unwrap(),
                    Default::default(),
                );
                link.route = route;
                let id = link.id;
                assert!(c.drawing.add_connector(link));
                c.select(Tool::Select);
                c.selection.change([id], Membership::Replace, &c.drawing);
                let handle = edge(&c).route_handles()[0];
                let target = [handle[0] + 10., handle[1] + 20.];
                let before = c.drawing.items().to_vec();
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
                    0.,
                    true,
                );
                assert!(c.connection_gesture.is_some());
                assert_eq!(c.drawing.items(), before);
                frame(
                    &ctx,
                    &mut c,
                    s,
                    view,
                    vec![pointer(screen(view, target), false, egui::Modifiers::NONE)],
                    0.1,
                    true,
                );
                assert_ne!(edge(&c).route, route);
                assert_eq!(edge(&c).source.target(), Some(a.id));
                assert_eq!(edge(&c).target.target(), Some(b.id));
                let after = c.drawing.items().to_vec();
                c.undo();
                assert_eq!(c.drawing.items(), before);
                c.redo();
                assert_eq!(c.drawing.items(), after);
                let handle = edge(&c).route_handles()[0];
                frame(
                    &ctx,
                    &mut c,
                    s,
                    view,
                    vec![
                        moved(view, handle),
                        pointer(screen(view, handle), true, egui::Modifiers::NONE),
                        moved(view, [handle[0] + 20., handle[1] + 10.]),
                    ],
                    0.2,
                    true,
                );
                frame(
                    &ctx,
                    &mut c,
                    s,
                    view,
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
                assert!(c.connection_gesture.is_none());
                assert_eq!(c.drawing.items(), after);
            }
        }
    }
}

#[test]
fn unchanged_curves_reuse_meshes_when_unrelated_shapes_move_and_pick_only_visible_paths() {
    use crate::connector::Route;
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let view = t();
    let a = add_shape(&mut c, s, ShapeKind::Rectangle, [25., 60.], [65., 90.]);
    let b = add_shape(&mut c, s, ShapeKind::Rectangle, [300., 250.], [340., 280.]);
    let other = add_shape(&mut c, s, ShapeKind::Rectangle, [500., 100.], [550., 140.]);
    let mut link = Connector::new(
        c.drawing.owner(),
        s,
        Endpoint::attached(&a, Port::Right).unwrap(),
        Endpoint::attached(&b, Port::Left).unwrap(),
        Default::default(),
    );
    link.route = Route::Curved { controls: None };
    let id = link.id;
    assert!(c.drawing.add_connector(link));
    c.select(Tool::Select);
    frame(&ctx, &mut c, s, view, vec![], 0., true);
    let meshes = c.shape_cache[&id].meshes.clone();
    let points = edge(&c).path(0.01);
    let p = points[points.len() / 2];
    assert_eq!(c.pick(s, p, 2.), Some(id));
    assert_eq!(c.pick(s, [100., 260.], 1.), None);
    assert!(c.drawing.transform_objects(
        &HashSet::from([other.id]),
        Placement {
            position: [10., 15.],
            ..Default::default()
        }
    ));
    frame(&ctx, &mut c, s, view, vec![], 0.1, true);
    assert_eq!(meshes.len(), c.shape_cache[&id].meshes.len());
    for (old, new) in meshes.iter().zip(&c.shape_cache[&id].meshes) {
        assert!(Arc::ptr_eq(old, new));
    }
}

#[test]
fn formatting_toolbar_keeps_bezier_controls_clickable_outside_curve_bounds() {
    use crate::connector::Route;
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let view = t();
    let a = add_shape(&mut c, s, ShapeKind::Rectangle, [100., 300.], [200., 380.]);
    let b = add_shape(&mut c, s, ShapeKind::Rectangle, [400., 350.], [500., 430.]);
    let mut link = Connector::new(
        c.drawing.owner(),
        s,
        Endpoint::attached(&a, Port::Right).unwrap(),
        Endpoint::attached(&b, Port::Left).unwrap(),
        Default::default(),
    );
    link.route = Route::Curved {
        controls: Some([[50., -90.], [-60., 0.]]),
    };
    let id = link.id;
    let handle = link.controls()[0];
    assert!(c.drawing.add_connector(link));
    c.select(Tool::Select);
    c.selection.change([id], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, view, vec![], 0., true);
    frame(&ctx, &mut c, s, view, vec![], 0.1, true);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            moved(view, handle),
            pointer(screen(view, handle), true, egui::Modifiers::NONE),
            moved(view, [handle[0] + 10., handle[1] - 10.]),
        ],
        0.2,
        true,
    );
    assert!(
        c.connection_gesture.is_some(),
        "Formatting area covered the control handle"
    );
}
