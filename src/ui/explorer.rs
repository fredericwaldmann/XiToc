use super::{
    Action,
    icons::{self, Icon},
    new_choices, style,
};
use crate::{document::DocumentKind, storage::Folder};
use egui::{RichText, Sense};

pub struct Explorer<'a> {
    pub tree: &'a Folder,
    pub selected_folder: &'a mut String,
    pub active_path: Option<&'a str>,
    pub can_create: bool,
    pub status: &'a mut String,
}

impl Explorer<'_> {
    pub fn show(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = style::palette(ui);
        ui.horizontal(|ui| {
            let title = if ui.available_width() < 170.0 {
                "Files"
            } else {
                "EXPLORER"
            };
            ui.label(RichText::new(title).small().strong().color(p.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(self.can_create, |ui| {
                    ui.menu_button("+ New", |ui| new_choices(ui, self.selected_folder, action));
                });
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                if ui.available_width() < 185.0 {
                    ui.menu_button("Workspace…", |ui| {
                        if ui.button("Open folder…").clicked() {
                            *action = Some(Action::Workspace);
                            ui.close();
                        }
                        if ui
                            .add_enabled(self.can_create, egui::Button::new("Refresh"))
                            .clicked()
                        {
                            *action = Some(Action::Refresh);
                            ui.close();
                        }
                    });
                } else {
                    if ui
                        .button("Open folder…")
                        .on_hover_text("Open a real workspace directory")
                        .clicked()
                    {
                        *action = Some(Action::Workspace);
                    }
                    if ui
                        .add_enabled(
                            self.can_create,
                            egui::Button::new("Refresh").frame_when_inactive(false),
                        )
                        .clicked()
                    {
                        *action = Some(Action::Refresh);
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            if ui.button("Import PDF…").clicked() {
                *action = Some(Action::Import);
            }
        });
        if cfg!(target_arch = "wasm32") {
            ui.label(
                RichText::new("Session files · export to keep")
                    .small()
                    .color(p.muted),
            )
            .on_hover_text("No directory access. Reimport your exported PDFs after reloading.");
        } else if !self.can_create {
            ui.add(
                egui::Label::new(
                    RichText::new("Choose a folder to start.")
                        .small()
                        .color(p.muted),
                )
                .wrap(),
            );
        } else {
            let folder = if self.selected_folder.is_empty() {
                "workspace root"
            } else {
                self.selected_folder.as_str()
            };
            ui.add(
                egui::Label::new(
                    RichText::new(format!("New in: {folder}"))
                        .small()
                        .color(p.muted),
                )
                .truncate(),
            )
            .on_hover_text(folder);
        }
        ui.add_space(10.0);
        egui::ScrollArea::both()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width().min(120.0));
                for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
                    let mut clicked = false;
                    let mut header =
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            ui.make_persistent_id(("category", kind.label())),
                            true,
                        )
                        .show_header(ui, |ui| {
                            icons::show(ui, kind.into(), p.accent);
                            clicked = ui
                                .selectable_label(
                                    false,
                                    RichText::new(match kind {
                                        DocumentKind::Page => "Pages",
                                        DocumentKind::Whiteboard => "Whiteboards",
                                    })
                                    .strong(),
                                )
                                .clicked();
                        });
                    if clicked {
                        header.toggle();
                    }
                    header.body(|ui| {
                        if self.tree.name.is_empty() {
                            ui.horizontal(|ui| {
                                icons::show(ui, kind.into(), p.muted);
                                ui.label(RichText::new("No documents yet").small().color(p.muted));
                            });
                        } else {
                            self.folder(ui, self.tree, kind, action);
                        }
                    });
                }
                problems(ui, self.tree, self.status);
            });
        ui.allocate_response(ui.available_size().max(egui::Vec2::ZERO), Sense::click())
            .context_menu(|ui| {
                ui.add_enabled_ui(self.can_create, |ui| {
                    new_choices(ui, self.selected_folder, action)
                });
            });
    }

    fn folder(
        &mut self,
        ui: &mut egui::Ui,
        folder: &Folder,
        kind: DocumentKind,
        action: &mut Option<Action>,
    ) {
        let p = style::palette(ui);
        let selected = *self.selected_folder == folder.path;
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            ui.make_persistent_id((kind.label(), &folder.path)),
            true,
        )
        .show_header(ui, |ui| {
            icons::show(ui, Icon::Folder, if selected { p.accent } else { p.muted });
            let response = ui.selectable_label(selected, &folder.name);
            if response.clicked() {
                *self.selected_folder = folder.path.clone();
            }
            response.context_menu(|ui| {
                *self.selected_folder = folder.path.clone();
                ui.label(if folder.path.is_empty() {
                    &folder.name
                } else {
                    &folder.path
                });
                ui.add_enabled_ui(self.can_create, |ui| new_choices(ui, &folder.path, action));
            });
        })
        .body(|ui| {
            for child in &folder.folders {
                self.folder(ui, child, kind, action);
            }
            for file in folder.files.iter().filter(|file| file.kind == Some(kind)) {
                ui.push_id((&file.path, file.document_id), |ui| {
                    ui.horizontal(|ui| {
                        let active = self.active_path == Some(file.path.as_str());
                        icons::show(ui, kind.into(), if active { p.accent } else { p.muted });
                        let title = file.path.rsplit('/').next().unwrap_or(&file.path);
                        let response = ui
                            .add_sized(
                                [ui.available_width().max(80.0), style::ROW],
                                egui::Button::new(title)
                                    .selected(active)
                                    .frame_when_inactive(active)
                                    .truncate(),
                            )
                            .on_hover_text(&file.path);
                        if response.clicked() {
                            *self.selected_folder = folder.path.clone();
                            *action = Some(Action::Open(file.path.clone()));
                        }
                        response.context_menu(|ui| {
                            ui.add_enabled_ui(self.can_create, |ui| {
                                new_choices(ui, &folder.path, action)
                            });
                        });
                    });
                });
            }
        });
    }
}

fn problems(ui: &mut egui::Ui, folder: &Folder, status: &mut String) {
    for file in folder.files.iter().filter(|file| file.problem.is_some()) {
        if ui
            .button(format!("! {}", file.path))
            .on_hover_text("Unreadable PDF; original unchanged")
            .clicked()
        {
            *status = file.problem.clone().unwrap_or_default();
        }
    }
    for child in &folder.folders {
        problems(ui, child, status);
    }
}
