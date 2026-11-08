use super::tests::{frame, moved, pointer, screen, setup};
use super::*;
use crate::{document::DocumentKind, text::TextObject, text_layout};
fn key(key: egui::Key, command: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: command,
            command,
            ..Default::default()
        },
    }
}
fn value(c: &Canvas) -> &TextObject {
    c.drawing
        .items()
        .iter()
        .find_map(|o| match o {
            ObjectSnapshot::Text(t) => Some(t.as_ref()),
            _ => None,
        })
        .unwrap()
}
fn create(
    ctx: &egui::Context,
    c: &mut Canvas,
    s: Surface,
    view: Transform,
    start: Point,
    end: Point,
) {
    c.select(Tool::Text);
    frame(
        ctx,
        c,
        s,
        view,
        vec![
            moved(view, start),
            pointer(screen(view, start), true, Default::default()),
            moved(view, end),
            pointer(screen(view, end), false, Default::default()),
        ],
        0.,
        true,
    );
}
#[test]
fn click_to_type_focus_unicode_multiline_and_escape_preserves_selection_across_views() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.5, 1., 2., 4.] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            text_layout::install(&ctx);
            let view = Transform {
                scale,
                origin: [20.; 2],
                center: [0.; 2],
            };
            create(&ctx, &mut c, s, view, [30., 40.], [150., 40.]);
            assert!(c.text_session.is_some());
            assert!(c.drawing.items().is_empty());
            let source = "Hello café\nΕλληνικά e\u{301} 中文 🌍";
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![Event::Text(source.into())],
                0.1,
                true,
            );
            assert_eq!(value(&c).content.text, source);
            let id = value(&c).id;
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![key(egui::Key::Escape, false)],
                0.2,
                true,
            );
            assert!(!c.text_editing());
            assert!(c.selection.ids.contains(&id));
            assert!(c.tool == c.selection_mode);
            let after = c.drawing.items().to_vec();
            c.undo();
            assert!(c.drawing.items().is_empty());
            c.redo();
            assert_eq!(c.drawing.items(), after);
        }
    }
}
#[test]
fn empty_creation_escape_and_tool_handoff_never_insert_accidental_objects() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let view = super::tests::t();
    create(&ctx, &mut c, s, view, [40., 80.], [40., 80.]);
    assert!(!c.dirty());
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![key(egui::Key::Escape, false)],
        0.2,
        true,
    );
    assert!(c.drawing.items().is_empty());
    assert!(!c.drawing.can_undo());
    create(&ctx, &mut c, s, view, [50., 90.], [50., 90.]);
    c.select(Tool::Pen);
    assert!(!c.text_editing());
    assert!(c.drawing.items().is_empty());
}
#[test]
fn editing_shortcuts_select_and_replace_text_instead_of_objects_and_accept_ime() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let view = super::tests::t();
    create(&ctx, &mut c, s, view, [40., 80.], [180., 80.]);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![Event::Text("original words".into())],
        0.1,
        true,
    );
    let id = value(&c).id;
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            key(egui::Key::A, true),
            Event::Paste("Unicode\n二行目".into()),
        ],
        0.2,
        true,
    );
    assert_eq!(value(&c).content.text, "Unicode\n二行目");
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![key(egui::Key::D, true)],
        0.3,
        true,
    );
    assert_eq!(c.drawing.items().len(), 1);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![key(egui::Key::A, true), Event::Cut],
        0.4,
        true,
    );
    assert_eq!(value(&c).content.text, "");
    assert_eq!(value(&c).id, id);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![Event::Ime(egui::ImeEvent::Preedit {
            text: "にほ".into(),
            active_range_chars: None,
        })],
        0.5,
        true,
    );
    assert_eq!(value(&c).content.text, "");
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![Event::Ime(egui::ImeEvent::Commit("日本語".into()))],
        0.6,
        true,
    );
    assert_eq!(value(&c).content.text, "日本語");
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![key(egui::Key::Escape, false)],
        0.7,
        true,
    );
    assert!(c.selection.ids.contains(&id));
}
#[test]
fn width_drag_reflows_without_changing_font_and_one_frame_release_is_undoable() {
    for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
        for scale in [0.5, 1., 2., 4.] {
            let (mut c, s) = setup(kind);
            let ctx = egui::Context::default();
            text_layout::install(&ctx);
            let view = Transform {
                scale,
                origin: [20.; 2],
                center: [0.; 2],
            };
            create(&ctx, &mut c, s, view, [30., 40.], [150., 40.]);
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![Event::Text(
                    "alpha beta gamma delta epsilon zeta eta theta".into(),
                )],
                0.1,
                true,
            );
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![key(egui::Key::Escape, false)],
                0.2,
                true,
            );
            let source = value(&c).clone();
            let p = source.placement.apply([source.width, source.height * 0.5]);
            let target = [p[0] - 40., p[1]];
            let before = c.drawing.items().to_vec();
            frame(
                &ctx,
                &mut c,
                s,
                view,
                vec![
                    moved(view, p),
                    pointer(screen(view, p), true, Default::default()),
                    moved(view, target),
                    pointer(screen(view, target), false, Default::default()),
                ],
                0.3,
                true,
            );
            assert!(value(&c).width < source.width);
            assert!(value(&c).height > source.height);
            assert_eq!(value(&c).content, source.content);
            assert!(c.text_resize.is_none());
            c.undo();
            assert_eq!(c.drawing.items(), before);
        }
    }
}
#[test]
fn double_click_reedits_same_object_and_inside_drag_does_not_move_it() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let view = super::tests::t();
    create(&ctx, &mut c, s, view, [60., 160.], [220., 160.]);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![Event::Text("Edit this existing text".into())],
        0.1,
        true,
    );
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![key(egui::Key::Escape, false)],
        0.2,
        true,
    );
    let source = value(&c).clone();
    let p = [80., 170.];
    for time in [0.8, 0.9] {
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![
                moved(view, p),
                pointer(screen(view, p), true, Default::default()),
                pointer(screen(view, p), false, Default::default()),
            ],
            time,
            true,
        );
    }
    assert!(c.text_editing());
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            moved(view, p),
            pointer(screen(view, p), true, Default::default()),
            moved(view, [160., 170.]),
            pointer(screen(view, [160., 170.]), false, Default::default()),
        ],
        1.1,
        true,
    );
    assert_eq!(value(&c).placement, source.placement);
    assert_eq!(value(&c).id, source.id);
    assert_eq!(value(&c).content, source.content);
}
#[test]
fn typing_is_bounded_and_idle_frames_do_not_write_history_or_rebuild_other_objects() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let view = super::tests::t();
    create(&ctx, &mut c, s, view, [40., 160.], [180., 160.]);
    for i in 1..=70 {
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![Event::Text("a".into())],
            i as f64 * 0.01,
            true,
        );
    }
    assert_eq!(value(&c).content.text.len(), 70);
    let revision = c.drawing.revision();
    for i in 0..10 {
        frame(&ctx, &mut c, s, view, vec![], 1. + i as f64 * 0.1, true);
    }
    assert_eq!(c.drawing.revision(), revision);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![key(egui::Key::Escape, false)],
        2.2,
        true,
    );
    c.undo();
    assert!(!c.drawing.items().is_empty());
    assert!(value(&c).content.text.len() < 70);
}

