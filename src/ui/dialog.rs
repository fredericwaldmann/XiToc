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
        let title = format!("New {}", self.kind.label());
        let window_closed = configuration(
            ctx,
            egui::Id::unique("create-document"),
            &title,
            [360., 270.],
            |ui| {
                ui.set_width((ui.available_width()).clamp(160., 320.));
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
            },
        );
        if cancel || window_closed {
            Outcome::Cancel
        } else if create {
            Outcome::Create
        } else {
            Outcome::Pending
        }
    }
}

/// Render a dialog against its existing draft. Immediate viewports let the
/// callback borrow that draft and return before the caller commits changes.
pub fn configuration(
    ctx: &egui::Context,
    id: egui::Id,
    title: &str,
    size: [f32; 2],
    mut content: impl FnMut(&mut egui::Ui),
) -> bool {
    if !ctx.embed_viewports() {
        return ctx.show_viewport_immediate(
            egui::ViewportId(id),
            egui::ViewportBuilder::default()
                .with_title(title)
                .with_app_id("XiToc")
                .with_taskbar(false)
                .with_inner_size(size)
                .with_clamp_size_to_monitor_size(true)
                .with_min_inner_size(if size[0] >= 1000. {
                    [690., 320.]
                } else {
                    [size[0].min(480.), 260.]
                })
                .with_decorations(true)
                .with_resizable(true),
            |ui, class| {
                if class == egui::ViewportClass::EmbeddedWindow {
                    dialog_content(ui, &mut content);
                    return false;
                }
                if ui.input(|input| input.viewport().close_requested()) {
                    return true;
                }
                egui::CentralPanel::default()
                    .frame(dialog_frame(ui))
                    .show(ui, |ui| dialog_content(ui, &mut content));
                false
            },
        );
    }
    embedded(ctx, id, &mut content)
}

fn dialog_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .inner_margin(12)
        .fill(super::style::palette(ui).surface)
}

fn dialog_content(ui: &mut egui::Ui, content: &mut impl FnMut(&mut egui::Ui)) {
    ui.scope(|ui| {
        let style = ui.style_mut();
        style.spacing.interact_size.y = 24.;
        style.spacing.button_padding = egui::vec2(8., 3.);
        for widgets in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
        ] {
            widgets.corner_radius = 5.into();
        }
        content(ui);
    });
}

fn embedded(ctx: &egui::Context, id: egui::Id, content: &mut impl FnMut(&mut egui::Ui)) -> bool {
    egui::Modal::new(id)
        .show(ctx, |ui| dialog_content(ui, content))
        .backdrop_response
        .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn native_close_is_delivered_to_the_dialog_without_rendering_its_draft() {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        let close = Rc::new(Cell::new(false));
        let close_event = close.clone();
        let id = egui::Id::unique("configuration-host-test");
        egui::Context::set_immediate_viewport_renderer(move |ctx, mut viewport| {
            assert_eq!(viewport.ids.parent, egui::ViewportId::ROOT);
            assert_eq!(viewport.ids.this, egui::ViewportId(id));
            assert_eq!(
                viewport.builder.title.as_deref(),
                Some("Test configuration")
            );
            assert_eq!(viewport.builder.decorations, Some(true));
            let mut input = egui::RawInput {
                viewport_id: viewport.ids.this,
                ..Default::default()
            };
            let info = input.viewports.entry(viewport.ids.this).or_default();
            if close_event.get() {
                info.events.push(egui::ViewportEvent::Close);
            }
            ctx.run_ui(input, |ui| (viewport.viewport_ui_cb)(ui))
                .textures_delta
                .clear();
        });

        for closing in [false, true] {
            close.set(closing);
            let mut rendered = false;
            ctx.run_ui(egui::RawInput::default(), |ui| {
                assert_eq!(
                    configuration(ui.ctx(), id, "Test configuration", [500., 380.], |ui| {
                        assert_eq!(ui.ctx().viewport_id(), egui::ViewportId(id));
                        rendered = true;
                    }),
                    closing
                );
            })
            .textures_delta
            .clear();
            assert_eq!(rendered, !closing);
        }
    }
}
