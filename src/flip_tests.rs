//! Reflection regressions through the shared placement, selection and journal.
use super::manipulation_tests::pressured;
use super::tests::{frame, moved, screen, setup, stroke, t};
use super::*;
use crate::{document::DocumentKind, object::CanvasObject};

fn near(a: Point, b: Point) {
    for i in 0..2 {
        assert!((a[i] - b[i]).abs() < 0.001, "{a:?} != {b:?}");
    }
}

#[test]
fn flips_single_groups_rotated_scaled_pressure_and_exact_history_independent_of_zoom() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.25, 1., 4.] {
            for group in [false, true] {
                for rotated in [false, true] {
                    for command in [
                        SelectionCommand::FlipHorizontal,
                        SelectionCommand::FlipVertical,
                    ] {
                        let (mut c, s) = setup(kind);
                        let ctx = egui::Context::default();
                        let a = pressured(&mut c, s);
                        let b = stroke(&mut c, s, [110., 120.], [140., 140.]);
                        let untouched = stroke(&mut c, s, [200., 200.], [220., 210.]);
                        let ids = if group {
                            HashSet::from([a, b])
                        } else {
                            HashSet::from([a])
                        };
                        if rotated {
                            assert!(c.drawing.transform_objects(
                                &ids,
                                Placement {
                                    origin: [100., 100.],
                                    position: [100., 100.],
                                    scale: 1.4,
                                    rotation: 0.37,
                                    ..Default::default()
                                }
                            ));
                        }
                        c.select(Tool::Lasso);
                        c.selection
                            .change(ids.clone(), Membership::Replace, &c.drawing);
                        let view = Transform { scale, ..t() };
                        frame(&ctx, &mut c, s, view, vec![], 0., true);
                        let bounds = c.selection.bounds.unwrap();
                        let center = bounds.center();
                        let original = c.drawing.strokes().to_vec();
                        let revision = c.drawing.revision();
                        let untouched_cache = c.cache[&untouched].meshes.first().cloned();
                        c.selection_action(command, &ctx);
                        assert_eq!(c.drawing.revision(), revision + 1);
                        assert_eq!(c.selection.ids, ids);
                        assert!(c.tool == Tool::Lasso);
                        near(c.selection.bounds.unwrap().min, bounds.min);
                        near(c.selection.bounds.unwrap().max, bounds.max);
                        let flipped = c.drawing.strokes().to_vec();
                        for (old, new) in original.iter().zip(&flipped) {
                            if !ids.contains(&old.id) {
                                assert!(Arc::ptr_eq(old, new));
                                continue;
                            }
                            let mut properties = (**new).clone();
                            properties.placement = old.placement;
                            assert_eq!(properties, **old);
                            assert!(new.placement.valid());
                            assert_eq!(new.visible_width(), old.visible_width());
                            for (before, after) in old.path().into_iter().zip(new.path()) {
                                let axis = usize::from(command == SelectionCommand::FlipVertical);
                                let mut expected = before;
                                expected[axis] = center[axis] - (before[axis] - center[axis]);
                                near(after, expected);
                                assert!(new.hit_test(after, 0.01));
                            }
                        }
                        if let Some(base) = untouched_cache {
                            assert!(Arc::ptr_eq(&base, &c.cache[&untouched].meshes[0]));
                        }
                        c.undo();
                        assert_eq!(c.drawing.strokes(), original);
                        assert_eq!(c.selection.ids, ids);
                        c.redo();
                        assert_eq!(c.drawing.strokes(), flipped);
                        c.selection_action(command, &ctx);
                        for (old, new) in original.iter().zip(c.drawing.strokes()) {
                            for (a, b) in old.path().into_iter().zip(new.path()) {
                                near(a, b);
                            }
                        }
                        // The second flip is one separate entry, too.
                        c.undo();
                        assert_eq!(c.drawing.strokes(), flipped);
                        c.undo();
                        assert_eq!(c.drawing.strokes(), original);
                    }
                }
            }
        }
    }
}

