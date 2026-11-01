use super::{
    Action,
    icons::{self, Icon},
    style,
};
use crate::{
    canvas::{Canvas, Tool},
    document::{Document, DocumentKind},
    drawing::EraserMode,
    editor::Editor,
};

pub fn show(
    ui: &mut egui::Ui,
    document: Option<(&Document, &mut Editor)>,
    action: &mut Option<Action>,
    eraser_mode: &mut EraserMode,
) {
    let p = style::palette(ui);
    let width = ui.available_width();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let Some((doc, editor)) = document else {
            ui.weak("Open a document to begin");
            return;
        };
        if editor.canvas.eraser_mode != *eraser_mode {
            editor.canvas.finish();
            editor.canvas.eraser_mode = *eraser_mode;
        }
        icons::show(ui, doc.data.kind().into(), p.accent);
        if width < 360.0 {
            menu(ui, doc.data.kind().label(), |ui| {
                tools(ui, doc, editor, eraser_mode);
                ui.separator();
                view(ui, doc, editor);
                export(ui, action, editor.canvas.dirty());
            });
            return;
        }
        ui.label(
            egui::RichText::new(doc.data.kind().label())
                .small()
                .color(p.muted),
        );
        ui.separator();
        if width >= 760.0 {
            tools(ui, doc, editor, eraser_mode);
        } else {
            menu(ui, "Tools", |ui| tools(ui, doc, editor, eraser_mode));
        }
        ui.separator();
        if width >= 1100.0 {
            view(ui, doc, editor);
        } else {
            menu(ui, "View", |ui| view(ui, doc, editor));
        }
        export(ui, action, editor.canvas.dirty());
    });
}

fn tools(ui: &mut egui::Ui, doc: &Document, editor: &mut Editor, eraser_mode: &mut EraserMode) {
    ui.add_enabled_ui(!doc.external, |ui| {
        ui.horizontal(|ui| {
            for (tool, label, hint) in [
                (Tool::Pen, "Pen", "Freehand ink"),
                (Tool::Eraser, "Eraser", "Erase whole strokes"),
                (Tool::Pan, "Hand", "Pan without drawing; or hold Space"),
            ] {
                if tool == Tool::Eraser {
                    eraser_menu(ui, &mut editor.canvas, eraser_mode);
                    continue;
                }
                if ui
                    .selectable_label(editor.canvas.tool == tool, label)
                    .on_hover_text(hint)
                    .clicked()
                {
                    editor.canvas.select(tool);
                }
            }
            menu(ui, "Ink", |ui| {
                let old_color = editor.canvas.color;
                super::drawing_controls::show(
                    ui,
                    &mut editor.canvas.color,
                    &mut editor.canvas.width,
                    &mut editor.canvas.eraser_radius,
                );
                if editor.canvas.color != old_color {
                    editor.canvas.finish();
                }
                ui.separator();
                ui.horizontal(|ui| {
                    for name in ["Select", "Text", "Shapes"] {
                        ui.add_enabled(false, egui::Button::new(name))
                            .on_disabled_hover_text(format!("{name} is unavailable until M6"));
                    }
                });
            });
            if ui
                .add_enabled(
                    editor.canvas.drawing.can_undo(),
                    egui::Button::new("Undo").frame_when_inactive(false),
                )
                .on_hover_text("Ctrl/Cmd+Z")
                .clicked()
            {
                editor.canvas.undo();
            }
            if ui
                .add_enabled(
                    editor.canvas.drawing.can_redo(),
                    egui::Button::new("Redo").frame_when_inactive(false),
                )
                .on_hover_text("Ctrl/Cmd+Shift+Z")
                .clicked()
            {
                editor.canvas.redo();
            }
        });
    });
}

fn eraser_menu(ui: &mut egui::Ui, canvas: &mut Canvas, mode: &mut EraserMode) {
    let config = egui::containers::menu::MenuConfig::new()
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    let button = egui::Button::new(mode.label())
        .selected(canvas.tool == Tool::Eraser)
        .right_text(egui::containers::menu::SubMenuButton::arrow_atom(None));
    let content = |ui: &mut egui::Ui| {
        for choice in [EraserMode::Stroke, EraserMode::Partial] {
            if ui
                .selectable_label(*mode == choice, choice.label())
                .clicked()
            {
                canvas.select(Tool::Eraser);
                canvas.eraser_mode = choice;
                *mode = choice;
                ui.close();
            }
        }
        ui.separator();
        super::drawing_controls::eraser_radius(ui, &mut canvas.eraser_radius);
    };
    if egui::containers::menu::is_in_menu(ui) {
        egui::containers::menu::SubMenuButton::from_button(button)
            .config(config)
            .ui(ui, content);
    } else {
        egui::containers::menu::MenuButton::from_button(button)
            .config(config)
            .ui(ui, content);
    }
}

