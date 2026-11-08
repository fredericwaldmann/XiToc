//! Text input/display adapters in the existing canvas controller.
use super::*;
use crate::{
    object::Point,
    selection::Membership,
    text::{Alignment, TextContent, TextFamily, TextObject},
    text_layout,
};
pub(super) struct TextSession {
    pub(super) value: TextObject,
    ctx: egui::Context,
    new: bool,
    focus: bool,
    chunk: f64,
    edits: usize,
    merge: bool,
    composing: bool,
    pending: Vec<Event>,
    clipboard_notice: bool,
    limit_notice: bool,
    history: (bool, bool),
    cursor: Option<egui::text::CCursorRange>,
    restore_cursor: bool,
}
pub(super) struct TextCreation {
    surface: Surface,
    start: Point,
    end: Point,
    view: Transform,
}
pub(super) struct TextResize {
    source: Arc<TextObject>,
    pub(super) preview: TextObject,
    start: Point,
    view: Transform,
    revision: u64,
    moved: bool,
}
pub(super) struct TextCache {
    content: Arc<TextContent>,
    width: f64,
    scale: f64,
    pub(super) galley: Arc<egui::Galley>,
}
pub(super) fn paint_text(
    ui: &egui::Ui,
    painter: &egui::Painter,
    view: Transform,
    text: &TextObject,
    cache: &mut HashMap<Uuid, TextCache>,
) {
    let bounds = text.world_bounds().unwrap();
    let screen = |p: Point| {
        let p = view.to_screen(p);
        egui::pos2(p[0] as f32, p[1] as f32)
    };
    if !Rect::from_min_max(screen(bounds.min), screen(bounds.max)).intersects(painter.clip_rect()) {
        return;
    }
    let scale = view.scale * text.placement.scale;
    if cache.get(&text.id).is_none_or(|c| {
        (!Arc::ptr_eq(&c.content, &text.content) && c.content != text.content)
            || c.width != text.width
            || c.scale != scale
    }) {
        let galley = text_layout::layout_scaled(ui.ctx(), &text.content, text.width, scale);
        cache.insert(
            text.id,
            TextCache {
                content: text.content.clone(),
                width: text.width,
                scale,
                galley,
            },
        );
    }
    let galley = cache[&text.id].galley.clone();
    let offset = match text.content.style.alignment {
        Alignment::Left => 0.,
        Alignment::Center => (text.width * scale - galley.size().x as f64) * 0.5,
        Alignment::Right => text.width * scale - galley.size().x as f64,
    } - f64::from(galley.rect.left());
    let (sin, cos) = text.placement.rotation.sin_cos();
    let pos = screen(text.placement.apply([0.; 2]))
        + egui::vec2((cos * offset) as f32, (sin * offset) as f32);
    painter.add(
        egui::epaint::TextShape::new(pos, galley, Color32::BLACK)
            .with_angle(text.placement.rotation as f32),
    );
}
impl Canvas {
    pub(super) fn text_action(
        &mut self,
        command: crate::selection::SelectionCommand,
        ctx: &egui::Context,
    ) -> bool {
        use crate::selection::SelectionCommand::*;
        let Some(s) = &mut self.text_session else {
            return false;
        };
        let key = |key, shift| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                shift,
                ..Default::default()
            },
        };
        match command {
            Copy => s.pending.push(Event::Copy),
            Cut => s.pending.push(Event::Cut),
            SelectAll => s.pending.push(key(egui::Key::A, false)),
            Undo => s.pending.push(key(egui::Key::Z, false)),
            Redo => s.pending.push(key(egui::Key::Z, true)),
            Delete => s.pending.push(Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }),
            Paste => {
                #[cfg(not(target_arch = "wasm32"))]
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                #[cfg(target_arch = "wasm32")]
                {
                    if let Some(text) =
                        ctx.data(|d| d.get_temp::<String>(egui::Id::unique("XiToc-text-clipboard")))
                    {
                        s.pending.push(Event::Paste(text));
                    } else {
                        s.clipboard_notice = true;
                    }
                }
            }
            _ => return false,
        }
        s.focus = true;
        s.restore_cursor = true;
        ctx.request_repaint();
        true
    }
    pub fn history_available(&self) -> (bool, bool) {
        self.text_session
            .as_ref()
            .map(|s| s.history)
            .unwrap_or((self.drawing.can_undo(), self.drawing.can_redo()))
    }
    pub fn text_editing(&self) -> bool {
        self.text_session.is_some()
    }
    pub(super) fn finish_text(&mut self) {
        if let Some(s) = self.text_session.take() {
            self.tool = self.selection_mode;
            s.ctx
                .memory_mut(|m| m.surrender_focus(egui::Id::unique(("text-editor", s.value.id))));
            if s.new && s.value.content.text.trim().is_empty() {
                self.drawing.erase(&HashSet::from([s.value.id]));
                self.selection.clear();
            }
        }
    }
    fn start_text(&mut self, ui: &egui::Ui, text: TextObject, new: bool) {
        self.finish_text();
        self.selection_gesture = None;
        self.connection_gesture = None;
        let id = egui::Id::unique(("text-editor", text.id));
        let mut state = egui::text_edit::TextEditState::default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(text.content.text.chars().count()),
            )));
        state.store(ui.ctx(), id);
        self.tool = self.selection_mode;
        if new {
            self.selection.clear();
        }
        self.selection.scope(text.surface);
        if !new {
            self.selection
                .change([text.id], Membership::Replace, &self.drawing);
        }
        self.text_session = Some(TextSession {
            value: text,
            ctx: ui.ctx().clone(),
            new,
            focus: true,
            chunk: ui.input(|i| i.time),
            edits: 0,
            merge: false,
            composing: false,
            pending: Vec::new(),
            clipboard_notice: false,
            limit_notice: false,
            history: (false, false),
            cursor: None,
            restore_cursor: false,
        });
    }
    fn editor_rect(text: &TextObject, view: Transform) -> Rect {
        let p = if text.placement.rotation.abs() < 1e-8 {
            text.placement.apply([0.; 2])
        } else {
            text.world_bounds().unwrap().min
        };
        let p = view.to_screen(p);
        Rect::from_min_size(
            egui::pos2(p[0] as f32, p[1] as f32),
            egui::vec2(
                (text.width * text.placement.scale * view.scale) as f32,
                (text.height * text.placement.scale * view.scale)
                    .max(text.content.style.size * view.scale * 1.3) as f32,
            ),
        )
    }
    pub(super) fn text_surface(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) -> bool {
        if self.tool == Tool::Text
            && ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| {
                bounds.intersect(ui.clip_rect()).contains(p)
                    && ui.ctx().layer_id_at(p) == Some(ui.layer_id())
            })
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if self
            .text_session
            .as_ref()
            .is_some_and(|s| s.value.surface == surface)
        {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.finish_text();
                return true;
            }
            let rect = Self::editor_rect(&self.text_session.as_ref().unwrap().value, view);
            let outside=ui.input(|i|i.events.iter().any(|e|matches!(e,Event::PointerButton{pos,pressed:true,..} if bounds.intersect(ui.clip_rect()).contains(*pos)&&!rect.contains(*pos)&&ui.ctx().layer_id_at(*pos)==Some(ui.layer_id()))));
            if outside {
                self.finish_text();
                self.tool = self.selection_mode;
                return true;
            }
            return true;
        }
        if self
            .text_resize
            .as_ref()
            .is_some_and(|r| r.source.surface == surface)
        {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
                || self
                    .text_resize
                    .as_ref()
                    .is_some_and(|r| r.view != view || r.revision != self.drawing.revision())
            {
                self.text_resize = None;
                return true;
            }
            for event in ui.input(|i| i.events.clone()) {
                match event {
                    Event::PointerMoved(pos)
                    | Event::PointerButton {
                        pos,
                        pressed: false,
                        button: PointerButton::Primary,
                        ..
                    } => {
                        let point = view.to_document([pos.x as f64, pos.y as f64]);
                        let r = self.text_resize.as_mut().unwrap();
                        let local = r.source.placement.inverse(point);
                        let start = r.source.placement.inverse(r.start);
                        let mut candidate = (*r.source).clone();
                        candidate.width = (candidate.width + local[0] - start[0]).clamp(24., 1e6);
                        text_layout::measure(ui.ctx(), &mut candidate);
                        self.drawing.fit_text(&mut candidate);
                        if self.drawing.valid_object(&candidate.clone().into()) {
                            r.preview = candidate;
                            r.moved = (point[0] - r.start[0]).hypot(point[1] - r.start[1])
                                * view.scale
                                >= 3.;
                        }
                        if matches!(event, Event::PointerButton { pressed: false, .. }) {
                            let r = self.text_resize.take().unwrap();
                            if r.moved {
                                self.drawing.replace_texts(vec![r.preview], false);
                                self.selection.refresh(&self.drawing);
                            }
                            break;
                        }
                    }
                    _ => (),
                }
            }
            return true;
        }
        if self.text_creation.is_some()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.text_creation = None;
            return true;
        }
        if self
            .text_creation
            .as_ref()
            .is_some_and(|d| d.surface == surface && d.view != view)
        {
            self.text_creation = None;
        }
        let mut consumed = self
            .text_creation
            .as_ref()
            .is_some_and(|d| d.surface == surface);
        for event in ui.input(|i| i.events.clone()) {
            match event {
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } if self.text_creation.is_none()
                    && self.selection_gesture.is_none()
                    && !modifiers.shift
                    && !modifiers.alt
                    && !ui.input(|i| i.key_down(egui::Key::Space))
                    && bounds.intersect(ui.clip_rect()).contains(pos)
                    && ui.is_enabled()
                    && ui.ctx().layer_id_at(pos) == Some(ui.layer_id()) =>
                {
                    let p = view.to_document([pos.x as f64, pos.y as f64]);
                    if matches!(self.tool, Tool::Select | Tool::Lasso)
                        && self.selection.ids.len() == 1
                        && let Some(ObjectSnapshot::Text(t)) = self
                            .selection
                            .ids
                            .iter()
                            .next()
                            .and_then(|id| self.drawing.snapshot(*id))
                    {
                        let h = t.placement.apply([t.width, t.height * 0.5]);
                        if (p[0] - h[0]).hypot(p[1] - h[1]) * view.scale <= 7. {
                            self.text_resize = Some(TextResize {
                                source: t.clone(),
                                preview: (**t).clone(),
                                start: p,
                                view,
                                revision: self.drawing.revision(),
                                moved: false,
                            });
                            return self.text_surface(ui, bounds, surface, view);
                        }
                    }
                    if self.tool == Tool::Text {
                        if let Some(id) = self.pick(surface, p, 0.)
                            && let Some(ObjectSnapshot::Text(t)) = self.drawing.snapshot(id)
                        {
                            self.start_text(ui, (**t).clone(), false);
                            return true;
                        }
                        if self.pick(surface, p, 0.).is_none() {
                            self.text_creation = Some(TextCreation {
                                surface,
                                start: p,
                                end: p,
                                view,
                            });
                            consumed = true;
                        }
                    }
                }
                Event::PointerMoved(pos)
                    if self
                        .text_creation
                        .as_ref()
                        .is_some_and(|d| d.surface == surface) =>
                {
                    self.text_creation.as_mut().unwrap().end =
                        view.to_document([pos.x as f64, pos.y as f64]);
                }
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                } if self
                    .text_creation
                    .as_ref()
                    .is_some_and(|d| d.surface == surface) =>
                {
                    let d = self.text_creation.take().unwrap();
                    let end = view.to_document([pos.x as f64, pos.y as f64]);
                    let dragged = (end[0] - d.start[0]).abs() * view.scale >= 3.;
                    let mut point = d.start;
                    let mut width = if dragged {
                        (end[0] - d.start[0]).abs().max(24.)
                    } else {
                        240.
                    };
                    if dragged {
                        point[0] = point[0].min(end[0]);
                    }
                    if let Some(page) = self.drawing.bounds(surface) {
                        width = width.min(page[0] - point[0]);
                        point[1] = point[1].min(page[1] - 26.);
                    }
                    let mut text = TextObject::new(self.drawing.owner(), surface, point, width);
                    if surface == Surface::Canvas {
                        text.content_mut().style.color =
                            crate::ui::style::palette(ui).text.to_array();
                    }
                    if text.valid() {
                        self.start_text(ui, text, true);
                    }
                    consumed = true;
                }
                _ => (),
            }
        }
        if matches!(self.tool, Tool::Select | Tool::Lasso)
            && ui.input(|i| i.pointer.button_double_clicked(PointerButton::Primary))
            && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
            && bounds.intersect(ui.clip_rect()).contains(pos)
            && let Some(id) = self.pick(surface, view.to_document([pos.x as f64, pos.y as f64]), 0.)
            && let Some(ObjectSnapshot::Text(t)) = self.drawing.snapshot(id)
        {
            self.start_text(ui, (**t).clone(), false);
            return true;
        }
        consumed
    }
    pub(super) fn paint_text_editor(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) {
        let Some(mut s) = self.text_session.take() else {
            if let Some(d) = &self.text_creation
                && d.surface == surface
            {
                let width = if (d.end[0] - d.start[0]).abs() * view.scale >= 3. {
                    (d.end[0] - d.start[0]).abs().max(24.)
                } else {
                    240.
                };
                let p = view.to_screen([d.start[0].min(d.end[0]), d.start[1]]);
                let rect = Rect::from_min_size(
                    egui::pos2(p[0] as f32, p[1] as f32),
                    egui::vec2((width * view.scale) as f32, (26. * view.scale) as f32),
                );
                ui.painter()
                    .with_clip_rect(bounds.intersect(ui.clip_rect()))
                    .rect_stroke(
                        rect,
                        2.,
                        Line::new(1., crate::ui::style::palette(ui).accent),
                        egui::StrokeKind::Outside,
                    );
            }
            return;
        };
        if s.value.surface != surface {
            self.text_session = Some(s);
            return;
        }
        if let Some(ObjectSnapshot::Text(current)) = self.drawing.snapshot(s.value.id) {
            s.value.placement = current.placement;
            s.value.width = current.width;
            s.value.height = current.height;
            if s.value.content.style != current.content.style {
                s.value.content_mut().style = current.content.style.clone();
            }
        }
        let rect = Self::editor_rect(&s.value, view);
        let scale = view.scale * s.value.placement.scale;
        let id = egui::Id::unique(("text-editor", s.value.id));
        let layer = egui::LayerId::new(ui.layer_id().order, id.with("layer"));
        ui.ctx().set_sublayer(ui.layer_id(), layer);
        let mut child = egui::Ui::new(
            ui.ctx().clone(),
            id.with("ui"),
            egui::UiBuilder::new().layer_id(layer).max_rect(rect),
        );
        child.set_clip_rect(bounds.intersect(ui.clip_rect()));
        let palette = crate::ui::style::palette(ui);
        child.painter().rect_stroke(
            rect,
            2.,
            Line::new(1., palette.accent),
            egui::StrokeKind::Outside,
        );
        if s.value.placement.rotation.abs() > 1e-8 {
            child.painter().text(
                rect.left_top() - egui::vec2(0., 5.),
                egui::Align2::LEFT_BOTTOM,
                "Editing unrotated · Escape finishes",
                egui::FontId::proportional(11.),
                palette.text,
            );
        }
        if s.restore_cursor {
            if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), id) {
                state.cursor.set_char_range(s.cursor);
                state.store(ui.ctx(), id);
            }
            s.restore_cursor = false;
        }
        let request_focus = s.focus;
        if request_focus {
            ui.memory_mut(|m| m.request_focus(id));
        }
        for event in ui.input(|i| i.events.clone()) {
            match event {
                Event::Ime(egui::ImeEvent::Preedit { text, .. }) => s.composing = !text.is_empty(),
                Event::Ime(egui::ImeEvent::Commit(_)) => s.composing = false,
                _ => (),
            }
        }
        if !s.pending.is_empty() {
            ui.input_mut(|i| i.events.append(&mut s.pending));
        }
        if s.clipboard_notice {
            child.painter().text(
                rect.left_bottom() + egui::vec2(0., 4.),
                egui::Align2::LEFT_TOP,
                "Browser paste: Ctrl/Cmd+V, or copy text within XiToc first.",
                egui::FontId::proportional(11.),
                egui::Color32::from_rgb(190, 100, 30),
            );
        }
        if ui.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    Event::Cut
                        | Event::Paste(_)
                        | Event::Ime(egui::ImeEvent::Commit(_))
                        | Event::Key {
                            key: egui::Key::Z,
                            pressed: true,
                            ..
                        }
                )
            })
        }) {
            s.merge = false;
        }
        let rejected = ui.input_mut(|i| {
            let mut rejected = false;
            i.events.retain(|event| {
                let text = match event {
                    Event::Text(text)
                    | Event::Paste(text)
                    | Event::Ime(egui::ImeEvent::Commit(text))
                    | Event::Ime(egui::ImeEvent::Preedit { text, .. }) => Some(text),
                    _ => None,
                };
                let keep = text.is_none_or(|text| {
                    text.len() <= crate::text::MAX_TEXT_BYTES && !text.contains('\0')
                });
                rejected |= !keep;
                keep
            });
            rejected
        });
        if rejected {
            s.limit_notice = true;
        }
        let old = s.value.content.text.clone();
        let output = egui::ScrollArea::vertical()
            .id_salt(id.with("scroll"))
            .max_height(rect.height().min(bounds.bottom() - rect.top()).max(24.))
            .show(&mut child, |ui| {
                text_layout::edit(
                    ui,
                    id,
                    Arc::make_mut(&mut s.value.content),
                    s.value.width,
                    scale,
                )
            })
            .inner;
        if request_focus {
            output.response.request_focus();
            s.focus = false;
            ui.ctx().request_repaint();
        }
        if output.response.has_focus() {
            s.cursor = output.state.cursor.char_range();
        }
        let current = (
            output.state.cursor.char_range().unwrap_or_default(),
            s.value.content.text.clone(),
        );
        let undoer = output.state.undoer();
        s.history = (undoer.has_undo(&current), undoer.has_redo(&current));
        let copied = ui.ctx().output(|o| {
            o.commands.iter().rev().find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
        });
        let pasted = ui.input(|i| {
            i.events.iter().rev().find_map(|event| match event {
                Event::Paste(text) => Some(text.clone()),
                _ => None,
            })
        });
        if let Some(text) = copied.or(pasted) {
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::unique("XiToc-text-clipboard"), text));
            s.clipboard_notice = false;
        }
        if output.response.changed() {
            if s.value.content.text.len() > crate::text::MAX_TEXT_BYTES
                || s.value.content.text.contains('\0')
            {
                s.value.content_mut().text = old;
                s.limit_notice = true;
            } else {
                text_layout::measure(ui.ctx(), &mut s.value);
                self.drawing.fit_text(&mut s.value);
                if s.composing {
                    self.text_session = Some(s);
                    return;
                }
                if !self.drawing.valid_object(&s.value.clone().into())
                    && let Some(ObjectSnapshot::Text(old)) = self.drawing.snapshot(s.value.id)
                {
                    s.value.height = old.height;
                }
                let time = ui.input(|i| i.time);
                let merge = s.merge && time - s.chunk < 2. && s.edits < 32;
                if self.drawing.replace_texts(vec![s.value.clone()], merge) {
                    if !merge {
                        s.chunk = time;
                        s.edits = 0;
                    }
                    s.edits += 1;
                    s.merge = true;
                    self.selection
                        .change([s.value.id], Membership::Replace, &self.drawing);
                }
            }
        }
        if s.limit_notice {
            child.painter().text(
                rect.left_bottom() + egui::vec2(0., 4.),
                egui::Align2::LEFT_TOP,
                "Text limit: 64 KiB. The rejected edit was not applied.",
                egui::FontId::proportional(11.),
                egui::Color32::from_rgb(190, 100, 30),
            );
        }
        self.text_session = Some(s);
    }
    pub(super) fn paint_text_handles(&self, ui: &egui::Ui, view: Transform, surface: Surface) {
        if self.text_session.is_some()
            || !matches!(self.tool, Tool::Select | Tool::Lasso)
            || self.selection.surface != surface
            || self.selection.ids.len() != 1
        {
            return;
        }
        if let Some(ObjectSnapshot::Text(t)) = self
            .selection
            .ids
            .iter()
            .next()
            .and_then(|id| self.drawing.snapshot(*id))
        {
            let t = self.text_resize.as_ref().map(|r| &r.preview).unwrap_or(t);
            let moved = self.selection_preview(surface).map(|d| t.transformed(d));
            let t = moved.as_ref().unwrap_or(t);
            let p = view.to_screen(t.placement.apply([t.width, t.height * 0.5]));
            let p = egui::pos2(p[0] as f32, p[1] as f32);
            ui.painter().rect_filled(
                Rect::from_center_size(p, egui::vec2(6., 12.)),
                2.,
                crate::ui::style::palette(ui).accent,
            );
        }
    }
    pub(super) fn text_format_controls(&mut self, ui: &mut egui::Ui) {
        let texts: Vec<_> = self
            .selection
            .ids
            .iter()
            .filter_map(|id| match self.drawing.snapshot(*id) {
                Some(ObjectSnapshot::Text(t)) => Some((**t).clone()),
                _ => None,
            })
            .collect();
        if texts.is_empty() {
            return;
        }
        let mut style = texts[0].content.style.clone();
        let mut changed = false;
        let mut patch = crate::text::TextStylePatch::default();
        let mut control = ui.scope_id();
        crate::ui::shape_controls::menu(ui, "Text", |ui| {
            ui.weak(if texts.iter().any(|t| t.content.style != style) {
                "Mixed text styles"
            } else {
                "Selected text"
            })
            .on_hover_text("Changes apply to all selected text boxes.");
            ui.horizontal(|ui| {
                for (label, family) in [("Sans", TextFamily::Sans), ("Mono", TextFamily::Monospace)]
                {
                    if ui
                        .selectable_value(&mut style.family, family, label)
                        .clicked()
                    {
                        changed = true;
                        patch.family = Some(family);
                    }
                }
            });
            let r = crate::ui::drawing_controls::styled_slider(
                ui,
                egui::Slider::new(&mut style.size, 4.0..=128.)
                    .step_by(1.)
                    .fixed_decimals(0)
                    .text("Font size")
                    .suffix(" pt"),
            );
            if r.changed() {
                changed = true;
                patch.size = Some(style.size);
                control = r.id;
            }
            ui.horizontal(|ui| {
                if ui.checkbox(&mut style.bold, "Bold").changed() {
                    patch.bold = Some(style.bold);
                    changed = true;
                }
                if ui.checkbox(&mut style.italic, "Italic").changed() {
                    patch.italic = Some(style.italic);
                    changed = true;
                }
                if ui.checkbox(&mut style.underline, "Underline").changed() {
                    patch.underline = Some(style.underline);
                    changed = true;
                }
            });
            let mut rgb = [style.color[0], style.color[1], style.color[2]];
            if let Some(id) = crate::ui::shape_controls::color_control(ui, &mut rgb, style.color[3])
            {
                style.color[..3].copy_from_slice(&rgb);
                patch.rgb = Some(rgb);
                changed = true;
                control = id;
            }
            let mut opacity = f32::from(style.color[3]) * 100. / 255.;
            let r = crate::ui::drawing_controls::styled_slider(
                ui,
                egui::Slider::new(&mut opacity, 0.0..=100.)
                    .text("Opacity")
                    .suffix("%"),
            );
            if r.changed() {
                style.color[3] = (opacity * 255. / 100.).round() as u8;
                patch.alpha = Some(style.color[3]);
                changed = true;
                control = r.id;
            }
            ui.horizontal(|ui| {
                for (label, a) in [
                    ("Left", Alignment::Left),
                    ("Center", Alignment::Center),
                    ("Right", Alignment::Right),
                ] {
                    if ui
                        .selectable_value(&mut style.alignment, a, label)
                        .clicked()
                    {
                        patch.alignment = Some(a);
                        changed = true;
                    }
                }
            });
        });
        if changed {
            let mut values = Vec::new();
            for mut t in texts {
                patch.apply(&mut t.content_mut().style);
                text_layout::measure(ui.ctx(), &mut t);
                self.drawing.fit_text(&mut t);
                values.push(t);
            }
            let merge = self.style_gesture == Some((control, self.drawing.revision()));
            if self.drawing.replace_texts(values, merge) {
                if let Some(s) = &mut self.text_session {
                    s.merge = false;
                }
                self.style_gesture = ui
                    .input(|i| i.pointer.primary_down())
                    .then_some((control, self.drawing.revision()));
                self.selection.refresh(&self.drawing);
            }
        }
    }
}
