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
                history(ui, &mut editor.canvas);
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
        history(ui, &mut editor.canvas);
        menu(ui, "View", |ui| view(ui, doc, editor));
        export(ui, action, editor.canvas.dirty());
    });
}

fn history(ui: &mut egui::Ui, canvas: &mut Canvas) {
    for (redo, label, enabled) in [
        (false, "Undo", canvas.drawing.can_undo()),
        (true, "Redo", canvas.drawing.can_redo()),
    ] {
        if ui
            .add_enabled(enabled, egui::Button::new(label).frame_when_inactive(false))
            .on_hover_text(if redo {
                "Ctrl/Cmd+Shift+Z"
            } else {
                "Ctrl/Cmd+Z"
            })
            .clicked()
        {
            if redo {
                canvas.redo();
            } else {
                canvas.undo();
            }
        }
    }
}

/// Foreground UI is registered before the canvas, so its hit regions own input.
pub fn palette(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    prefs: &mut crate::settings::DrawingPreferences,
) -> egui::Rect {
    let bounds = ui.available_rect_before_wrap();
    let p = style::palette(ui);
    egui::Area::new(egui::Id::unique("drawing-palette").with(editor.canvas.drawing.owner()))
        .order(egui::Order::Foreground)
        .movable(false)
        .enabled(ui.is_enabled())
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(
            bounds.center().x,
            bounds.bottom() - style::PALETTE_INSET,
        ))
        .constrain_to(bounds)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(p.surface)
                .stroke(egui::Stroke::new(1.0, p.border))
                .corner_radius(style::PALETTE_RADIUS)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 3],
                    blur: 12,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(24),
                })
                .inner_margin(6)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (tool, icon, label) in [
                            (Tool::Pan, Icon::Hand, "Hand · Space-drag to pan"),
                            (Tool::Pen, Icon::Pen, "Pen · click again for settings"),
                            (
                                Tool::Eraser,
                                Icon::Eraser,
                                "Eraser · click again for settings",
                            ),
                        ] {
                            let selected = editor.canvas.tool == tool;
                            let response = ui.add(
                                egui::Button::new("")
                                    .min_size(egui::Vec2::splat(style::PALETTE_TOOL))
                                    .selected(selected),
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::Role::Button,
                                    true,
                                    selected,
                                    label,
                                )
                            });
                            icons::paint(
                                ui.painter(),
                                response.rect.center(),
                                icon,
                                if selected { p.accent } else { p.text },
                            );
                            let response = response.on_hover_text(label);
                            if response.clicked() {
                                editor.canvas.select(tool);
                            }
                            if tool != Tool::Pan && ui.is_enabled() {
                                egui::Popup::from_toggle_button_response(&response)
                                    .align(egui::emath::RectAlign::TOP)
                                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                    .width(style::TOOL_POPOVER_WIDTH.min(bounds.width() - 24.0))
                                    .show(|ui| {
                                        egui::ScrollArea::vertical()
                                            .max_height((bounds.height() - 90.0).max(60.0))
                                            .show(ui, |ui| {
                                                if tool == Tool::Pen {
                                                    ui.strong("Pen");
                                                    super::drawing_controls::pen(
                                                        ui,
                                                        &mut editor.canvas.color,
                                                        &mut editor.canvas.width,
                                                    );
                                                    ui.separator();
                                                    super::drawing_controls::response(ui, prefs);
                                                } else {
                                                    ui.strong("Eraser");
                                                    for mode in
                                                        [EraserMode::Stroke, EraserMode::Partial]
                                                    {
                                                        if ui
                                                            .selectable_value(
                                                                &mut prefs.eraser_mode,
                                                                mode,
                                                                mode.label(),
                                                            )
                                                            .changed()
                                                        {
                                                            editor.canvas.finish();
                                                            editor.canvas.eraser_mode = mode;
                                                        }
                                                    }
                                                    super::drawing_controls::eraser_radius(
                                                        ui,
                                                        &mut editor.canvas.eraser_radius,
                                                    );
                                                }
                                            });
                                    });
                            }
                        }
                    });
                });
        })
        .response
        .rect
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
    fn floating_popovers_own_input_and_edit_existing_preferences() {
        use crate::ui::settings::tests::{pointer, render};
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let ctx = egui::Context::default();
            let data = crate::document::DocumentData::blank(kind);
            let mut editor = Editor::new(&data);
            let mut prefs = crate::settings::DrawingPreferences::default();
            let mut rect = egui::Rect::NOTHING;
            let mut frame = |events| {
                render(&ctx, events, |ui| {
                    rect = palette(ui, &mut editor, &mut prefs);
                    editor.show(ui, &data);
                })
            };
            frame(vec![]);
            frame(vec![]);
            drop(frame);
            let pen = rect.center();
            let eraser = pen + egui::vec2(style::PALETTE_TOOL + 4.0, 0.0);
            let mut frame = |events| {
                render(&ctx, events, |ui| {
                    palette(ui, &mut editor, &mut prefs);
                    editor.show(ui, &data);
                })
            };
            frame(pointer(pen, true));
            frame(pointer(pen, false));
            let labels = frame(vec![]);
            assert!(labels.contains_key("Pen color"));
            assert!(labels.contains_key("Pen pressure"));
            frame(pointer(eraser, true));
            frame(pointer(eraser, false));
            let labels = frame(vec![]);
            let partial = labels["Partial Eraser"];
            frame(pointer(partial, true));
            frame(pointer(partial, false));
            assert!(frame(vec![]).contains_key("Eraser radius"));
            drop(frame);
            render(
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                |ui| {
                    palette(ui, &mut editor, &mut prefs);
                    editor.show(ui, &data);
                },
            );
            let labels = render(&ctx, vec![], |ui| {
                palette(ui, &mut editor, &mut prefs);
                editor.show(ui, &data);
            });
            assert!(
                !labels.contains_key("Eraser radius"),
                "Escape dismisses settings"
            );
            let hand = pen - egui::vec2(style::PALETTE_TOOL + 4.0, 0.0);
            for events in [vec![], pointer(hand, true), pointer(hand, false)] {
                render(&ctx, events, |ui| {
                    ui.disable();
                    palette(ui, &mut editor, &mut prefs);
                    editor.show(ui, &data);
                });
            }
            assert_eq!(prefs.eraser_mode, EraserMode::Partial);
            assert_eq!(editor.canvas.eraser_mode, EraserMode::Partial);
            assert!(editor.canvas.tool == Tool::Eraser);
            assert!(
                !editor.canvas.dirty(),
                "Palette/popover input must never draw ink"
            );
        }
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
