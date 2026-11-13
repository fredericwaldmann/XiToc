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
    focus: bool,
    config: (
        crate::latex_config::LatexPreferences,
        crate::latex_config::LatexConfig,
    ),
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
        texture: Default::default(),
    };
    session.update_preview();
    session
}
impl FormulaSession {
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
        session.update_preview();
        let mut commit = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique(("formula-editor", session.id))).show(ui.ctx(), |ui| {
            ui.set_width((ui.ctx().content_rect().width() - 48.0).clamp(300.0, 560.0));
            ui.heading(if session.original.is_some() {
                "Edit Formula"
            } else {
                "New Formula"
            });
            ui.label("LaTeX math source");
            let editor = ui.add_sized(
                [ui.available_width(), 116.0],
                egui::TextEdit::multiline(&mut session.source)
                    .id(egui::Id::unique(("formula-source", session.id)))
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(5)
                    .hint_text(r"\frac{-b \pm \sqrt{b^2-4ac}}{2a}"),
            );
            if session.focus {
                editor.request_focus();
                session.focus = false;
            }
            if editor.changed() {
                session.update_preview();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
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
        let mut formula = source.clone();
        if !delta.is_identity() {
            formula = formula.transformed(delta);
        }
        if let Some(FormulaCache {
            rendered: Ok(rendered),
            texture,
            ..
        }) = cache.get_mut(&source.id)
        {
            let mut map = |p: Pos2| {
                let world = formula.placement.apply([p.x as f64, p.y as f64]);
                let screen = view.to_screen(world);
                Pos2::new(screen[0] as f32, screen[1] as f32)
            };
            texture.paint(
                painter,
                rendered,
                (formula.placement.scale * view.scale) as f32 * painter.ctx().pixels_per_point(),
                global.high_quality,
                &mut map,
            );
        } else if let Some(bounds) = formula.world_bounds() {
            let p = view.to_screen(bounds.min);
            painter.text(
                Pos2::new(p[0] as f32, p[1] as f32),
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