#[test]
fn text_in_mixed_connected_group_follows_persistent_offscreen_drag_and_exact_undo() {
    use crate::{
        connector::{Connector, Endpoint, Port},
        object::Placement,
        shape::{Shape, ShapeKind},
    };
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let view = Transform {
        scale: 1.5,
        origin: [20.; 2],
        center: [10., -20.],
    };
    frame(&ctx, &mut c, s, view, vec![], 0., true);
    let mut text = TextObject::new(c.drawing.owner(), s, [40., 160.], 120.);
    text.content_mut().text = "Move this text".into();
    text_layout::measure(&ctx, &mut text);
    assert!(c.drawing.replace_texts(vec![text.clone()], false));
    let a = Shape::create(
        c.drawing.owner(),
        s,
        ShapeKind::Rectangle,
        [250., 160.],
        [330., 220.],
        false,
    )
    .unwrap();
    let b = Shape::create(
        c.drawing.owner(),
        s,
        ShapeKind::Rectangle,
        [450., 160.],
        [530., 220.],
        false,
    )
    .unwrap();
    let edge = Connector::new(
        c.drawing.owner(),
        s,
        Endpoint::attached(&a, Port::Right).unwrap(),
        Endpoint::attached(&b, Port::Left).unwrap(),
        Default::default(),
    );
    let ids = HashSet::from([text.id, a.id, b.id, edge.id]);
    assert!(c.drawing.add_shape(a));
    assert!(c.drawing.add_shape(b));
    assert!(c.drawing.add_connector(edge));
    c.select(Tool::Select);
    c.selection
        .change(ids.clone(), Membership::Replace, &c.drawing);
    let before = c.drawing.items().to_vec();
    let start = [60., 170.];
    let end = [900., 300.];
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            moved(view, start),
            pointer(screen(view, start), true, Default::default()),
            moved(view, [1000., 260.]),
            Event::PointerGone,
            moved(view, end),
        ],
        0.2,
        true,
    );
    assert!(c.selection_gesture.is_some());
    assert_eq!(c.drawing.items(), before);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![pointer(screen(view, end), false, Default::default())],
        0.3,
        true,
    );
    assert_eq!(c.selection.ids, ids);
    let expected = text.transformed(Placement {
        position: [end[0] - start[0], end[1] - start[1]],
        ..Default::default()
    });
    assert_eq!(value(&c), &expected);
    c.undo();
    assert_eq!(c.drawing.items(), before);
}
#[test]
fn edit_menu_commands_reach_text_editor_and_keep_object_identity() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let view = super::tests::t();
    create(&ctx, &mut c, s, view, [40., 160.], [180., 160.]);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![Event::Text("Copy selected text".into())],
        0.1,
        true,
    );
    let id = value(&c).id;
    c.selection_action(SelectionCommand::SelectAll, &ctx);
    frame(&ctx, &mut c, s, view, vec![], 0.2, true);
    // Opening an application menu transfers focus away from TextEdit. Its
    // selected range must survive when the menu dispatches Copy/Cut.
    ctx.memory_mut(|m| m.request_focus(egui::Id::unique("application-edit-menu")));
    frame(&ctx, &mut c, s, view, vec![], 0.25, true);
    c.selection_action(SelectionCommand::Copy, &ctx);
    frame(&ctx, &mut c, s, view, vec![], 0.3, true);
    assert_eq!(
        ctx.data(|d| d.get_temp::<String>(egui::Id::unique("XiToc-text-clipboard")))
            .unwrap(),
        "Copy selected text"
    );
    c.selection_action(SelectionCommand::Cut, &ctx);
    frame(&ctx, &mut c, s, view, vec![], 0.4, true);
    assert_eq!(value(&c).id, id);
    assert_eq!(value(&c).content.text, "");
    c.selection_action(SelectionCommand::Undo, &ctx);
    frame(&ctx, &mut c, s, view, vec![], 0.5, true);
    assert_eq!(value(&c).content.text, "Copy selected text");
    assert!(c.text_editing());
}

