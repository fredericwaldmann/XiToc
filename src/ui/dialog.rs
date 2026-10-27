use crate::document::{DocumentKind, pdf_filename};

pub struct Creation {
    pub kind: DocumentKind,
    pub name: String,
    pub folder: String,
    pub error: Option<String>,
    pub focus: bool,
}

pub enum Outcome {
    Pending,
    Create,
    Cancel,
}
impl Creation {
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut create = false;
        let mut cancel = false;
        egui::Window::new(format!("New {}", self.kind.label()))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .default_width((ctx.content_rect().width() - 40.0).clamp(160.0, 320.0))
            .max_width((ctx.content_rect().width() - 40.0).max(160.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                #[cfg(not(target_arch = "wasm32"))]
                ui.label(format!(
                    "Folder: {}",
                    if self.folder.is_empty() {
                        "workspace root"
                    } else {
                        &self.folder
                    }
                ));
                #[cfg(target_arch = "wasm32")]
                ui.label(
                    "Browser session · export the PDF to choose where to keep it after reload.",
                );
                ui.label("Filename");
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.name).desired_width(ui.available_width()),
                );
                if self.focus {
                    field.request_focus();
                    self.focus = false;
                }
                if field.changed() {
                    self.error = None;
                }
                let valid = pdf_filename(&self.name);
                if let Err(error) = &valid {
                    ui.weak(error);
                }
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                create = valid.is_ok()
                    && field.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                ui.horizontal_wrapped(|ui| {
                    create |= ui
                        .add_enabled(
                            valid.is_ok(),
                            egui::Button::new(if cfg!(target_arch = "wasm32") {
                                "Create & export PDF"
                            } else {
                                "Create"
                            }),
                        )
                        .clicked();
                    cancel |= ui.button("Cancel").clicked();
                });
            });
        if cancel {
            Outcome::Cancel
        } else if create {
            Outcome::Create
        } else {
            Outcome::Pending
        }
    }
}
