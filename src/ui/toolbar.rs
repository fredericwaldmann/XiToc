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
    prefs: &mut crate::settings::DrawingPreferences,
) {
    let Some((doc, editor)) = document else {
        ui.weak("Open a document to begin");
        return;
    };
    if editor.canvas.eraser_mode != prefs.eraser_mode {
        editor.canvas.finish();
        editor.canvas.eraser_mode = prefs.eraser_mode;
    }
    let width = ui.available_width();
    ui.push_id(doc.data.id(), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if doc.external { ui.weak("Read-only PDF"); return; }
            for (tool,icon,label) in [(Tool::Select,Icon::Select,"Selection · Shift-click to toggle"),(Tool::Lasso,Icon::Lasso,"Lasso · enclose complete strokes; Shift adds, Alt subtracts")] {
                if icons::button(ui,icon,label,editor.canvas.tool==tool).clicked(){editor.canvas.select(tool);}
            }
            if icons::button(ui, Icon::Hand, "Hand · Space-drag to pan", editor.canvas.tool == Tool::Pan).clicked() { editor.canvas.select(Tool::Pan); }
            let reserved = if width >= 1000.0 { 480.0 } else if width >= 800.0 { 380.0 } else if width >= 500.0 { 268.0 } else { 210.0 };
            let reserved=reserved+if width>=400.0{30.0}else{0.0};
            let strip_width = (width - reserved).max(24.0);
            ui.allocate_ui(egui::vec2(strip_width, style::CONTROL), |ui| {
                egui::ScrollArea::horizontal().id_salt("pen-presets").min_scrolled_width(24.0).auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let reveal_id = ui.make_persistent_id("revealed-preset");
                            let position = prefs.presets.items.iter().position(|p| p.id == prefs.presets.selected).unwrap();
                            let current = (prefs.presets.selected, position, strip_width as i32);
                            let reveal = ui.data_mut(|data| {
                                let changed = data.get_temp::<(uuid::Uuid, usize, i32)>(reveal_id) != Some(current);
                                data.insert_temp(reveal_id, current);
                                changed
                            });
                            for preset in &prefs.presets.items {
                                let selected = editor.canvas.tool == Tool::Pen && prefs.presets.selected == preset.id;
                                let response = ui.push_id(preset.id, |ui| {
                                    let response = ui.add(egui::Button::new("").min_size(egui::vec2(style::PRESET_WIDTH, style::CONTROL)).selected(selected));
                                    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, preset.label()));
                                    icons::paint(ui.painter(), response.rect.center() - egui::vec2(9.0, 0.0), Icon::Pen, style::palette(ui).text);
                                    let c = preset.style.rgba();
                                    let color = egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]);
                                    ui.painter().line_segment([response.rect.center() + egui::vec2(2.0, 3.0), response.rect.center() + egui::vec2(12.0, -3.0)], egui::Stroke::new((preset.style.width as f32).clamp(1.0, 8.0), color));
                                    response.on_hover_text(format!("{} · {}\n{:.2} pt · {}% opacity\nClick to draw; edit with Pen settings", preset.label(), preset.style.pen_type.label(), preset.style.width, preset.style.opacity))
                                }).inner;
                                if response.clicked() {
                                    editor.canvas.finish();
                                    prefs.presets.selected = preset.id;
                                    editor.canvas.select(Tool::Pen);
                                }
                                if (selected && reveal) || response.has_focus() { response.scroll_to_me(Some(egui::Align::Center)); }
                            }
                        });
                    });
            });
            let added = width>=400.0 && ui.add_enabled_ui(prefs.presets.items.len() < crate::pen::MAX_PRESETS, |ui| icons::button(ui, Icon::Plus, "New pen preset · copy selected", false)).inner.clicked();
            if added { prefs.presets.duplicate(); editor.canvas.select(Tool::Pen); }
            let settings = icons::button(ui, Icon::Settings, "Pen settings", false);
            egui::Popup::from_toggle_button_response(&settings)
                .open_memory(if added { Some(egui::SetOpenCommand::Bool(true)) } else { settings.clicked().then_some(egui::SetOpenCommand::Toggle) })
                .close_behavior(if added { egui::PopupCloseBehavior::IgnoreClicks } else { egui::PopupCloseBehavior::CloseOnClickOutside })
                .width(style::TOOL_POPOVER_WIDTH)
                .show(|ui| { egui::ScrollArea::vertical().max_height((ui.ctx().content_rect().height() - 150.0).max(80.0)).show(ui, |ui| super::drawing_controls::preset(ui, &mut prefs.presets)); });
            let eraser = icons::button(ui, Icon::Eraser, "Eraser · click for mode and size", editor.canvas.tool == Tool::Eraser);
            if eraser.clicked() { editor.canvas.select(Tool::Eraser); }
            egui::Popup::from_toggle_button_response(&eraser)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.strong("Eraser");
                    for mode in [EraserMode::Stroke, EraserMode::Partial] {
                        if ui.selectable_value(&mut prefs.eraser_mode, mode, mode.label()).changed() {
                            editor.canvas.finish(); editor.canvas.eraser_mode = mode;
                        }
                    }
                    super::drawing_controls::eraser_radius(ui, &mut editor.canvas.eraser_radius);
                });
            if width >= 500.0 { history(ui, &mut editor.canvas); }
            if width >= 800.0 { view(ui, doc, editor); }
            else { menu(ui, "View", |ui| { history(ui, &mut editor.canvas); view(ui, doc, editor); export(ui, action, editor.canvas.dirty()); }); }
            #[cfg(target_arch = "wasm32")]
            if width >= 1000.0 { export(ui, action, editor.canvas.dirty()); }
        });
    });
    editor.canvas.apply_pen(&prefs.presets.active().style);
}

