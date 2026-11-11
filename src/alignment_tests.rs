//! Real canvas gestures and connector-aware organization transactions.
use super::tests::{frame, moved, pointer, screen, setup, t};
use super::*;
use crate::{
    connector::{Connector, Endpoint, Port, Route},
    document::DocumentKind,
    shape::{Shape, ShapeKind},
    snapping::{Align, SnapSettings},
};
fn ids(values: &[Uuid]) -> HashSet<Uuid> {
    values.iter().copied().collect()
}
fn add(c: &mut Canvas, s: Surface, x: f64, y: f64, w: f64, h: f64) -> Uuid {
    let shape = Shape::create(
        c.drawing.owner(),
        s,
        ShapeKind::Rectangle,
        [x, y],
        [x + w, y + h],
        false,
    )
    .unwrap();
    let id = shape.id;
    assert!(c.drawing.add_shape(shape));
    id
}
fn bounds(c: &Canvas, id: Uuid) -> ObjectBounds {
    c.drawing.object(id).unwrap().world_bounds().unwrap()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}
fn link(c: &mut Canvas, s: Surface, a: Uuid, b: Uuid, route: Route) -> Uuid {
    let Some(ObjectSnapshot::Shape(left)) = c.drawing.snapshot(a) else {
        panic!()
    };
    let Some(ObjectSnapshot::Shape(right)) = c.drawing.snapshot(b) else {
        panic!()
    };
    let mut connector = Connector::new(
        c.drawing.owner(),
        s,
        Endpoint::attached(left, Port::Right).unwrap(),
        Endpoint::attached(right, Port::Left).unwrap(),
        Default::default(),
    );
    connector.route = route;
    let id = connector.id;
    assert!(c.drawing.add_connector(connector));
    id
}
fn assert_link(c: &Canvas, id: Uuid) {
    let Some(ObjectSnapshot::Connector(wire)) = c.drawing.snapshot(id) else {
        panic!()
    };
    for end in wire.ends() {
        if let Endpoint::Attached {
            object_id, anchor, ..
        } = end
        {
            let Some(ObjectSnapshot::Shape(shape)) = c.drawing.snapshot(*object_id) else {
                panic!()
            };
            let p = anchor.point(shape).unwrap();
            near(end.position()[0], p[0]);
            near(end.position()[1], p[1]);
        }
    }
}
#[test]
fn six_alignments_are_atomic_on_both_layouts_without_changing_ids_styles_or_order() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for command in [
            Align::Left,
            Align::Center,
            Align::Right,
            Align::Top,
            Align::Middle,
            Align::Bottom,
        ] {
            let (mut c, s) = setup(kind);
            let a = add(&mut c, s, 50., 80., 40., 60.);
            let b = add(&mut c, s, 200., 240., 90., 40.);
            let d = add(&mut c, s, 350., 150., 60., 100.);
            let before = c.drawing.items().to_vec();
            let reference = [a, b, d]
                .iter()
                .map(|id| bounds(&c, *id))
                .reduce(ObjectBounds::union)
                .unwrap();
            let rev = c.drawing.revision();
            assert!(c.drawing.align_objects(&ids(&[a, b, d]), command).unwrap());
            assert_eq!(c.drawing.revision(), rev + 1);
            let axis = usize::from(matches!(
                command,
                Align::Top | Align::Middle | Align::Bottom
            ));
            let feature = |b: ObjectBounds| match command {
                Align::Left | Align::Top => b.min[axis],
                Align::Center | Align::Middle => b.center()[axis],
                _ => b.max[axis],
            };
            for (old, new) in before.iter().zip(c.drawing.items()) {
                assert_eq!(old.id(), new.id());
                let (ObjectSnapshot::Shape(a), ObjectSnapshot::Shape(b)) = (old, new) else {
                    panic!()
                };
                assert_eq!(a.geometry, b.geometry);
                assert_eq!(a.style, b.style);
                assert_eq!(a.text, b.text);
                assert_eq!(a.placement.rotation, b.placement.rotation);
                near(feature(b.world_bounds().unwrap()), feature(reference));
            }
            let after = c.drawing.items().to_vec();
            c.drawing.undo();
            assert_eq!(c.drawing.items(), before);
            c.drawing.redo();
            assert_eq!(c.drawing.items(), after);
        }
    }
}
#[test]
fn distribution_of_varied_sizes_preserves_extremes_and_no_space_is_a_noop() {
    for command in [Align::DistributeHorizontal, Align::DistributeVertical] {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let axis = usize::from(command == Align::DistributeVertical);
        let values: Vec<_> = [(0., 20.), (30., 40.), (100., 10.), (240., 30.)]
            .into_iter()
            .map(|(position, size)| {
                if axis == 0 {
                    add(&mut c, s, position, 100., size, 30.)
                } else {
                    add(&mut c, s, 100., position, 30., size)
                }
            })
            .collect();
        let first = bounds(&c, values[0]);
        let last = bounds(&c, values[3]);
        let original = c.drawing.items().to_vec();
        assert!(
            c.drawing
                .align_objects(&values.iter().copied().collect(), command)
                .unwrap()
        );
        assert_eq!(bounds(&c, values[0]), first);
        assert_eq!(bounds(&c, values[3]), last);
        let boxes: Vec<_> = values.iter().map(|id| bounds(&c, *id)).collect();
        let gap = boxes[1].min[axis] - boxes[0].max[axis];
        for pair in boxes.windows(2) {
            near(pair[1].min[axis] - pair[0].max[axis], gap);
            assert!(gap >= 0.);
        }
        c.drawing.undo();
        assert_eq!(c.drawing.items(), original);
        let (mut tight, s) = setup(DocumentKind::Whiteboard);
        let a = add(&mut tight, s, 0., 0., 80., 80.);
        let b = add(&mut tight, s, 10., 10., 80., 80.);
        let d = add(&mut tight, s, 20., 20., 80., 80.);
        let before = tight.drawing.items().to_vec();
        let rev = tight.drawing.revision();
        assert!(
            tight
                .drawing
                .align_objects(&ids(&[a, b, d]), command)
                .is_err()
        );
        assert_eq!(tight.drawing.items(), before);
        assert_eq!(tight.drawing.revision(), rev);
    }
}
#[test]
fn nested_groups_locks_rich_hosts_and_active_scope_keep_their_contract() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, 50., 80., 60., 60.);
    let b = add(&mut c, s, 150., 100., 40., 40.);
    let outside = add(&mut c, s, 350., 220., 50., 80.);
    let inner = c.drawing.group_selection(&ids(&[a, b])).unwrap();
    let note = Shape::create_sticky_note(
        c.drawing.owner(),
        s,
        [220., 180.],
        [100., 80.],
        [255, 244, 177],
    )
    .unwrap();
    let nid = note.id;
    let text = note.text.clone();
    assert!(c.drawing.add_shape(note));
    let outer = c.drawing.group_selection(&ids(&[inner, nid])).unwrap();
    let relative = [bounds(&c, a).center(), bounds(&c, b).center()];
    let before = c.drawing.items().to_vec();
    assert!(
        c.drawing
            .align_objects(&ids(&[outer, outside]), Align::Top)
            .unwrap()
    );
    let after = c.drawing.items().to_vec();
    near(
        bounds(&c, b).center()[0] - bounds(&c, a).center()[0],
        relative[1][0] - relative[0][0],
    );
    near(
        bounds(&c, b).center()[1] - bounds(&c, a).center()[1],
        relative[1][1] - relative[0][1],
    );
    let Some(ObjectSnapshot::Shape(note)) = c.drawing.snapshot(nid) else {
        panic!()
    };
    assert_eq!(note.text, text);
    assert_eq!(c.drawing.parent(inner), Some(outer));
    c.drawing.undo();
    assert_eq!(c.drawing.items(), before);
    c.drawing.redo();
    assert_eq!(c.drawing.items(), after);
    assert!(c.drawing.enter_group(outer));
    let external = bounds(&c, outside);
    assert!(
        c.drawing
            .align_objects(&ids(&[inner, nid, outside]), Align::Bottom)
            .unwrap()
    );
    assert_eq!(bounds(&c, outside), external);
    assert!(c.drawing.lock_objects(&ids(&[a]), true));
    let old = c.drawing.snapshot(a).cloned();
    assert!(
        c.drawing
            .align_objects(&ids(&[inner, nid]), Align::Left)
            .is_err()
    );
    assert_eq!(c.drawing.snapshot(a), old.as_ref());
    let candidates = c.drawing.snap_targets(s, &ids(&[nid]));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].id, inner);
}
#[test]
fn connectors_all_routes_follow_atomic_alignment_and_preserve_manual_route_data() {
    use crate::connector::Rail;
    for route in [
        Route::Straight,
        Route::Orthogonal {
            rail: Some(Rail {
                axis: 0,
                offset: 27.,
            }),
        },
        Route::Curved {
            controls: Some([[25., -30.], [-40., 20.]]),
        },
    ] {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let a = add(&mut c, s, 50., 100., 50., 50.);
        let b = add(&mut c, s, 220., 200., 70., 60.);
        let d = add(&mut c, s, 400., 400., 50., 50.);
        let wire = link(&mut c, s, a, b, route);
        let external = link(&mut c, s, b, d, route);
        let untouched = add(&mut c, s, 800., 800., 30., 30.);
        let original = c.drawing.snapshot(untouched).cloned();
        let before = c.drawing.items().to_vec();
        assert!(
            c.drawing
                .align_objects(&ids(&[a, b, wire]), Align::Top)
                .unwrap()
        );
        assert_link(&c, wire);
        assert_link(&c, external);
        let Some(ObjectSnapshot::Connector(w)) = c.drawing.snapshot(wire) else {
            panic!()
        };
        assert_eq!(w.route, route);
        assert_eq!(c.drawing.snapshot(untouched), original.as_ref());
        let after = c.drawing.items().to_vec();
        c.drawing.undo();
        assert_eq!(c.drawing.items(), before);
        c.drawing.redo();
        assert_eq!(c.drawing.items(), after);
    }
}
#[test]
fn snapped_drag_uses_original_pointer_and_one_release_entry_across_zoom_and_pan() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.5, 1., 2., 4.] {
            let (mut c, s) = setup(kind);
            c.snapping = SnapSettings::default();
            let ctx = egui::Context::default();
            let a = add(&mut c, s, 100., 100., 40., 40.);
            let target = add(&mut c, s, 300., 250., 40., 40.);
            c.select(Tool::Select);
            c.selection.change([a], Membership::Replace, &c.drawing);
            let view = Transform {
                origin: [150., 130.],
                center: [100., 90.],
                scale,
            };
            frame(&ctx, &mut c, s, view, vec![], 0., true);
            let start = [120., 120.];
            let end = [320. - 3. / scale, 270. - 2. / scale];
            let before = c.drawing.items().to_vec();
            let rev = c.drawing.revision();
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, start),
                    pointer(screen(view, start), true, egui::Modifiers::NONE),
                ],
                0.1,
                true,
            );
            for i in 0..10 {
                frame(
                    &ctx,
                    &mut c,
                    s,
                    view,
                    vec![moved(view, end)],
                    0.2 + i as f64 * 0.01,
                    true,
                );
                let delta = c.selection_preview(s).unwrap();
                let b = bounds(&c, a).placed(delta);
                near(b.min[0], bounds(&c, target).min[0]);
                near(b.min[1], bounds(&c, target).min[1]);
                assert_eq!(c.drawing.items(), before);
                assert!(
                    c.snap_session
                        .as_ref()
                        .unwrap()
                        .guides
                        .iter()
                        .all(Option::is_some)
                );
            }
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
                0.4,
                true,
            );
            assert_eq!(c.drawing.revision(), rev + 1);
            near(bounds(&c, a).min[0], bounds(&c, target).min[0]);
            let after = c.drawing.items().to_vec();
            c.undo();
            assert_eq!(c.drawing.items(), before);
            c.redo();
            assert_eq!(c.drawing.items(), after);
        }
    }
}
#[test]
fn active_alt_override_and_escape_keep_membership_and_original_geometry() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    c.snapping = SnapSettings::default();
    let ctx = egui::Context::default();
    let a = add(&mut c, s, 100., 100., 40., 40.);
    add(&mut c, s, 300., 250., 40., 40.);
    c.select(Tool::Select);
    c.selection.change([a], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let before = c.drawing.items().to_vec();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            moved(t(), [120., 120.]),
            pointer([120., 120.], true, egui::Modifiers::NONE),
            moved(t(), [317., 268.]),
        ],
        0.1,
        true,
    );
    near(c.selection_preview(s).unwrap().apply([100., 100.])[0], 300.);
    c.selection_sample(s, [317., 268.], 1., false, true);
    near(c.selection_preview(s).unwrap().apply([100., 100.])[0], 297.);
    assert_eq!(c.selection.ids, ids(&[a]));
    assert!(
        c.snap_session
            .as_ref()
            .unwrap()
            .guides
            .iter()
            .all(Option::is_none)
    );
    c.selection_sample(s, [317., 268.], 1., false, false);
    near(c.selection_preview(s).unwrap().apply([100., 100.])[0], 300.);
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
        0.2,
        true,
    );
    assert!(c.selection_gesture.is_none());
    assert_eq!(c.drawing.items(), before);
}
#[test]
fn axis_aligned_shape_resize_snaps_actual_edges_with_grab_offset_preserved() {
    for scale in [0.5, 1., 2., 4.] {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        c.snapping = SnapSettings::default();
        let ctx = egui::Context::default();
        let a = add(&mut c, s, 100., 100., 80., 60.);
        let target = add(&mut c, s, 300., 250., 60., 60.);
        c.select(Tool::Select);
        c.selection.change([a], Membership::Replace, &c.drawing);
        let view = Transform {
            scale,
            origin: [200.; 2],
            center: [140., 130.],
        };
        frame(&ctx, &mut c, s, view, vec![], 0., true);
        let start = [180. + 2. / scale, 160. + 1. / scale];
        let end = [296. + 2. / scale, 246. + 1. / scale];
        let before = c.drawing.items().to_vec();
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
            0.1,
            true,
        );
        let Some(SelectionGesture::ShapeEdit { preview, .. }) = &c.selection_gesture else {
            panic!("local resize gesture")
        };
        near(
            preview.world_bounds().unwrap().max[0],
            bounds(&c, target).min[0],
        );
        near(
            preview.world_bounds().unwrap().max[1],
            bounds(&c, target).min[1],
        );
        near(preview.world_bounds().unwrap().min[0], bounds(&c, a).min[0]);
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
            0.2,
            true,
        );
        let after = c.drawing.items().to_vec();
        c.undo();
        assert_eq!(c.drawing.items(), before);
        c.redo();
        assert_eq!(c.drawing.items(), after);
    }
}
#[test]
fn group_grid_drag_keeps_descendants_once_and_survives_offscreen_samples() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    c.snapping = SnapSettings {
        objects: false,
        grid: true,
        spacing: 20.,
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let a = add(&mut c, s, 100., 100., 40., 40.);
    let b = add(&mut c, s, 3000., 200., 40., 40.);
    let g = c.drawing.group_selection(&ids(&[a, b])).unwrap();
    c.select(Tool::Select);
    c.selection.change([g], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let before = c.drawing.items().to_vec();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            moved(t(), [120., 120.]),
            pointer([120., 120.], true, egui::Modifiers::NONE),
            Event::PointerGone,
            moved(t(), [-1001., 1257.]),
        ],
        0.1,
        true,
    );
    assert!(c.selection_gesture.is_some());
    let delta = c.selection_preview(s).unwrap();
    let expected = bounds(&c, g).placed(delta);
    near(expected.min[0].rem_euclid(20.), 0.);
    near(expected.min[1].rem_euclid(20.), 0.);
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![pointer([-1001., 1257.], false, egui::Modifiers::NONE)],
        0.2,
        true,
    );
    near(bounds(&c, b).min[0] - bounds(&c, a).min[0], 2900.);
    c.undo();
    assert_eq!(c.drawing.items(), before);
}
#[test]
fn group_owned_connector_keeps_external_attachment_in_snap_preview_and_commit() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, 50., 100., 50., 50.);
    let external = add(&mut c, s, 400., 300., 60., 60.);
    let wire = link(&mut c, s, a, external, Route::Straight);
    let group = c.drawing.group_selection(&ids(&[a, wire])).unwrap();
    let target = add(&mut c, s, 500., 20., 40., 40.);
    let before = c.drawing.items().to_vec();
    assert!(
        c.drawing
            .align_objects(&ids(&[group, target]), Align::Top)
            .unwrap()
    );
    assert_link(&c, wire);
    c.drawing.undo();
    assert_eq!(c.drawing.items(), before);
    let expanded = c.drawing.expanded(&ids(&[group]));
    let delta = Placement {
        position: [30., 40.],
        ..Default::default()
    };
    let Some(ObjectSnapshot::Connector(connector)) = c.drawing.snapshot(wire) else {
        panic!()
    };
    let preview =
        connectors_ui::preview_connector(connector, &c.drawing, &expanded, Some(delta), None);
    assert_eq!(preview.target.target(), Some(external));
    near(preview.target.position()[0], connector.target.position()[0]);
    assert!(c.drawing.transform_objects(&ids(&[group]), delta));
    assert_link(&c, wire);
}
#[test]
fn rotated_handle_snaps_world_grip_and_preserves_the_opposite_local_corner() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    c.snapping = SnapSettings::default();
    let a = add(&mut c, s, 150., 150., 100., 60.);
    assert!(c.drawing.transform_objects(
        &ids(&[a]),
        Placement {
            origin: [150., 150.],
            position: [150., 150.],
            rotation: 0.4,
            ..Default::default()
        }
    ));
    let Some(ObjectSnapshot::Shape(source)) = c.drawing.snapshot(a).cloned() else {
        panic!()
    };
    let handles = source.box_corners().unwrap();
    let proposed = [handles[2][0] + 50., handles[2][1] + 40.];
    let reference = add(
        &mut c,
        s,
        proposed[0] + 4. + 1.,
        proposed[1] + 100.,
        40.,
        40.,
    );
    c.selection.change([a], Membership::Replace, &c.drawing);
    c.begin_snapping(s);
    c.selection_gesture = Some(SelectionGesture::ShapeEdit {
        surface: s,
        source: source.clone(),
        preview: (*source).clone(),
        index: 2,
        corner: true,
        start: handles[2],
        moved: false,
    });
    c.selection_sample(s, proposed, 1., false, false);
    let Some(SelectionGesture::ShapeEdit { preview, .. }) = &c.selection_gesture else {
        panic!()
    };
    let resized = preview.box_corners().unwrap();
    near(resized[2][0], bounds(&c, reference).min[0]);
    near(resized[0][0], handles[0][0]);
    near(resized[0][1], handles[0][1]);
    assert_eq!(preview.placement.rotation, source.placement.rotation);
}
#[test]
fn standalone_rich_text_resize_keeps_runs_and_snaps_width_after_measurement() {
    use crate::text::{FormatRun, TextObject};
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    c.snapping = SnapSettings::default();
    let ctx = egui::Context::default();
    crate::text_layout::install(&ctx);
    let mut text = TextObject::new(c.drawing.owner(), s, [100., 180.], 120.);
    text.content_mut().text = "small BIG".into();
    let mut large = text.content.style.clone();
    large.size = 28.;
    text.content_mut().runs.push(FormatRun {
        range: 6..9,
        style: large,
    });
    text.height = 80.;
    let id = text.id;
    let content = text.content.clone();
    assert!(c.drawing.replace_texts(vec![text], false));
    let reference = add(&mut c, s, 300., 400., 50., 50.);
    c.select(Tool::Select);
    c.selection.change([id], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let start = [220., 220.];
    let end = [296., 220.];
    let before = c.drawing.items().to_vec();
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![
            moved(t(), start),
            pointer(start, true, egui::Modifiers::NONE),
            moved(t(), end),
        ],
        0.1,
        true,
    );
    assert!(c.text_resize.is_some());
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![pointer(end, false, egui::Modifiers::NONE)],
        0.2,
        true,
    );
    let Some(ObjectSnapshot::Text(text)) = c.drawing.snapshot(id) else {
        panic!()
    };
    assert_eq!(text.content, content);
    near(
        text.world_bounds().unwrap().max[0],
        bounds(&c, reference).min[0],
    );
    near(text.world_bounds().unwrap().min[0], 100.);
    c.undo();
    assert_eq!(c.drawing.items(), before);
}
#[test]
fn locked_objects_are_stationary_references_but_excluded_from_alignment() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, 100., 100., 40., 40.);
    let b = add(&mut c, s, 300., 300., 40., 40.);
    let d = add(&mut c, s, 180., 200., 40., 40.);
    assert!(c.drawing.lock_objects(&ids(&[b]), true));
    let stationary = c.drawing.snapshot(b).cloned();
    assert!(
        c.drawing
            .align_objects(&ids(&[a, b, d]), Align::Left)
            .unwrap()
    );
    assert_eq!(c.drawing.snapshot(b), stationary.as_ref());
    near(bounds(&c, a).min[0], bounds(&c, d).min[0]);
    c.snapping = Default::default();
    c.selection.change([a], Membership::Replace, &c.drawing);
    c.begin_snapping(s);
    let b_bounds = bounds(&c, b);
    let proposed = ObjectBounds {
        min: [b_bounds.min[0] - 2., b_bounds.min[1] - 1.],
        max: [b_bounds.max[0] - 2., b_bounds.max[1] - 1.],
    };
    assert_eq!(
        c.snap_session
            .as_mut()
            .unwrap()
            .solve(proposed, 1., false, None),
        [2., 1.]
    );
    assert_eq!(c.drawing.snapshot(b), stationary.as_ref());
}
#[test]
fn snapped_resize_crossing_an_anchor_keeps_the_original_normalization_policy() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    c.snapping = Default::default();
    let a = add(&mut c, s, 100., 100., 80., 60.);
    add(&mut c, s, 500., 500., 40., 40.);
    let Some(ObjectSnapshot::Shape(source)) = c.drawing.snapshot(a).cloned() else {
        panic!()
    };
    let start = source.box_corners().unwrap()[2];
    let target = [80., 70.];
    let expected = source.with_corner(2, target, false).unwrap();
    c.selection.change([a], Membership::Replace, &c.drawing);
    c.begin_snapping(s);
    c.selection_gesture = Some(SelectionGesture::ShapeEdit {
        surface: s,
        source: source.clone(),
        preview: (*source).clone(),
        index: 2,
        corner: true,
        start,
        moved: false,
    });
    c.selection_sample(s, target, 1., false, false);
    let Some(SelectionGesture::ShapeEdit { preview, .. }) = &c.selection_gesture else {
        panic!()
    };
    assert_eq!(*preview, expected);
}
#[test]
fn grouped_resize_snaps_one_uniform_factor_without_normalizing_child_geometry() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    c.snapping = Default::default();
    let a = add(&mut c, s, 100., 100., 40., 40.);
    let b = add(&mut c, s, 200., 100., 40., 40.);
    let group = c.drawing.group_selection(&ids(&[a, b])).unwrap();
    let reference = add(&mut c, s, 300., 300., 40., 40.);
    c.select(Tool::Select);
    c.selection.change([group], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let original = bounds(&c, group);
    let start = original.max;
    let end = [
        start[0] + (original.max[0] - original.min[0]) * 0.4,
        start[1] + (original.max[1] - original.min[1]) * 0.4,
    ];
    c.start_transform(s, start, Some(2), 1.);
    c.selection_sample(s, end, 1., false, false);
    let delta = c.selection_preview(s).unwrap();
    near(original.placed(delta).max[0], bounds(&c, reference).min[0]);
    let before = c.drawing.items().to_vec();
    c.commit_selection();
    for id in [a, b] {
        let old = before.iter().find(|o| o.id() == id).unwrap();
        let Some(ObjectSnapshot::Shape(after)) = c.drawing.snapshot(id) else {
            panic!()
        };
        let ObjectSnapshot::Shape(old) = old else {
            panic!()
        };
        assert_eq!(old.geometry, after.geometry);
        assert_eq!(old.style, after.style);
        near(after.placement.scale, delta.scale);
    }
    c.undo();
    assert_eq!(c.drawing.items(), before);
}
#[test]
fn page_clamping_and_axis_snaps_remain_independent_for_far_outside_pointer_samples() {
    let (mut c, s) = setup(DocumentKind::Page);
    c.snapping = Default::default();
    let ctx = egui::Context::default();
    let a = add(&mut c, s, 100., 100., 40., 40.);
    let reference = add(&mut c, s, 300., 2., 40., 40.);
    c.select(Tool::Select);
    c.selection.change([a], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    c.start_transform(s, [120., 120.], None, 1.);
    c.selection_sample(s, [317., -900.], 1., false, false);
    let actual = bounds(&c, a).placed(c.selection_preview(s).unwrap());
    let expected = bounds(&c, reference);
    near(actual.min[0], expected.min[0]);
    near(actual.min[1], expected.min[1]);
    let before = c.drawing.items().to_vec();
    c.commit_selection();
    near(bounds(&c, a).min[0], expected.min[0]);
    near(bounds(&c, a).min[1], expected.min[1]);
    c.undo();
    assert_eq!(c.drawing.items(), before);
}
