//! Organization regressions through the shared model and real canvas controller.
use super::tests::{frame, moved, pointer, screen, setup, t};
use super::*;
use crate::{
    connector::{Connector, Endpoint, Port},
    document::DocumentKind,
    shape::{Shape, ShapeKind},
    text::{FormatRun, TextObject},
};
fn add(c: &mut Canvas, s: Surface, x: f64) -> Uuid {
    let shape = Shape::create(
        c.drawing.owner(),
        s,
        ShapeKind::Rectangle,
        [x, 100.],
        [x + 40., 140.],
        false,
    )
    .unwrap();
    let id = shape.id;
    assert!(c.drawing.add_shape(shape));
    id
}
fn members(c: &Canvas, id: Uuid) -> Vec<Uuid> {
    let Some(ObjectSnapshot::Group(g)) = c.drawing.snapshot(id) else {
        panic!("missing group");
    };
    g.children.clone()
}
fn ids(values: &[Uuid]) -> HashSet<Uuid> {
    values.iter().copied().collect()
}
fn group(c: &mut Canvas, values: &[Uuid]) -> Uuid {
    c.drawing.group_selection(&ids(values)).unwrap()
}
fn near(a: Point, b: Point) {
    for i in 0..2 {
        assert!((a[i] - b[i]).abs() < 1e-7, "{a:?} != {b:?}");
    }
}
#[test]
fn hierarchy_identity_edit_scope_ungroup_and_exact_history() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        let (mut c, s) = setup(kind);
        let a = add(&mut c, s, 40.);
        let b = add(&mut c, s, 100.);
        let d = add(&mut c, s, 180.);
        let before = c.drawing.items().to_vec();
        let inner = group(&mut c, &[a, b]);
        assert_eq!(members(&c, inner), vec![a, b]);
        let outer = group(&mut c, &[inner, d]);
        assert_eq!(c.pick(s, [60., 120.], 0.), Some(outer));
        assert!(c.drawing.enter_group(outer));
        assert_eq!(c.pick(s, [60., 120.], 0.), Some(inner));
        assert!(c.drawing.enter_group(inner));
        assert_eq!(c.pick(s, [60., 120.], 0.), Some(a));
        assert!(c.drawing.group_selection(&ids(&[a, d])).is_none());
        let new = add(&mut c, s, 240.);
        assert_eq!(c.drawing.parent(new), Some(inner));
        c.drawing.undo();
        assert_eq!(members(&c, inner), vec![a, b]);
        assert_eq!(c.drawing.exit_group(), Some(inner));
        assert_eq!(c.drawing.exit_group(), Some(outer));
        let grouped = c.drawing.items().to_vec();
        assert_eq!(c.drawing.ungroup_selection(&ids(&[outer])), vec![inner, d]);
        assert!(c.drawing.snapshot(outer).is_none());
        assert_eq!(members(&c, inner), vec![a, b]);
        c.drawing.undo();
        assert_eq!(c.drawing.items(), grouped);
        c.drawing.redo();
        assert!(c.drawing.snapshot(outer).is_none());
        c.drawing.undo();
        c.drawing.undo();
        c.drawing.undo();
        assert_eq!(c.drawing.items(), before);
        assert!(c.drawing.group_selection(&ids(&[a])).is_none());
    }
}
#[test]
fn nested_transforms_keep_raw_ink_rich_text_sticky_content_and_connectors() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        let (mut c, s) = setup(kind);
        let a = add(&mut c, s, 80.);
        let b = add(&mut c, s, 180.);
        let external = add(&mut c, s, 400.);
        let mut text = TextObject::new(c.drawing.owner(), s, [80., 180.], 160.);
        text.content_mut().text = "Small BIG".into();
        let mut large = text.content.style.clone();
        large.size = 28.;
        text.content_mut().runs.push(FormatRun {
            range: 6..9,
            style: large,
        });
        let tid = text.id;
        let content = text.content.clone();
        assert!(c.drawing.replace_texts(vec![text], false));
        let mut note = Shape::create_sticky_note(
            c.drawing.owner(),
            s,
            [250., 200.],
            [100., 100.],
            [255, 240, 100],
        )
        .unwrap();
        note.text.as_mut().unwrap().content = content.clone();
        let nid = note.id;
        let note_content = note.text.clone();
        assert!(c.drawing.add_shape(note));
        let mut ink = crate::drawing::Stroke::new(c.drawing.owner(), s, [0, 0, 0, 255], 2.);
        ink.push([120., 230.]);
        ink.push([150., 250.]);
        let iid = ink.id;
        let points = ink.points.clone();
        assert!(c.drawing.add(ink));
        let link = |c: &Canvas, from, to| {
            Connector::new(
                c.drawing.owner(),
                s,
                attached(c, from, Port::Right),
                attached(c, to, Port::Left),
                Default::default(),
            )
        };
        let internal = link(&c, a, b);
        let internal_id = internal.id;
        assert!(c.drawing.add_connector(internal));
        let external_link = link(&c, b, external);
        let external_id = external_link.id;
        assert!(c.drawing.add_connector(external_link));
        let peer = add(&mut c, s, 450.);
        let external_group = group(&mut c, &[external, peer]);
        assert_eq!(c.drawing.parent(external), Some(external_group));
        let inner = group(&mut c, &[a, b, internal_id]);
        let outer = group(&mut c, &[inner, tid, nid, iid]);
        for delta in [
            Placement {
                position: [12., 15.],
                ..Default::default()
            },
            Placement {
                origin: [200., 200.],
                position: [200., 200.],
                scale: 0.9,
                ..Default::default()
            },
            Placement {
                origin: [200., 200.],
                position: [200., 200.],
                rotation: 0.15,
                ..Default::default()
            },
            Placement {
                origin: [200., 200.],
                position: [200., 200.],
                reflected: [true, false],
                ..Default::default()
            },
            Placement {
                origin: [200., 200.],
                position: [200., 200.],
                reflected: [false, true],
                ..Default::default()
            },
        ] {
            let before = c.drawing.items().to_vec();
            assert!(c.drawing.transform_objects(&ids(&[outer]), delta));
            let Some(ObjectSnapshot::Text(text)) = c.drawing.snapshot(tid) else {
                panic!()
            };
            assert_eq!(text.content, content);
            let Some(ObjectSnapshot::Shape(note)) = c.drawing.snapshot(nid) else {
                panic!()
            };
            assert_eq!(note.text, note_content);
            let Some(ObjectSnapshot::Stroke(ink)) = c.drawing.snapshot(iid) else {
                panic!()
            };
            assert_eq!(ink.points, points);
            for (link_id, left, right) in [(internal_id, a, b), (external_id, b, external)] {
                let Some(ObjectSnapshot::Connector(link)) = c.drawing.snapshot(link_id) else {
                    panic!()
                };
                for (end, target, port) in [
                    (&link.source, left, Port::Right),
                    (&link.target, right, Port::Left),
                ] {
                    assert_eq!(end.target(), Some(target));
                    let Some(ObjectSnapshot::Shape(shape)) = c.drawing.snapshot(target) else {
                        panic!()
                    };
                    near(end.position(), port.point(shape).unwrap());
                }
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
fn clipboard_nested_groups_internal_remap_external_detach_locks_and_deletion() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, 40.);
    let b = add(&mut c, s, 120.);
    let outside = add(&mut c, s, 250.);
    let wire = Connector::new(
        c.drawing.owner(),
        s,
        attached(&c, a, Port::Right),
        attached(&c, b, Port::Left),
        Default::default(),
    );
    let wid = wire.id;
    assert!(c.drawing.add_connector(wire));
    let external = Connector::new(
        c.drawing.owner(),
        s,
        attached(&c, b, Port::Right),
        attached(&c, outside, Port::Left),
        Default::default(),
    );
    let eid = external.id;
    assert!(c.drawing.add_connector(external));
    let inner = group(&mut c, &[a, b, wid]);
    let outer = group(&mut c, &[inner, eid]);
    assert!(c.drawing.lock_objects(&ids(&[a]), true));
    let expanded = c.drawing.expanded(&ids(&[outer]));
    let bundle = ObjectBundle::capture_objects(
        c.drawing
            .items()
            .iter()
            .filter(|o| expanded.contains(&o.id()))
            .cloned(),
    )
    .unwrap();
    let bundle = ObjectBundle::decode(&bundle.encode().unwrap()).unwrap();
    let pasted = bundle.duplicate(&mut c.drawing, s);
    assert_eq!(pasted.len(), 1);
    let copied = c.drawing.expanded(&ids(&pasted));
    assert!(copied.is_disjoint(&expanded));
    assert_eq!(copied.len(), expanded.len());
    for object in c
        .drawing
        .items()
        .iter()
        .filter(|o| copied.contains(&o.id()))
    {
        if let ObjectSnapshot::Connector(link) = object {
            for end in link.ends() {
                assert!(end.target().is_none_or(|id| copied.contains(&id)));
            }
        }
    }
    assert_eq!(
        copied
            .iter()
            .filter(|id| c.drawing.snapshot(**id).unwrap().locked())
            .count(),
        1
    );
    // Child and nested-container locks survive parent unlock.
    assert!(c.drawing.lock_objects(&ids(&[inner]), true));
    assert!(c.drawing.lock_objects(&ids(&[outer]), true));
    assert!(c.drawing.is_locked(b));
    assert!(c.drawing.lock_objects(&ids(&[outer]), false));
    assert!(c.drawing.is_locked(a));
    assert!(c.drawing.is_locked(b));
    assert!(c.drawing.lock_objects(&ids(&[inner]), false));
    assert!(c.drawing.is_locked(a));
    assert!(!c.drawing.is_locked(b));
    assert!(!c.drawing.transform_objects(
        &ids(&[outer]),
        Placement {
            position: [1.; 2],
            ..Default::default()
        }
    ));
    assert!(c.drawing.lock_objects(&ids(&[a]), false));
    let before = c.drawing.items().to_vec();
    c.drawing.erase(&ids(&[outer]));
    assert!(expanded.iter().all(|id| c.drawing.snapshot(*id).is_none()));
    c.drawing.undo();
    assert_eq!(c.drawing.items(), before);
    let pasted = bundle.paste(&mut c.drawing, s, [-1000., -2000.]);
    assert_eq!(pasted.len(), 1);
}
#[test]
fn locking_rejects_normal_edits_and_reliably_targets_parent_for_unlock() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, 40.);
    let b = add(&mut c, s, 120.);
    let g = group(&mut c, &[a, b]);
    assert!(c.drawing.lock_objects(&ids(&[g]), true));
    let before = c.drawing.items().to_vec();
    let revision = c.drawing.revision();
    assert_eq!(c.pick(s, [60., 120.], 0.), None);
    assert_eq!(c.pick_locked(s, [60., 120.], 0.), Some(g));
    assert!(!c.drawing.enter_group(g));
    c.drawing.erase(&ids(&[g]));
    c.drawing.erase(&ids(&[a]));
    assert!(!c.drawing.transform_objects(
        &ids(&[a]),
        Placement {
            position: [10.; 2],
            ..Default::default()
        }
    ));
    assert!(
        !c.drawing
            .format_shapes(&ids(&[a]), crate::shape::StylePatch::Width(6.), false)
    );
    assert_eq!(c.drawing.items(), before);
    assert_eq!(c.drawing.revision(), revision);
    c.drawing.undo();
    assert!(!c.drawing.is_locked(a));
    c.drawing.redo();
    assert!(c.drawing.is_locked(a));
    assert!(c.drawing.lock_objects(&ids(&[g]), false));
    assert_eq!(c.pick(s, [60., 120.], 0.), Some(g));
}
#[test]
fn arrange_all_four_commands_multi_selection_and_nested_context_hit_priority() {
    use SelectionCommand::*;
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, 50.);
    let b = add(&mut c, s, 50.);
    let d = add(&mut c, s, 50.);
    let e = add(&mut c, s, 50.);
    let roots = |c: &Canvas| {
        c.drawing
            .items()
            .iter()
            .filter(|o| c.drawing.parent(o.id()).is_none())
            .map(ObjectSnapshot::id)
            .collect::<Vec<_>>()
    };
    assert!(c.drawing.arrange(&ids(&[a, d]), BringToFront));
    assert_eq!(roots(&c), vec![b, e, a, d]);
    assert_eq!(c.pick(s, [70., 120.], 0.), Some(d));
    c.drawing.undo();
    assert!(c.drawing.arrange(&ids(&[b, d]), SendToBack));
    assert_eq!(roots(&c), vec![b, d, a, e]);
    c.drawing.undo();
    assert!(c.drawing.arrange(&ids(&[a, b]), BringForward));
    assert_eq!(roots(&c), vec![d, a, b, e]);
    c.drawing.undo();
    assert!(c.drawing.arrange(&ids(&[d, e]), SendBackward));
    assert_eq!(roots(&c), vec![a, d, e, b]);
    c.drawing.undo();
    let g = group(&mut c, &[a, b]);
    assert!(c.drawing.arrange(&ids(&[g]), BringToFront));
    assert_eq!(c.pick(s, [70., 120.], 0.), Some(g));
    assert!(c.drawing.enter_group(g));
    assert_eq!(c.pick(s, [70., 120.], 0.), Some(b));
    assert!(c.drawing.arrange(&ids(&[a]), BringToFront));
    assert_eq!(members(&c, g), vec![b, a]);
    assert_eq!(c.pick(s, [70., 120.], 0.), Some(a));
    let after = c.drawing.items().to_vec();
    c.drawing.undo();
    assert_eq!(members(&c, g), vec![a, b]);
    c.drawing.redo();
    assert_eq!(c.drawing.items(), after);
}
#[test]
fn invalid_group_clipboards_reject_cycles_missing_duplicate_and_cross_surface_ownership() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, 40.);
    let b = add(&mut c, s, 100.);
    let g = group(&mut c, &[a, b]);
    let objects = c.drawing.items().to_vec();
    for children in [vec![g], vec![Uuid::new_v4()], vec![a, a]] {
        let mut bad = objects.clone();
        let ObjectSnapshot::Group(group) = bad.iter_mut().find(|o| o.id() == g).unwrap() else {
            panic!()
        };
        Arc::make_mut(group).children = children;
        assert!(ObjectBundle::capture_objects(bad.into_iter()).is_err());
    }
    let mut bad = objects;
    let ObjectSnapshot::Group(group) = bad.iter_mut().find(|o| o.id() == g).unwrap() else {
        panic!()
    };
    Arc::make_mut(group).surface = Surface::Page(0);
    assert!(ObjectBundle::capture_objects(bad.into_iter()).is_err());
}
#[test]
fn real_group_drag_freezes_descendants_offscreen_at_every_zoom_with_one_history_entry() {
    for scale in [0.5, 1., 2., 4.] {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        let a = add(&mut c, s, 50.);
        let b = add(&mut c, s, 3000.);
        let g = group(&mut c, &[a, b]);
        c.select(Tool::Select);
        c.selection.change([g], Membership::Replace, &c.drawing);
        let view = Transform { scale, ..t() };
        let start = [70., 120.];
        let end = [-1200., 1700.];
        let before = c.drawing.items().to_vec();
        frame(&ctx, &mut c, s, view, vec![], 0., true);
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![
                moved(view, start),
                pointer(screen(view, start), true, egui::Modifiers::NONE),
                Event::PointerGone,
                moved(view, end),
            ],
            0.1,
            true,
        );
        let Some(SelectionGesture::Transform { session, .. }) = &c.selection_gesture else {
            panic!("missing frozen group drag")
        };
        assert_eq!(session.ids, ids(&[g, a, b]));
        assert_eq!(c.drawing.items(), before);
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![pointer(screen(view, end), false, egui::Modifiers::NONE)],
            0.2,
            true,
        );
        let delta = [end[0] - start[0], end[1] - start[1]];
        for id in [a, b] {
            let original = before
                .iter()
                .find(|o| o.id() == id)
                .unwrap()
                .as_object()
                .world_bounds()
                .unwrap()
                .center();
            near(
                c.drawing
                    .object(id)
                    .unwrap()
                    .world_bounds()
                    .unwrap()
                    .center(),
                [original[0] + delta[0], original[1] + delta[1]],
            );
        }
        let after = c.drawing.items().to_vec();
        c.undo();
        assert_eq!(c.drawing.items(), before);
        c.redo();
        assert_eq!(c.drawing.items(), after);
    }
}
#[test]
fn group_area_selection_is_coherent_and_children_become_independent_in_edit_scope() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let a = add(&mut c, s, 50.);
    let b = add(&mut c, s, 200.);
    let g = group(&mut c, &[a, b]);
    assert!(c.drawing.area_targets(&[a], false).is_empty());
    assert_eq!(c.drawing.area_targets(&[a], true), vec![g]);
    assert_eq!(c.drawing.area_targets(&[a, b], false), vec![g]);
    c.drawing.enter_group(g);
    assert_eq!(c.drawing.area_targets(&[a], false), vec![a]);
    c.drawing.lock_objects(&ids(&[a]), true);
    assert!(c.drawing.area_targets(&[a], true).is_empty());
}

