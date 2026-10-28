//! Settings presentation; persistence stays in the app/framework boundary.
use super::style;
use crate::settings::{Preferences, Theme};

#[derive(Clone, Copy, Default, PartialEq)]
enum Section {
    #[default]
    General,
    Appearance,
    Drawing,
    Input,
}
#[derive(Default)]
pub struct SettingsDialog {
    pub open: bool,
    section: Section,
}
impl SettingsDialog {
    pub fn show(&mut self, ctx: &egui::Context, prefs: &mut Preferences, warning: Option<&str>) {
        if !self.open {
            return;
        }
        let mut close = false;
        egui::Modal::new(egui::Id::unique("application-settings")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 48.0).clamp(180.0, style::SETTINGS_WIDTH));
            ui.heading("Settings");
            ui.weak("Application preferences · changes apply automatically");
            ui.add_space(style::GAP);
            ui.horizontal_wrapped(|ui| {
                for (section, label) in [(Section::General, "General"), (Section::Appearance, "Appearance"),
                    (Section::Drawing, "Drawing"), (Section::Input, "Input Devices")] {
                    ui.selectable_value(&mut self.section, section, label);
                }
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 240.0).max(60.0))
                .show(ui, |ui| {
                    ui.add_space(style::GAP);
                    match self.section {
                        Section::General => {
                            ui.strong("Editor behavior");
                            ui.checkbox(&mut prefs.explorer_visible, "Show Explorer");
                            ui.weak("Preferences are stored locally on this device.");
                        }
                        Section::Appearance => {
                            ui.strong("Theme");
                            ui.horizontal(|ui| {
                                ui.selectable_value(&mut prefs.theme, Theme::Light, "Light");
                                ui.selectable_value(&mut prefs.theme, Theme::Dark, "Dark");
                            });
                        }
                        Section::Drawing => {
                            ui.strong("Default ink tools");
                            ui.label("Used for newly created or opened tabs.");
                            super::drawing_controls::show(ui, &mut prefs.drawing.color,
                                &mut prefs.drawing.width, &mut prefs.drawing.eraser_radius);
                            ui.add_space(style::GAP);
                            ui.strong("Stroke Smoothing");
                            ui.label("Applies to new strokes in every open tab.");
                            ui.spacing_mut().slider_width = style::SMOOTHING_SLIDER_WIDTH.min(ui.available_width() - 70.0);
                            ui.scope(|ui| {
                                let p = style::palette(ui);
                                ui.visuals_mut().widgets.inactive.bg_fill = p.border;
                                ui.add(egui::Slider::new(&mut prefs.drawing.smoothing_strength, 0..=100)
                                    .suffix("%")
                                    .trailing_fill(true)
                                    .text("Strength"));
                            });
                            ui.weak("0% Off · 25% Light · 50% Medium · 75% Strong · 100% Maximum");
                            ui.weak("Partial-erasing settings will arrive with that feature.");
                        }
                        Section::Input => {
                            ui.strong("Navigation");
                            ui.checkbox(&mut prefs.middle_button_pan, "Middle-button pan on Whiteboards");
                            ui.label("Hand and Space-drag also pan. Ctrl/Cmd-wheel or pinch zooms.");
                            ui.add_space(style::GAP);
                            ui.weak("Pressure-sensitive pen settings will arrive with pressure input support.");
                        }
                    }
                    ui.add_space(style::GAP);
                });
            ui.separator();
            if let Some(warning) = warning {
                ui.colored_label(ui.visuals().error_fg_color, warning);
            }
            close = ui.button("Done").clicked();
            close |= ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        });
        if close {
            self.open = false;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    pub fn render(
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        content: impl FnMut(&mut egui::Ui),
    ) -> std::collections::HashMap<String, egui::Pos2> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            content,
        );
        output.textures_delta.clear();
        let mut labels = std::collections::HashMap::new();
        fn collect(
            shape: &egui::Shape,
            labels: &mut std::collections::HashMap<String, egui::Pos2>,
        ) {
            match shape {
                egui::Shape::Text(text) => {
                    labels.insert(
                        text.galley.text().into(),
                        text.pos + text.galley.rect.center().to_vec2(),
                    );
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, labels);
                    }
                }
                _ => {}
            }
        }
        for shape in output.shapes {
            collect(&shape.shape, &mut labels);
        }
        labels
    }
    pub fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
}
