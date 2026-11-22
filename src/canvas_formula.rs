//! Formula source editing and cached vector painting on the shared canvas.
use super::*;
use crate::{formula::FormulaObject, formula_render::RenderedFormula};

pub(super) struct FormulaSession {
    pub(super) id: Uuid,
    surface: Surface,
    position: [f64; 2],
    original: Option<FormulaObject>,
    source: String,
    color: [u8; 4],
    preview: Option<RenderedFormula>,
    attempted: Option<(String, [u8; 4])>,
    error: Option<String>,
    pub(super) focus: bool,
    config: (
        crate::latex_config::LatexPreferences,
        crate::latex_config::LatexConfig,
    ),
    view: Option<crate::latex_config::ConfigurationView>,
    cursor: Option<egui::text::CCursorRange>,
    texture: crate::formula_render::FormulaTexture,
}
pub(super) struct FormulaCache {
    source: String,
    color: [u8; 4],
    config: (
        crate::latex_config::LatexPreferences,
        crate::latex_config::LatexConfig,
    ),
    rendered: Result<RenderedFormula, String>,
    texture: crate::formula_render::FormulaTexture,
}

fn make_session(
    surface: Surface,
    position: [f64; 2],
    original: Option<FormulaObject>,
    global: &crate::latex_config::LatexPreferences,
    doc: &crate::latex_config::LatexConfig,
) -> FormulaSession {
    let (id, source, color) = original.as_ref().map_or_else(
        || {
            (
                Uuid::new_v4(),
                String::new(),
                [global.color[0], global.color[1], global.color[2], 255],
            )
        },
        |formula| (formula.id, formula.latex_source.clone(), formula.color),
    );
    let mut session = FormulaSession {
        id,
        surface,
        position,
        original,
        source,
        color,
        preview: None,
        attempted: None,
        error: None,
        focus: true,
        config: (global.clone(), doc.clone()),
        view: None,
        cursor: None,
        texture: Default::default(),
    };
    session.update_preview();
    session
}
impl FormulaSession {
    fn refresh_config(
        &mut self,
        global: &crate::latex_config::LatexConfig,
        doc: &crate::latex_config::LatexConfig,
    ) {
        if self.config.0.config != *global || self.config.1 != *doc {
            self.config.0.config = global.clone();
            self.config.1 = doc.clone();
            self.attempted = None;
            self.view = None;
        }
        if self.view.is_none() {
            self.view = Some(crate::latex_config::inspect(global, doc));
        }
    }