#[test]
fn real_editor_creation_with_separate_press_release_emits_ime_without_second_click() {
    use crate::{document::DocumentData, editor::Editor};
    let data = DocumentData::blank(DocumentKind::Whiteboard);
    let mut editor = Editor::new(&data);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    editor.canvas.select(Tool::Text);
    let mut render = |events, time| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.))),
                focused: true,
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| editor.show(ui, &data),
        );
        output.textures_delta.clear();
        output.platform_output.ime
    };
    render(vec![], 0.);
    render(
        vec![
            Event::PointerMoved(egui::pos2(200., 200.)),
            pointer([200., 200.], true, Default::default()),
        ],
        0.1,
    );
    render(vec![pointer([200., 200.], false, Default::default())], 0.2);
    assert!(
        render(vec![], 0.3).is_some(),
        "New text box did not publish IME/caret output"
    );
}
#[test]
fn unchanged_text_reuses_layout_during_unrelated_edits() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let view = super::tests::t();
    create(&ctx, &mut c, s, view, [40., 160.], [180., 160.]);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![Event::Text("Cache this Unicode café 中文".into())],
        0.1,
        true,
    );
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![key(egui::Key::Escape, false)],
        0.2,
        true,
    );
    frame(&ctx, &mut c, s, view, vec![], 0.3, true);
    let id = value(&c).id;
    let cached = c.text_cache[&id].galley.clone();
    let mut ink = Stroke::new(c.drawing.owner(), s, [0, 0, 0, 255], 2.);
    ink.push([500., 400.]);
    ink.push([600., 450.]);
    assert!(c.drawing.add(ink));
    frame(&ctx, &mut c, s, view, vec![], 0.4, true);
    assert!(Arc::ptr_eq(&cached, &c.text_cache[&id].galley));
}
#[test]
fn oversized_paste_is_rejected_without_losing_existing_source() {
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let ctx = egui::Context::default();
    text_layout::install(&ctx);
    let view = super::tests::t();
    create(&ctx, &mut c, s, view, [40., 160.], [180., 160.]);
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![Event::Text("Keep this".into())],
        0.1,
        true,
    );
    frame(
        &ctx,
        &mut c,
        s,
        view,
        vec![
            key(egui::Key::A, true),
            Event::Paste("a".repeat(crate::text::MAX_TEXT_BYTES + 1)),
        ],
        0.2,
        true,
    );
    assert_eq!(value(&c).content.text, "Keep this");
}
#[test]
fn a_text_body_takes_pick_priority_over_a_crossing_connector() {
    use crate::connector::{Connector, Endpoint};
    let (mut c, s) = setup(DocumentKind::Whiteboard);
    let mut text = TextObject::new(c.drawing.owner(), s, [40., 160.], 120.);
    text.content_mut().text = "Text in front of a wire".into();
    let id = text.id;
    assert!(c.drawing.replace_texts(vec![text], false));
    let wire = Connector::new(
        c.drawing.owner(),
        s,
        Endpoint::Free {
            position: [0., 170.],
        },
        Endpoint::Free {
            position: [200., 170.],
        },
        Default::default(),
    );
    assert!(c.drawing.add_connector(wire));
    assert_eq!(c.pick(s, [60., 170.], 1.), Some(id));
}
