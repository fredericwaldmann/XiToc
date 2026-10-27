use super::{
    Action,
    icons::{self, Icon},
    new_choices,
    style::{self, Theme},
};
use egui::{Align, Layout, RichText};

#[derive(Default)]
pub struct Chrome {
    pub theme: Theme,
    pub explorer_hidden: bool,
    details: bool,
}

impl Chrome {
    pub fn top(&mut self, ui: &mut egui::Ui) {
        let p = style::palette(ui);
        egui::Panel::top("application-bar")
            .resizable(false)
            .exact_size(38.0)
            .frame(style::frame(p.shell, 10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if icons::button(
                        ui,
                        Icon::Sidebar,
                        "Show or hide Explorer",
                        !self.explorer_hidden,
                    )
                    .clicked()
                    {
                        self.explorer_hidden = !self.explorer_hidden;
                    }
                    ui.label(RichText::new("XiToc").size(16.0).strong());
                    if ui.available_width() > 360.0 {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(if cfg!(target_arch = "wasm32") {
                                "Browser session"
                            } else {
                                "Local workspace"
                            })
                            .small()
                            .color(p.muted),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.menu_button("Appearance", |ui| {
                            let light = ui.selectable_value(&mut self.theme, Theme::Light, "Light");
                            let dark = ui.selectable_value(&mut self.theme, Theme::Dark, "Dark");
                            if light.changed() || dark.changed() {
                                ui.ctx().request_repaint();
                            }
                        });
                    });
                });
            });
    }

    pub fn bottom(&mut self, ui: &mut egui::Ui, status: &str, context: &str) {
        let p = style::palette(ui);
        egui::Panel::bottom("status-bar")
            .resizable(false)
            .exact_size(28.0)
            .frame(style::frame(p.shell, 10, 1))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .small_button("…")
                        .on_hover_text("Show full status message")
                        .clicked()
                    {
                        self.details = !self.details;
                    }
                    let right = if ui.available_width() > 560.0 {
                        170.0
                    } else {
                        0.0
                    };
                    let width = (ui.available_width() - right).max(20.0);
                    ui.add_sized(
                        [width, 22.0],
                        egui::Label::new(RichText::new(status).small().color(p.muted)).truncate(),
                    )
                    .on_hover_text(status);
                    if right > 0.0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(context).small().color(p.muted));
                        });
                    }
                });
            });
        if self.details {
            egui::Window::new("Status")
                .open(&mut self.details)
                .resizable(true)
                .default_width(360.0)
                .show(ui.ctx(), |ui| {
                    ui.label(status);
                });
        }
    }
}

pub fn empty(ui: &mut egui::Ui, can_create: bool, folder: &str, action: &mut Option<Action>) {
    let p = style::palette(ui);
    let height = ui.available_height();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space((height * 0.25).clamp(12.0, 140.0));
        ui.vertical_centered(|ui| {
            ui.add(egui::Label::new(RichText::new("No document open").size(20.0).strong()).wrap());
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    RichText::new("Create a Page Note or Whiteboard, or open an existing PDF.")
                        .color(p.muted),
                )
                .wrap(),
            );
            ui.add_space(16.0);
            if can_create {
                ui.menu_button("New Document", |ui| new_choices(ui, folder, action));
            }
            #[cfg(not(target_arch = "wasm32"))]
            if !can_create && ui.button("Open workspace…").clicked() {
                *action = Some(Action::Workspace);
            }
            #[cfg(target_arch = "wasm32")]
            if ui.button("Import PDF…").clicked() {
                *action = Some(Action::Import);
            }
            ui.add_space(12.0);
            ui.add(
                egui::Label::new(
                    RichText::new(if cfg!(target_arch = "wasm32") {
                        "Export PDFs to keep your work. Reimport them after reloading."
                    } else {
                        "Choose a folder to open or create your PDF documents."
                    })
                    .small()
                    .color(p.muted),
                )
                .wrap(),
            );
        });
    });
}