fn history(ui: &mut egui::Ui, canvas: &mut Canvas) {
    for (redo, icon, label, enabled) in [
        (
            false,
            Icon::Undo,
            "Undo · Ctrl/Cmd+Z",
            canvas.drawing.can_undo(),
        ),
        (
            true,
            Icon::Redo,
            "Redo · Ctrl/Cmd+Shift+Z",
            canvas.drawing.can_redo(),
        ),
    ] {
        if ui
            .add_enabled_ui(enabled, |ui| icons::button(ui, icon, label, false))
            .inner
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
    fn new_preset_button_opens_editor_and_does_not_dismiss_itself() {
        use crate::ui::settings::tests::pointer;
        let data = crate::document::DocumentData::blank(DocumentKind::Whiteboard);
        let doc = Document {
            data: data.clone(),
            location: crate::document::Location::new("test.pdf".into()),
            external: false,
        };
        let mut editor = Editor::new(&data);
        let mut prefs = crate::settings::DrawingPreferences::default();
        let ctx = egui::Context::default();
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 600.0),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::Panel::top("tools").exact_size(40.0).show(ui, |ui| {
                        show(ui, Some((&doc, &mut editor)), &mut None, &mut prefs);
                    });
                    editor.show(ui, &data);
                },
            );
            output.textures_delta.clear();
            output.shapes
        };
        frame(vec![]);
        let shapes = frame(vec![]);
        fn plus(shape: &egui::Shape) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::LineSegment { points, .. }
                    if points[1].y == points[0].y
                        && (points[1].x - points[0].x - 9.0).abs() < 0.01 =>
                {
                    Some(points[0].lerp(points[1], 0.5))
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(plus),
                _ => None,
            }
        }
        fn open(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.text() == "Pen preset",
                egui::Shape::Vec(shapes) => shapes.iter().any(open),
                _ => false,
            }
        }
        let pos = shapes
            .iter()
            .find_map(|s| plus(&s.shape))
            .expect("New preset plus icon");
        frame(pointer(pos, true));
        frame(pointer(pos, false));
        assert!(
            frame(vec![]).iter().any(|s| open(&s.shape)),
            "New preset must leave its editor open"
        );
        drop(frame);
        assert_eq!(prefs.presets.items.len(), 6);
        assert!(!prefs.presets.active().builtin());
        assert!(!editor.canvas.dirty());
    }

    #[test]
    fn top_presets_select_in_one_click_without_ink_on_both_layouts() {
        use crate::ui::settings::tests::pointer;
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let data = crate::document::DocumentData::blank(kind);
            let doc = Document {
                data: data.clone(),
                location: crate::document::Location::new("test.pdf".into()),
                external: false,
            };
            let mut editor = Editor::new(&data);
            let mut prefs = crate::settings::DrawingPreferences::default();
            let ctx = egui::Context::default();
            let mut frame = |events, disabled| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1000.0, 600.0),
                        )),
                        focused: true,
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        if disabled {
                            ui.disable();
                        }
                        egui::Panel::top("tools").exact_size(40.0).show(ui, |ui| {
                            show(ui, Some((&doc, &mut editor)), &mut None, &mut prefs);
                        });
                        editor.show(ui, &data);
                    },
                );
                output.textures_delta.clear();
                output.shapes
            };
            frame(vec![], false);
            let shapes = frame(vec![], false);
            fn find(shape: &egui::Shape, color: egui::Color32) -> Option<egui::Pos2> {
                match shape {
                    egui::Shape::LineSegment { points, stroke } if stroke.color == color => {
                        Some(points[0].lerp(points[1], 0.5))
                    }
                    egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, color)),
                    _ => None,
                }
            }
            let yellow = egui::Color32::from_rgba_unmultiplied(255, 206, 38, 89);
            let pos = shapes
                .iter()
                .find_map(|s| find(&s.shape, yellow))
                .expect("Highlighter preview");
            assert!(pos.y < 40.0, "Drawing presets must stay in the top panel");
            frame(pointer(pos, true), false);
            frame(pointer(pos, false), false);
            drop(frame);
            assert_eq!(prefs.presets.selected, uuid::Uuid::from_u128(4));
            assert_eq!(editor.canvas.pen_type, crate::pen::PenType::Highlighter);
            assert_eq!(editor.canvas.color[3], 89);
            assert!(!editor.canvas.dirty(), "Preset controls must not start ink");
            let selected = prefs.presets.selected;
            let black = shapes
                .iter()
                .find_map(|s| find(&s.shape, egui::Color32::from_rgb(32, 39, 49)))
                .expect("Black preset icon");
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: pointer(black, true),
                    ..Default::default()
                },
                |ui| {
                    ui.disable();
                    show(ui, Some((&doc, &mut editor)), &mut None, &mut prefs);
                },
            );
            output.textures_delta.clear();
            assert_eq!(prefs.presets.selected, selected);
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
