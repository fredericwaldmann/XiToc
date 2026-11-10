//! Shared command presentation for Edit and canvas context menus.
use crate::{canvas::Canvas, selection::SelectionCommand};
#[derive(Clone, Copy, Default)]
pub struct MenuState {
    pub editable: bool,
    pub selected: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_objects: bool,
}
impl MenuState {
    pub fn for_canvas(canvas: &Canvas) -> Self {
        Self {
            editable: true,
            selected: !canvas.selection.ids.is_empty(),
            can_undo: canvas.history_available().0,
            can_redo: canvas.history_available().1,
            has_objects: !canvas.drawing.items().is_empty(),
        }
    }
}
pub fn menu(ui: &mut egui::Ui, state: MenuState) -> Option<SelectionCommand> {
    use SelectionCommand::*;
    let mut command = None;
    for (label, key, enabled, value) in [
        ("Undo", "Ctrl/Cmd+Z", state.can_undo, Undo),
        ("Redo", "Ctrl/Cmd+Shift+Z", state.can_redo, Redo),
        ("Cut", "Ctrl/Cmd+X", state.selected, Cut),
        ("Copy", "Ctrl/Cmd+C", state.selected, Copy),
        ("Paste", "Ctrl/Cmd+V", state.editable, Paste),
        ("Duplicate", "Ctrl/Cmd+D", state.selected, Duplicate),
        ("Delete", "Delete / Backspace", state.selected, Delete),
        ("Select All", "Ctrl/Cmd+A", state.has_objects, SelectAll),
        ("Smaller", "Scale 90%", state.selected, Smaller),
        ("Larger", "Scale 110%", state.selected, Larger),
    ] {
        if ui
            .add_enabled(
                state.editable && enabled,
                egui::Button::new(label).shortcut_text(key),
            )
            .clicked()
        {
            command = Some(value);
        }
    }
    ui.add_enabled_ui(state.editable && state.selected, |ui| {
        ui.menu_button("Transform", |ui| {
            for (label, value) in [
                ("Rotate left 15°", RotateLeft),
                ("Rotate right 15°", RotateRight),
                ("Flip Horizontally", FlipHorizontal),
                ("Flip Vertically", FlipVertical),
            ] {
                if ui.button(label).clicked() {
                    command = Some(value);
                }
            }
        });
    });
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transform_submenu_routes_flip_and_retains_rotation_actions() {
        use crate::ui::settings::tests::pointer;
        for (label, expected) in [
            ("Flip Horizontally", SelectionCommand::FlipHorizontal),
            ("Flip Vertically", SelectionCommand::FlipVertical),
            ("Rotate left 15°", SelectionCommand::RotateLeft),
            ("Rotate right 15°", SelectionCommand::RotateRight),
        ] {
            let ctx = egui::Context::default();
            let mut chosen = None;
            let mut render = |events| {
                let mut targets = Vec::new();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800., 600.),
                        )),
                        events,
                        focused: true,
                        ..Default::default()
                    },
                    |ui| {
                        chosen = menu(
                            ui,
                            MenuState {
                                editable: true,
                                selected: true,
                                ..Default::default()
                            },
                        );
                    },
                );
                fn texts(shape: &egui::Shape, targets: &mut Vec<(String, egui::Pos2)>) {
                    match shape {
                        egui::Shape::Text(t) => targets
                            .push((t.galley.text().to_owned(), t.pos + t.galley.size() * 0.5)),
                        egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, targets)),
                        _ => {}
                    }
                }
                for s in &output.shapes {
                    texts(&s.shape, &mut targets);
                }
                output.textures_delta.clear();
                targets
            };
            render(vec![]);
            let targets = render(vec![]);
            let transform = targets.iter().find(|(s, _)| s == "Transform").unwrap().1;
            render(pointer(transform, true));
            render(pointer(transform, false));
            let targets = render(vec![]);
            let button = targets
                .iter()
                .find(|(s, _)| s == label)
                .expect("Transform command")
                .1;
            render(pointer(button, true));
            render(pointer(button, false));
            drop(render);
            assert_eq!(chosen, Some(expected));
        }
    }
}