#[test]
fn reflected_ink_clipboard_partial_erasure_and_followup_transforms() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    let id = pressured(&mut c, s);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    c.selection_action(SelectionCommand::FlipHorizontal, &ctx);
    let reflected = c.drawing.strokes()[0].clone();
    let bundle = ObjectBundle::capture(std::iter::once((*reflected).clone())).unwrap();
    let decoded = ObjectBundle::decode(&bundle.encode().unwrap()).unwrap();
    let copies = decoded.paste(&mut c.drawing, s, [300., 300.]);
    assert_eq!(copies.len(), 1);
    let pasted = c.drawing.strokes()[1].clone();
    assert_eq!(pasted.placement.reflected, reflected.placement.reflected);
    assert_eq!(pasted.points, reflected.points);
    for delta in [
        Placement {
            position: [10., 20.],
            ..Default::default()
        },
        Placement {
            origin: [80., 70.],
            position: [80., 70.],
            scale: 1.6,
            ..Default::default()
        },
        Placement {
            origin: [80., 70.],
            position: [80., 70.],
            rotation: 0.8,
            ..Default::default()
        },
    ] {
        let old = c.drawing.strokes()[0].clone();
        assert!(c.drawing.transform_objects(&HashSet::from([id]), delta));
        let new = &c.drawing.strokes()[0];
        for (a, b) in old.path().into_iter().zip(new.path()) {
            near(delta.apply(a), b);
        }
        for p in &new.points {
            near(
                new.placement.inverse(new.placement.apply(p.position)),
                p.position,
            );
        }
    }
    let source = c.drawing.strokes()[0].clone();
    let path = source.path();
    let pressures = crate::partial_erase::path_pressures(&source, &path).unwrap();
    let mut cut = crate::partial_erase::PartialCut::with_pressure(&path, &source, &pressures);
    let midpoint = path[path.len() / 2];
    assert!(cut.sweep(&path, s, midpoint, midpoint, 2., |_, _| {}));
    let fragments = crate::partial_erase::fragments(&source, &path, &cut.cuts);
    assert!(!fragments.is_empty());
    for fragment in &fragments {
        assert_eq!(fragment.placement, source.placement);
        assert_eq!(fragment.pressure_style, source.pressure_style);
        assert!(
            fragment
                .path()
                .iter()
                .all(|p| p.iter().all(|v| v.is_finite()))
        );
    }
}

fn secondary(pos: Point, pressed: bool) -> Event {
    Event::PointerButton {
        pos: egui::pos2(pos[0] as f32, pos[1] as f32),
        button: PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn right_click_preserves_group_selects_other_and_empty_canvas_does_not_edit() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for mode in [
            Tool::Select,
            Tool::Lasso,
            Tool::Pen,
            Tool::Eraser,
            Tool::Pan,
        ] {
            let (mut c, s) = setup(kind);
            let a = stroke(&mut c, s, [100., 200.], [140., 220.]);
            let b = stroke(&mut c, s, [200., 300.], [240., 310.]);
            let other = stroke(&mut c, s, [400., 400.], [440., 420.]);
            c.select(mode);
            c.selection.change([a, b], Membership::Replace, &c.drawing);
            let original = c.drawing.strokes().to_vec();
            for (point, expected) in [
                ([120., 210.], HashSet::from([a, b])),
                ([420., 410.], HashSet::from([other])),
                ([500., 500.], HashSet::from([other])),
            ] {
                let ctx = egui::Context::default();
                frame(&ctx, &mut c, s, t(), vec![], 0., true);
                frame(
                    &ctx,
                    &mut c,
                    s,
                    t(),
                    vec![moved(t(), point), secondary(screen(t(), point), true)],
                    0.1,
                    true,
                );
                frame(
                    &ctx,
                    &mut c,
                    s,
                    t(),
                    vec![secondary(screen(t(), point), false)],
                    0.2,
                    true,
                );
                assert_eq!(c.selection.ids, expected);
                assert!(egui::Popup::is_any_open(&ctx));
                assert_eq!(c.drawing.strokes(), original);
            }
        }
    }
}

#[test]
fn flips_reject_page_escape_atomically_and_whiteboard_handles_far_coordinates() {
    let ctx = egui::Context::default();
    let (mut c, s) = setup(DocumentKind::Page);
    // A wider stroke at the left edge shifts the collective envelope center far
    // enough to mirror a thin stroke's centerline beyond the paper.
    let mut wide = Stroke::new(c.drawing.owner(), s, [0, 0, 0, 255], 24.);
    wide.push([0., 100.]);
    wide.push([10., 100.]);
    assert!(c.drawing.add(wide));
    stroke(&mut c, s, [50., 150.], [60., 150.]);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    let before = c.drawing.strokes().to_vec();
    let revision = c.drawing.revision();
    c.selection_action(SelectionCommand::FlipHorizontal, &ctx);
    assert_eq!(c.drawing.revision(), revision);
    assert_eq!(c.drawing.strokes(), before);
    assert!(c.notice.as_ref().unwrap().contains("leave this page"));
    c.undo(); // No failed-flip history entry: undoes the preceding stroke insert.
    assert_eq!(c.drawing.strokes().len(), 1);

    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let id = stroke(&mut c, s, [1e12, -1e12], [1e12 + 50., -1e12 + 20.]);
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    let before = c.drawing.strokes()[0].clone();
    c.selection_action(SelectionCommand::FlipVertical, &ctx);
    assert!(c.selection.ids.contains(&id));
    near(c.drawing.strokes()[0].path()[0], [1e12, -1e12 + 20.]);
    c.selection_action(SelectionCommand::FlipVertical, &ctx);
    for (a, b) in before.path().into_iter().zip(c.drawing.strokes()[0].path()) {
        near(a, b);
    }
}