fn attached(c: &Canvas, id: Uuid, port: Port) -> Endpoint {
    let Some(ObjectSnapshot::Shape(shape)) = c.drawing.snapshot(id) else {
        panic!()
    };
    Endpoint::attached(shape, port).unwrap()
}
#[test]
fn keyboard_group_ungroup_and_nested_regroup_use_existing_journal() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let a = add(&mut c, s, 40.);
    let b = add(&mut c, s, 120.);
    let d = add(&mut c, s, 200.);
    c.select(Tool::Select);
    c.selection
        .change([a, b, d], Membership::Replace, &c.drawing);
    let key = |shift| Event::Key {
        key: egui::Key::G,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: if shift {
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::COMMAND
        },
    };
    frame(&ctx, &mut c, s, t(), vec![key(false)], 0., true);
    let outer = *c.selection.ids.iter().next().unwrap();
    assert_eq!(members(&c, outer), vec![a, b, d]);
    c.drawing.enter_group(outer);
    c.selection.change([b, d], Membership::Replace, &c.drawing);
    frame(&ctx, &mut c, s, t(), vec![key(false)], 0.1, true);
    let inner = *c.selection.ids.iter().next().unwrap();
    assert_eq!(members(&c, outer), vec![a, inner]);
    assert_eq!(members(&c, inner), vec![b, d]);
    frame(&ctx, &mut c, s, t(), vec![key(true)], 0.2, true);
    assert_eq!(c.selection.ids, ids(&[b, d]));
    assert_eq!(members(&c, outer), vec![a, b, d]);
    c.undo();
    assert_eq!(members(&c, outer), vec![a, inner]);
    assert_eq!(c.selection.ids, ids(&[inner]));
    c.redo();
    assert_eq!(c.selection.ids, ids(&[b, d]));
}
#[test]
fn double_click_enters_one_group_level_then_edits_rich_text_and_escape_exits() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    crate::text_layout::install(&ctx);
    let a = add(&mut c, s, 40.);
    let b = add(&mut c, s, 120.);
    let g = group(&mut c, &[a, b]);
    c.select(Tool::Select);
    let click_at = |c: &mut Canvas, time: f64| {
        frame(
            &ctx,
            c,
            s,
            t(),
            vec![
                moved(t(), [60., 120.]),
                pointer([60., 120.], true, egui::Modifiers::NONE),
            ],
            time,
            true,
        );
        frame(
            &ctx,
            c,
            s,
            t(),
            vec![pointer([60., 120.], false, egui::Modifiers::NONE)],
            time + 0.01,
            true,
        );
    };
    click_at(&mut c, 0.);
    click_at(&mut c, 0.1);
    assert_eq!(c.drawing.editing_group, Some(g));
    assert!(!c.text_editing());
    click_at(&mut c, 1.);
    click_at(&mut c, 1.1);
    assert!(c.text_editing());
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![Event::Text("Grouped text".into())],
        1.2,
        true,
    );
    let esc = || Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&ctx, &mut c, s, t(), vec![esc()], 1.3, true);
    assert!(!c.text_editing());
    assert_eq!(c.drawing.editing_group, Some(g));
    let Some(ObjectSnapshot::Shape(shape)) = c.drawing.snapshot(a) else {
        panic!()
    };
    assert_eq!(shape.text.as_ref().unwrap().content.text, "Grouped text");
    frame(&ctx, &mut c, s, t(), vec![esc()], 1.4, true);
    assert_eq!(c.drawing.editing_group, None);
    assert_eq!(c.selection.ids, ids(&[g]));
}
#[test]
fn partial_erase_inside_group_preserves_fragment_sibling_order_and_history() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let mut ink = crate::drawing::Stroke::new(c.drawing.owner(), s, [0, 0, 0, 255], 2.);
    ink.smoothing = crate::drawing::Smoothing::PolylineV1;
    ink.push([50., 200.]);
    ink.push([200., 200.]);
    let iid = ink.id;
    assert!(c.drawing.add(ink.clone()));
    let a = add(&mut c, s, 50.);
    let g = group(&mut c, &[iid, a]);
    assert!(c.drawing.enter_group(g));
    let mut left = ink.clone();
    left.id = Uuid::new_v4();
    left.points[1].position = [100., 200.];
    let mut right = ink.clone();
    right.id = Uuid::new_v4();
    right.points[0].position = [150., 200.];
    let expected = vec![left.id, right.id, a];
    let before = c.drawing.items().to_vec();
    assert!(
        c.drawing
            .replace_partial(HashMap::from([(iid, vec![left, right])]))
    );
    assert_eq!(members(&c, g), expected);
    let after = c.drawing.items().to_vec();
    c.drawing.undo();
    assert_eq!(c.drawing.items(), before);
    c.drawing.redo();
    assert_eq!(c.drawing.items(), after);
    assert!(
        c.drawing
            .group_selection(&ids(&[Uuid::new_v4(), Uuid::new_v4()]))
            .is_none()
    );
}