    fn update_preview(&mut self) {
        if self.attempted.as_ref() == Some(&(self.source.clone(), self.color)) {
            return;
        }
        self.attempted = Some((self.source.clone(), self.color));
        match crate::latex_config::assemble(&self.config.0.config, &self.config.1, &self.source)
            .and_then(|source| crate::formula_render::render(&source, self.color))
        {
            Ok(rendered) => {
                self.texture = Default::default();
                self.preview = Some(rendered);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
}

impl Canvas {
    pub(crate) fn edit_formula(&mut self, id: Uuid) {
        let Some(ObjectSnapshot::Formula(formula)) = self.drawing.snapshot(id) else {
            return;
        };
        let formula = (**formula).clone();
        if self.drawing.is_locked(id) {
            return;
        }
        self.formula_session = Some(make_session(
            formula.surface,
            formula.placement.position,
            Some(formula),
            &crate::latex_config::LatexPreferences {
                config: self.drawing.latex_global.clone(),
                ..self.latex.clone()
            },
            &self.drawing.latex,
        ));
    }

    pub(crate) fn set_formula_color(&mut self, id: Uuid, rgb: [u8; 3]) -> bool {
        let Some(ObjectSnapshot::Formula(formula)) = self.drawing.snapshot(id) else {
            return false;
        };
        let mut formula = (**formula).clone();
        if self.drawing.is_locked(id) || formula.color[..3] == rgb {
            return false;
        }
        formula.color[..3].copy_from_slice(&rgb);
        if !self.drawing.replace_formulas(vec![formula], true) {
            return false;
        }
        self.selection.refresh(&self.drawing);
        true
    }

    pub(crate) fn reset_formula_scale(&mut self, id: Uuid) -> bool {
        let Some(ObjectSnapshot::Formula(formula)) = self.drawing.snapshot(id) else {
            return false;
        };
        let mut formula = (**formula).clone();
        if self.drawing.is_locked(id) || formula.placement.scale == 1.0 {
            return false;
        }
        formula.placement.scale = 1.0;
        if !self.drawing.replace_formulas(vec![formula], false) {
            return false;
        }
        self.selection.refresh(&self.drawing);
        true
    }

    pub(super) fn formula_surface(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) -> bool {
        if self.formula_session.is_none() && self.tool == Tool::Formula {
            for event in ui.input(|i| i.events.clone()) {
                if let Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    ..
                } = event
                    && bounds.intersect(ui.clip_rect()).contains(pos)
                    && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
                    && self
                        .drawing
                        .contains(surface, view.to_document([pos.x as f64, pos.y as f64]))
                {
                    self.formula_session = Some(make_session(
                        surface,
                        view.to_document([pos.x as f64, pos.y as f64]),
                        None,
                        &crate::latex_config::LatexPreferences {
                            config: self.drawing.latex_global.clone(),
                            ..self.latex.clone()
                        },
                        &self.drawing.latex,
                    ));
                    self.tool = self.selection_mode;
                    break;
                }
            }
        }
        if self.formula_session.is_none()
            && matches!(self.tool, Tool::Select | Tool::Lasso)
            && ui.input(|i| i.pointer.button_double_clicked(PointerButton::Primary))
            && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
            && bounds.intersect(ui.clip_rect()).contains(pos)
            && let Some(id) =
                self.pick(surface, view.to_document([pos.x as f64, pos.y as f64]), 0.0)
            && !self.drawing.is_locked(id)
            && let Some(ObjectSnapshot::Formula(formula)) = self.drawing.snapshot(id)
        {
            self.formula_session = Some(make_session(
                surface,
                formula.placement.position,
                Some((**formula).clone()),
                &crate::latex_config::LatexPreferences {
                    config: self.drawing.latex_global.clone(),
                    ..self.latex.clone()
                },
                &self.drawing.latex,
            ));
        }
        if self.formula_session.is_some() {
            self.show_formula_editor(ui);
            return true;
        }
        false
    }

    fn show_formula_editor(&mut self, ui: &egui::Ui) {
        let Some(mut session) = self.formula_session.take() else {
            return;
        };
        session.refresh_config(&self.drawing.latex_global, &self.drawing.latex);
        session.update_preview();
        if self.latex_dialog.open || self.formula_settings_suspended {
            session.focus = true;
            self.formula_session = Some(session);
            return;
        }
        let popup_was_open = egui::Popup::is_any_open(ui.ctx());
        let mut commit = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique(("formula-editor", session.id))).show(ui.ctx(), |ui| {
            ui.set_width((ui.ctx().content_rect().width() - 48.0).clamp(300.0, 560.0));
            ui.heading(if session.original.is_some() {
                "Edit Formula"
            } else {
                "New Formula"
            });
            let view = session.view.as_ref().unwrap();
            let label = if view.error.is_some() { egui::RichText::new(view.label).small().color(ui.visuals().error_fg_color) } else { egui::RichText::new(view.label).small() };
            ui.menu_button(label, |ui| {
                ui.set_max_width(440.0);
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for (title, text) in [
                        ("Active global preamble", &session.config.0.config.preamble),
                        ("Active global postamble", &session.config.0.config.postamble),
                        ("Active document preamble", &session.config.1.preamble),
                        ("Active document postamble", &session.config.1.postamble),
                        ("Effective preamble (definitions removed)", &view.preamble),
                        ("Effective postamble (definitions removed)", &view.postamble),
                    ] {
                        ui.strong(title);
                        ui.add(egui::Label::new(egui::RichText::new(if text.trim().is_empty() { "(empty)" } else { text }).monospace()).wrap());
                    }
                    ui.weak("Wrapper order: Global → Document → source → Document → Global. Macros resolve in that same order.");
                    ui.strong("Available custom commands");
                    if view.macros.is_empty() { ui.weak("None"); }
                    for m in &view.macros {
                        ui.label(format!("{}[{}] = {} · {} · {}", m.name, m.args, m.body, m.source, if m.shadowed { "shadowed" } else { "effective" }));
                    }
                    if let Some(error) = &view.error { ui.colored_label(ui.visuals().error_fg_color, error); ui.weak("Commands unavailable until configuration is valid. Check both settings sources below."); }
                });
                ui.separator();
                if ui.button("Open Global LaTeX Settings").clicked() { self.open_global_latex = true; ui.close(); }
                if ui.button("Open Document LaTeX Settings").clicked() { self.latex_dialog.open = true; ui.close(); }
                if ui.button("Copy Effective Configuration").clicked() { ui.ctx().copy_text(view.copy_text(&session.config.0.config, &session.config.1)); }
            });
            ui.label("LaTeX math source");
            let source_id = egui::Id::unique(("formula-source", session.id));
            if let Some(cursor) = session.cursor
                && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), source_id) { state.cursor.set_char_range(Some(cursor)); state.store(ui.ctx(), source_id); }
            let editor = ui.add_sized(
                [ui.available_width(), 116.0],
                egui::TextEdit::multiline(&mut session.source)
                    .id(source_id)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(5)
                    .hint_text(r"\frac{-b \pm \sqrt{b^2-4ac}}{2a}"),
            );
            if session.focus {
                editor.request_focus();
                session.focus = false;
            }
            if editor.has_focus() { session.cursor = egui::TextEdit::load_state(ui.ctx(), source_id).and_then(|state| state.cursor.char_range()); }
            if editor.changed() {
                session.update_preview();
            }
            if !popup_was_open && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                cancel = true;
            }
            if editor.has_focus()
                && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter))
            {
                commit = true;
            }
            ui.add_space(6.0);
            ui.strong("Preview");
            let (preview_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 128.0),
                egui::Sense::hover(),
            );
            ui.painter()
                .rect_filled(preview_rect, 3.0, ui.visuals().extreme_bg_color);
            if let Some(preview) = &session.preview {
                let fit = (preview_rect.width() / preview.width.max(1.0))
                    .min((preview_rect.height() - 12.0) / preview.height.max(1.0))
                    .min(1.0);
                let center = preview_rect.center();
                let w = preview.width * fit;
                let h = preview.height * fit;
                let map = |p: Pos2| {
                    Pos2::new(
                        center.x + (p.x - preview.width * 0.5) * fit,
                        center.y + (p.y - preview.height * 0.5) * fit,
                    )
                };
                let mut map = map;
                session.texture.paint(
                    ui.painter(),
                    preview,
                    fit * ui.ctx().pixels_per_point(),
                    session.config.0.high_quality,
                    &mut map,
                );
                let _ = (w, h);
            } else {
                ui.painter().text(
                    preview_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Formula preview appears here",
                    egui::FontId::proportional(13.0),
                    ui.visuals().weak_text_color(),
                );
            }
            if let Some(error) = &session.error {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("Invalid formula · {error}"),
                );
                if session.preview.is_some() {
                    ui.weak("Showing the last valid preview. Correct the source before finishing.");
                }
            } else {
                ui.weak("Ctrl/Cmd+Enter to finish · Escape to cancel");
            }
            ui.horizontal(|ui| {
                commit |= ui
                    .add_enabled(session.error.is_none(), egui::Button::new("Finish Formula"))
                    .clicked();
                cancel |= ui.button("Cancel").clicked();
            });
        });
        if cancel {
            self.formula_session = None;
            return;
        }
        if commit && session.error.is_none() {
            if let Some(rendered) = session.preview.take() {
                let was_editing = session.original.is_some();
                let mut formula = session.original.clone().unwrap_or_else(|| {
                    FormulaObject::new(
                        self.drawing.owner(),
                        session.surface,
                        session.position,
                        session.source.clone(),
                        session.color,
                    )
                });
                if !was_editing {
                    formula.placement.scale =
                        session.config.0.size / crate::formula::FORMULA_FONT_SIZE;
                }
                formula.latex_source = session.source.clone();
                formula.color = session.color;
                formula.layout = crate::formula::FormulaLayout {
                    width: rendered.width as f64,
                    height: rendered.height as f64,
                    baseline: rendered.baseline,
                };
                let id = formula.id;
                let changed = if was_editing {
                    self.drawing.replace_formulas(vec![formula], false)
                } else {
                    self.drawing.add_formula(formula)
                };
                if changed {
                    self.formula_cache.insert(
                        id,
                        FormulaCache {
                            source: session.attempted.map(|v| v.0).unwrap_or_default(),
                            color: session.color,
                            config: session.config.clone(),
                            rendered: Ok(rendered),
                            texture: session.texture,
                        },
                    );
                    self.selection.scope(session.surface);
                    self.selection.change(
                        [id],
                        crate::selection::Membership::Replace,
                        &self.drawing,
                    );
                    self.tool = self.selection_mode;
                } else {
                    self.notice = Some("Formula could not be placed on this surface.".into());
                    self.formula_session = Some(session);
                }
            }
        } else {
            self.formula_session = Some(session);
        }
    }

    pub(super) fn paint_formula(
        cache: &mut HashMap<Uuid, FormulaCache>,
        painter: &egui::Painter,
        view: Transform,
        source: &FormulaObject,
        delta: crate::object::Placement,
        global: &crate::latex_config::LatexPreferences,
        doc: &crate::latex_config::LatexConfig,
    ) {
        let transformed;
        let formula = if delta.is_identity() {
            source
        } else {
            transformed = source.transformed(delta);
            &transformed
        };
        let Some(bounds) = formula.world_bounds() else {
            return;
        };
        let screen = |p| {
            let [x, y] = view.to_screen(p);
            Pos2::new(x as f32, y as f32)
        };
        if !painter
            .clip_rect()
            .intersects(Rect::from_two_pos(screen(bounds.min), screen(bounds.max)))
        {
            return;
        }
        let cached = cache.get(&source.id);
        if cached.is_none_or(|cache| {
            cache.source != source.latex_source
                || cache.color != source.color
                || cache.config != (global.clone(), doc.clone())
        }) {
            let result = crate::latex_config::assemble(&global.config, doc, &source.latex_source)
                .and_then(|s| crate::formula_render::render(&s, source.color));
            cache.insert(
                source.id,
                FormulaCache {
                    source: source.latex_source.clone(),
                    color: source.color,
                    config: (global.clone(), doc.clone()),
                    rendered: result,
                    texture: Default::default(),
                },
            );
        }
        if let Some(FormulaCache {
            rendered: Ok(rendered),
            texture,
            ..
        }) = cache.get_mut(&source.id)
        {
            let mut map = |p: Pos2| screen(formula.placement.apply([p.x as f64, p.y as f64]));
            texture.paint(
                painter,
                rendered,
                (formula.placement.scale * view.scale) as f32 * painter.ctx().pixels_per_point(),
                global.high_quality,
                &mut map,
            );
        } else {
            painter.text(
                screen(bounds.min),
                egui::Align2::LEFT_TOP,
                "!",
                egui::FontId::proportional(13.0),
                ui_error_color(painter),
            );
        }
    }
}