fn view(ui: &mut egui::Ui, doc: &Document, editor: &mut Editor) {
    if doc.external {
        ui.label("Read-only · preview unavailable");
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if icons::button(ui, Icon::Minus, "Zoom out", false).clicked() {
            editor.zoom_by(0.8);
        }
        ui.add_sized(
            [44.0, 26.0],
            egui::Label::new(format!("{:.0}%", editor.zoom() * 100.0)),
        );
        if icons::button(ui, Icon::Plus, "Zoom in", false).clicked() {
            editor.zoom_by(1.25);
        }
        if icons::button(ui, Icon::Reset, "Reset view / fit page width", false).clicked() {
            editor.reset_view();
        }
        if doc.data.kind() == DocumentKind::Whiteboard
            && icons::button(
                ui,
                Icon::Grid,
                "Toggle dot grid (session only)",
                editor.grid_visible(),
            )
            .clicked()
        {
            editor.toggle_grid();
        }
    });
}

fn export(_ui: &mut egui::Ui, _action: &mut Option<Action>, _dirty: bool) {
    #[cfg(target_arch = "wasm32")]
    if _ui
        .add_enabled(!_dirty, egui::Button::new("Export PDF…"))
        .on_disabled_hover_text("Unsaved ink is memory-only. Drawing PDF export arrives in M2.")
        .clicked()
    {
        *_action = Some(Action::Export);
    }
}

// Settings contain interactive pickers/drag values: inside clicks must not close
// their parent menu. Preserve egui's actual submenu hierarchy at narrow widths.
fn menu(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    let config = egui::containers::menu::MenuConfig::new()
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    if egui::containers::menu::is_in_menu(ui) {
        egui::containers::menu::SubMenuButton::new(label)
            .config(config)
            .ui(ui, content);
    } else {
        egui::containers::menu::MenuButton::new(label)
            .config(config)
            .ui(ui, content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eraser_dropdown_selects_mode_and_exposes_size() {
        use crate::ui::settings::tests::{pointer, render};
        let ctx = egui::Context::default();
        let mut canvas = Canvas::new(&crate::document::DocumentData::blank(
            DocumentKind::Whiteboard,
        ));
        let mut mode = EraserMode::Stroke;
        fn click(ctx: &egui::Context, canvas: &mut Canvas, mode: &mut EraserMode, label: &str) {
            render(ctx, vec![], |ui| eraser_menu(ui, canvas, mode));
            let labels = render(ctx, vec![], |ui| eraser_menu(ui, canvas, mode));
            let pos = *labels.get(label).unwrap();
            render(ctx, pointer(pos, true), |ui| eraser_menu(ui, canvas, mode));
            render(ctx, pointer(pos, false), |ui| eraser_menu(ui, canvas, mode));
        }
        click(&ctx, &mut canvas, &mut mode, "Stroke Eraser");
        click(&ctx, &mut canvas, &mut mode, "Partial Eraser");
        assert_eq!(mode, EraserMode::Partial);
        assert_eq!(canvas.eraser_mode, mode);
        assert!(canvas.tool == Tool::Eraser);
        click(&ctx, &mut canvas, &mut mode, "Partial Eraser");
        let labels = render(&ctx, vec![], |ui| eraser_menu(ui, &mut canvas, &mut mode));
        assert!(labels.contains_key("Eraser radius"));
    }

    #[test]
    fn settings_menu_stays_open_while_adjusting_a_control() {
        let ctx = egui::Context::default();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut clicks = 0;
        let mut render = |events| {
            let mut target = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 300.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    menu(ui, "Ink", |ui| {
                        let response = ui.button("Adjust");
                        target = Some(response.rect.center());
                        if response.clicked() {
                            clicks += 1;
                        }
                    });
                },
            );
            output.textures_delta.clear();
            target
        };
        render(vec![]);
        let opener = egui::pos2(20.0, 13.0);
        render(vec![
            egui::Event::PointerMoved(opener),
            button(opener, true),
        ]);
        render(vec![button(opener, false)]);
        let target = render(vec![]).expect("Settings must be open");
        render(vec![
            egui::Event::PointerMoved(target),
            button(target, true),
        ]);
        render(vec![button(target, false)]);
        assert!(
            render(vec![]).is_some(),
            "Adjusting a setting must keep its menu open"
        );
        assert_eq!(clicks, 1);
    }
}