#[test]
#[ignore = "manual release-mode nested group movement preview benchmark"]
fn nested_group_drag_preview_performance() {
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
    let half: HashSet<_> = c
        .drawing
        .items()
        .iter()
        .take(500)
        .map(ObjectSnapshot::id)
        .collect();
    c.drawing.group_selection(&half).unwrap();
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    c.selection_action(SelectionCommand::Group, &ctx);
    assert_eq!(c.selection.ids.len(), 1);
    frame(&ctx, &mut c, s, t(), vec![], 0., true);
    let meshes: Vec<_> = c
        .shape_cache
        .iter()
        .map(|(id, c)| (*id, c.meshes[0].clone()))
        .collect();
    assert_eq!(meshes.len(), 1000);
    // Dense neighboring magnetic ports overlap these tiny bodies. Capture the
    // intended move explicitly so this benchmark cannot measure a wire gesture.
    c.start_transform(s, [24., 24.], None, 1.);
    frame(
        &ctx,
        &mut c,
        s,
        t(),
        vec![pointer([24., 24.], true, egui::Modifiers::NONE)],
        0.1,
        true,
    );
    assert!(
        matches!(
            c.selection_gesture,
            Some(SelectionGesture::Transform { .. })
        ),
        "benchmark must own a frozen group drag"
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
        "1,000 grouped ellipses / 40 changed movement preview frames: {:?}",
        start.elapsed()
    );
    assert_eq!(c.drawing.revision(), revision);
    assert!(
        meshes
            .iter()
            .all(|(id, m)| Arc::ptr_eq(m, &c.shape_cache[id].meshes[0]))
    );
}