fn ui_error_color(_painter: &egui::Painter) -> egui::Color32 {
    egui::Color32::from_rgb(196, 64, 64)
}

#[cfg(test)]
mod configuration_tests {
    use super::*;
    #[test]
    fn opening_configuration_popover_does_not_edit_or_add_history() {
        use crate::ui::settings::tests::{pointer, render};
        let data = crate::document::DocumentData::blank(crate::document::DocumentKind::Whiteboard);
        let mut canvas = Canvas::new(&data);
        canvas.formula_session = Some(make_session(
            Surface::Canvas,
            [0., 0.],
            None,
            &Default::default(),
            &Default::default(),
        ));
        canvas.formula_session.as_mut().unwrap().source = "x + y".into();
        let revision = canvas.drawing.revision();
        let ctx = egui::Context::default();
        let mut frame = |events| render(&ctx, events, |ui| canvas.show_formula_editor(ui));
        frame(vec![]);
        let labels = frame(vec![]);
        let pos = labels["Default"];
        frame(pointer(pos, true));
        frame(pointer(pos, false));
        let labels = frame(vec![]);
        assert!(
            labels.contains_key("Open Global LaTeX Settings"),
            "{labels:?}"
        );
        assert_eq!(canvas.formula_session.as_ref().unwrap().source, "x + y");
        assert_eq!(canvas.drawing.revision(), revision);
        assert!(canvas.drawing.undo().is_empty());
        canvas.formula_settings_suspended = true;
        render(&ctx, vec![], |ui| {
            ui.disable();
            canvas.begin_frame(ui);
        });
        assert_eq!(canvas.formula_session.as_ref().unwrap().source, "x + y");
        canvas.formula_settings_suspended = false;
        canvas.latex_dialog.open = true;
        render(&ctx, vec![], |ui| canvas.show_formula_editor(ui));
        assert_eq!(canvas.formula_session.as_ref().unwrap().source, "x + y");
        assert_eq!(canvas.drawing.revision(), revision);
    }
    #[test]
    fn live_config_preserves_source_cursor_and_unchanged_preview() {
        let global = crate::latex_config::LatexPreferences::default();
        let empty = crate::latex_config::LatexConfig::default();
        let mut session = make_session(Surface::Canvas, [0., 0.], None, &global, &empty);
        session.source = "x + y".into();
        session.cursor = Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(1),
            egui::text::CCursor::new(4),
        ));
        session.update_preview();
        let cursor = session.cursor;
        let attempted = session.attempted.clone();
        session.refresh_config(&global.config, &empty);
        assert_eq!(session.attempted, attempted);
        let doc = crate::latex_config::LatexConfig {
            preamble: r"\newcommand{\R}{\mathbb{R}}".into(),
            ..Default::default()
        };
        session.refresh_config(&global.config, &doc);
        assert!(session.attempted.is_none());
        session.update_preview();
        assert!(session.error.is_none());
        assert_eq!(session.view.as_ref().unwrap().label, "Document");
        session.refresh_config(&global.config, &empty);
        assert!(session.view.as_ref().unwrap().macros.is_empty());
        assert_eq!(session.source, "x + y");
        assert_eq!(session.cursor, cursor);
        session.refresh_config(
            &global.config,
            &crate::latex_config::LatexConfig {
                preamble: r"\unsupportedcommand".into(),
                ..Default::default()
            },
        );
        session.update_preview();
        assert!(session.error.is_some());
        assert!(session.preview.is_some());
        assert_eq!(session.source, "x + y");
    }
}
