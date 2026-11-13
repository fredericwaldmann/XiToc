//! Shared draft editors. Failed validation retains all text.
use crate::latex_config::{LatexConfig, LatexPreferences};

pub fn editors(ui: &mut egui::Ui, config: &mut LatexConfig, prefix: &str) {
    ui.label(format!("{prefix} Preamble"));
    ui.add(
        egui::TextEdit::multiline(&mut config.preamble)
            .code_editor()
            .desired_rows(5)
            .desired_width(f32::INFINITY),
    );
    ui.label(format!("{prefix} Postamble"));
    ui.add(
        egui::TextEdit::multiline(&mut config.postamble)
            .code_editor()
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );
    ui.weak(r"Math-only: \newcommand{\vect}[1]{\mathbf{#1}}, \renewcommand for custom commands, and math wrappers (e.g. \left( / \right)). Packages, full documents and file commands are unsupported.");
}
#[derive(Default)]
pub struct GlobalLatexDraft {
    value: Option<LatexPreferences>,
    error: Option<String>,
}
impl GlobalLatexDraft {
    pub fn show(&mut self, ui: &mut egui::Ui, prefs: &mut LatexPreferences) {
        let value = self.value.get_or_insert_with(|| prefs.clone());
        ui.strong("Rendering");
        egui::ComboBox::from_id_salt("math-preset")
            .selected_text("Computer Modern · OpenType MATH")
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut value.preset,
                    "computer-modern-math".into(),
                    "Computer Modern · OpenType MATH",
                );
            });
        if value.preset != "computer-modern-math" {
            ui.weak("Unavailable saved preset: using bundled Computer Modern.");
        }
        ui.horizontal(|ui| {
            ui.label("Default formula size (pt)");
            ui.add(egui::DragValue::new(&mut value.size).range(6.0..=144.0));
        });
        ui.label("Default formula color");
        super::shape_controls::color_control(ui, &mut value.color, 255);
        ui.checkbox(
            &mut value.high_quality,
            "High-quality rendering (supersampled outlines)",
        );
        editors(ui, &mut value.config, "Global");
        ui.horizontal(|ui| {
            if ui.button("Reset to Defaults").clicked() {
                *value = LatexPreferences::default();
                self.error = None;
            }
            if ui.button("Apply").clicked() {
                match value.validate() {
                    Ok(()) => {
                        *prefs = value.clone();
                        self.error = None;
                    }
                    Err(e) => self.error = Some(e),
                }
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        ui.weak("Defaults affect new formulas. Configuration applies to all documents. Document changes remain memory-only until M2.");
    }
}
#[derive(Default)]
pub struct DocumentLatexDialog {
    pub open: bool,
    draft: Option<LatexConfig>,
    error: Option<String>,
}
impl DocumentLatexDialog {
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        drawing: &mut crate::drawing::Drawing,
        global: &LatexPreferences,
    ) {
        if !self.open {
            self.draft = None;
            return;
        }
        let draft = self.draft.get_or_insert_with(|| drawing.latex.clone());
        let mut close = false;
        egui::Modal::new(egui::Id::unique(("document-latex", drawing.owner()))).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width()-48.).clamp(180., super::style::SETTINGS_WIDTH));
            ui.heading("Document Settings · LaTeX");
            ui.label("Global configuration: Enabled");
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-220.).max(60.)).show(ui, |ui| {
                editors(ui, draft, "Document");
                ui.weak("Order: Global preamble → Document preamble → Formula → Document postamble → Global postamble. Settings affect this document only; memory-only until M2.");
            });
            ui.horizontal(|ui| {
                if ui.button("Validate").clicked() { self.error = Some(match drawing.validate_latex_change(draft, &global.config) { Ok(()) => "Configuration is valid.".into(), Err(e) => e }); }
                if ui.button("Apply").clicked() { match drawing.set_latex(draft.clone(), &global.config) { Ok(()) => { self.error = None; close = true; }, Err(e) => self.error = Some(e) } }
                close |= ui.button("Cancel").clicked();
            });
            if let Some(e) = &self.error { ui.label(e); }
            close |= ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        });
        if close {
            self.open = false;
            self.draft = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::settings::tests::{pointer, render};
    #[test]
    fn global_apply_preserves_invalid_draft_and_only_commits_valid_input() {
        let ctx = egui::Context::default();
        let mut prefs = LatexPreferences::default();
        let mut draft = GlobalLatexDraft {
            value: Some(LatexPreferences {
                config: LatexConfig {
                    preamble: r"\usepackage{missing}".into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut frame = |events| render(&ctx, events, |ui| draft.show(ui, &mut prefs));
        let labels = frame(vec![]);
        let pos = labels["Apply"];
        frame(pointer(pos, true));
        frame(pointer(pos, false));
        assert_eq!(prefs, LatexPreferences::default());
        assert!(draft.error.is_some());
        assert_eq!(
            draft.value.as_ref().unwrap().config.preamble,
            r"\usepackage{missing}"
        );
        draft.value.as_mut().unwrap().config.preamble = "\\newcommand{\\R}{\\mathbb{R}}\n".into();
        let mut frame = |events| render(&ctx, events, |ui| draft.show(ui, &mut prefs));
        let pos = frame(vec![])["Apply"];
        frame(pointer(pos, true));
        frame(pointer(pos, false));
        assert!(draft.error.is_none());
        assert_eq!(prefs.config.preamble, "\\newcommand{\\R}{\\mathbb{R}}\n");
    }
}
