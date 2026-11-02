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
            can_undo: canvas.drawing.can_undo(),
            can_redo: canvas.drawing.can_redo(),
            has_objects: !canvas.drawing.strokes().is_empty(),
        }
    }
}
pub fn menu(ui: &mut egui::Ui, state: MenuState) -> Option<SelectionCommand> {
    use SelectionCommand::*;
    let mut command = None;
    for (label, key, enabled, value) in [
        ("Undo", "Ctrl/Cmd+Z", state.can_undo, Undo),
        ("Redo", "Ctrl/Cmd+Shift+Z", state.can_redo, Redo),
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
    command
}
