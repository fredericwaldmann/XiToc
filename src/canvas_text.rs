//! Text input/display adapters in the existing canvas controller.
use super::*;
use crate::{
    object::Point,
    selection::Membership,
    text::{Alignment, TextContent, TextObject},
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
    typing: crate::text::TextStyle,
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
    handle: usize,
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
    let pos = screen(text.placement.apply([0.; 2]));
    painter.add(
        egui::epaint::TextShape::new(pos, galley, Color32::BLACK)
            .with_angle(text.placement.rotation as f32),
    );
}
impl Canvas {
    pub(super) fn scale_selected_text_sizes(&mut self, factor: f64, ctx: &egui::Context) -> bool {
        self.finish_text();
        let mut texts = Vec::new();
        let mut any_changed = false;
        for id in &self.selection.ids {
            let Some(ObjectSnapshot::Text(snapshot)) = self.drawing.snapshot(*id) else {
                return false;
            };
            let mut text = (**snapshot).clone();
            any_changed |= text.content_mut().scale_font_sizes(factor);
            text_layout::measure(ctx, &mut text);
            self.drawing.fit_text(&mut text);
            texts.push(text);
        }
        if texts.is_empty() || !any_changed || !self.drawing.replace_texts(texts, false) {
            return false;
        }
        self.selection.refresh(&self.drawing);
        self.prune_cache();
        true
    }

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
        let cursor = egui::text::CCursorRange::one(egui::text::CCursor::new(
            text.content.text.chars().count(),
        ));
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(cursor));
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
            typing: text.content.style.clone(),
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
            cursor: Some(cursor),
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
        if self.font_browser.is_some() && self.text_session.is_none() {
            return true;
        }
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
            if self.font_browser.is_some() {
                return true;
            }
            if !egui::Popup::is_any_open(ui.ctx())
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            {
                self.finish_text();
                return true;
            }
            let rect = Self::editor_rect(&self.text_session.as_ref().unwrap().value, view);
            let events = ui.input(|i| i.events.clone());
            let outside = events.iter().any(|e| {
                matches!(e, Event::PointerButton {pos,pressed:true,..}
                if bounds.intersect(ui.clip_rect()).contains(*pos) && !rect.contains(*pos)
                && ui.ctx().layer_id_at(*pos)==Some(ui.layer_id()))
            });
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
                        let mut candidate = r.source.resized(r.handle, r.start, point);
                        let desired = candidate.height;
                        text_layout::measure(ui.ctx(), &mut candidate);
                        candidate.height = candidate.height.max(desired);
                        candidate.anchor_resize(&r.source, r.handle);
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
                        && let Some(handle) = crate::text::TEXT_HANDLES.iter().position(|h| {
                            let h = t.placement.apply([t.width * h[0], t.height * h[1]]);
                            (p[0] - h[0]).hypot(p[1] - h[1]) * view.scale <= 7.
                        })
                    {
                        self.text_resize = Some(TextResize {
                            source: t.clone(),
                            preview: (**t).clone(),
                            start: p,
                            view,
                            revision: self.drawing.revision(),
                            moved: false,
                            handle,
                        });
                        return self.text_surface(ui, bounds, surface, view);
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
                    text.layout.resize = self.text_resize_default;
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
            if !s.composing {
                s.value.content = current.content.clone();
                s.value.layout = current.layout.clone();
            }
        }
        if let Some(preview) = self
            .font_browser
            .as_ref()
            .and_then(|b| b.previews.iter().find(|p| p.id == s.value.id))
        {
            s.value.content = preview.content.clone();
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
        let editor_pointer = ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| {
            rect.contains(p)
                && ui
                    .ctx()
                    .layer_id_at(p)
                    .is_some_and(|l| l == layer || l == ui.layer_id())
        });
        let outside_press = ui.input(|i| i.pointer.any_pressed()) && !editor_pointer;
        let locked = self.font_browser.is_some()
            || outside_press
            || (egui::Popup::is_any_open(ui.ctx()) && !editor_pointer);
        if locked {
            s.focus = true;
            ui.memory_mut(|m| m.surrender_focus(id));
        }
        let editor_input = !s.pending.is_empty()
            || ui.input(|i| {
                i.events.iter().any(|e| {
                    matches!(
                        e,
                        Event::Key { .. }
                            | Event::Text(_)
                            | Event::Paste(_)
                            | Event::Cut
                            | Event::Ime(_)
                    )
                })
            })
            || (editor_pointer
                && ui.input(|i| i.pointer.primary_down() || i.pointer.any_released()));
        let request_focus = s.focus
            && !locked
            && !egui::Popup::is_any_open(ui.ctx())
            && self.font_browser.is_none();
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
        if !locked && !s.pending.is_empty() {
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
        let editor_focused = ui.memory(|m| m.has_focus(id));
        let history = ui.input_mut(|i| {
            let mut command = None;
            i.events.retain(|e| {
                if let Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = e
                    && matches!(key, egui::Key::Z | egui::Key::Y)
                    && modifiers.command
                    && editor_focused
                {
                    command = Some(modifiers.shift || *key == egui::Key::Y);
                    false
                } else {
                    true
                }
            });
            command
        });
        if let Some(redo) = history {
            if redo {
                self.drawing.redo();
            } else {
                self.drawing.undo();
            }
            if let Some(ObjectSnapshot::Text(t)) = self.drawing.snapshot(s.value.id) {
                s.value = (**t).clone();
            } else {
                s.value.content_mut().text.clear();
                s.value.content_mut().runs.clear();
            }
            s.merge = false;
        }
        ui.ctx()
            .data_mut(|d| d.insert_temp(id.with("typing"), s.typing.clone()));
        let wrap_width = s.value.width;
        let before = s.value.content.clone();
        let selected_before = s.cursor.map(|r| {
            let [a, b] = r.sorted_cursors();
            a.index.0..b.index.0
        });
        let old = s.value.content.text.clone();
        let output = egui::ScrollArea::vertical()
            .id_salt(id.with("scroll"))
            .max_height(rect.height().min(bounds.bottom() - rect.top()).max(24.))
            .show(&mut child, |ui| {
                text_layout::edit(
                    ui,
                    id,
                    Arc::make_mut(&mut s.value.content),
                    wrap_width,
                    scale,
                    s.cursor,
                    !locked,
                )
            })
            .inner;
        if request_focus {
            output.response.request_focus();
            s.focus = false;
            ui.ctx().request_repaint();
        }
        if !locked && output.response.has_focus() && (editor_input || output.response.changed()) {
            let next = output.state.cursor.char_range();
            if next != s.cursor
                && !output.response.changed()
                && let Some(r) = next
            {
                let index = if r.is_empty() {
                    r.primary.index.0.saturating_sub(1)
                } else {
                    r.sorted_cursors()[0].index.0
                };
                s.typing = s.value.content.style_at(index).clone();
            }
            s.cursor = next;
        }
        s.history = (self.drawing.can_undo(), self.drawing.can_redo());
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
        if copied.is_some()
            && let Some(range) = selected_before
            && !range.is_empty()
        {
            let rich = before.slice(range);
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::unique("XiToc-rich-clipboard"), rich));
        }
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
            for h in crate::text::TEXT_HANDLES {
                let p = view.to_screen(t.placement.apply([t.width * h[0], t.height * h[1]]));
                ui.painter().rect_filled(
                    Rect::from_center_size(
                        egui::pos2(p[0] as f32, p[1] as f32),
                        egui::vec2(8., 8.),
                    ),
                    2.,
                    crate::ui::style::palette(ui).accent,
                );
            }
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
        self.font_browser_ui(ui);
        let mut style = self
            .text_session
            .as_ref()
            .map(|s| {
                if let Some(r) = s.cursor
                    && !r.is_empty()
                {
                    s.value
                        .content
                        .style_at(r.sorted_cursors()[0].index.0)
                        .clone()
                } else {
                    s.typing.clone()
                }
            })
            .unwrap_or_else(|| texts[0].content.style.clone());
        let selected_styles: Vec<_> = if let Some(s) = &self.text_session {
            if let Some(r) = s.cursor
                && !r.is_empty()
            {
                let [a, b] = r.sorted_cursors();
                s.value
                    .content
                    .segments()
                    .into_iter()
                    .filter(|r| r.range.start < b.index.0 && r.range.end > a.index.0)
                    .map(|r| r.style)
                    .collect()
            } else {
                vec![s.typing.clone()]
            }
        } else {
            texts
                .iter()
                .flat_map(|t| {
                    if t.content.runs.is_empty() {
                        vec![t.content.style.clone()]
                    } else {
                        t.content.runs.iter().map(|r| r.style.clone()).collect()
                    }
                })
                .collect()
        };
        let mixed_family = selected_styles.iter().any(|s| s.family != style.family);
        let mixed_size = selected_styles.iter().any(|s| s.size != style.size);
        let mixed_bold = selected_styles.iter().any(|s| s.bold != style.bold);
        let mixed_italic = selected_styles.iter().any(|s| s.italic != style.italic);
        let mixed_underline = selected_styles
            .iter()
            .any(|s| s.underline != style.underline);
        let mixed_color = selected_styles.iter().any(|s| s.color != style.color);
        let mut changed = false;
        let mut patch = crate::text::TextStylePatch::default();
        let paragraph_offset = self
            .text_session
            .as_ref()
            .and_then(|s| s.cursor)
            .map(|r| r.sorted_cursors()[0].index.0)
            .unwrap_or(0);
        let paragraph_index = texts[0]
            .content
            .text
            .chars()
            .take(paragraph_offset)
            .filter(|c| *c == '\n')
            .count();
        let mut paragraph_style = texts[0].content.paragraph(paragraph_index);
        let mut alignment_patch = None;
        let mut resize_patch = None;
        let mut line_spacing = None;
        let mut paragraph_spacing = None;
        let mut control = ui.scope_id();
        crate::ui::shape_controls::menu(ui, "Text", |ui| {
            ui.weak(if selected_styles.iter().any(|s| s != &style) {
                "Mixed text styles"
            } else {
                "Selected text"
            })
            .on_hover_text("Selected characters, or the typing style at the caret.");
            if ui
                .button(format!(
                    "Font: {}",
                    if mixed_family {
                        "Mixed"
                    } else {
                        crate::text_fonts::label(&style.family)
                    }
                ))
                .clicked()
            {
                self.open_font_browser(&texts);
                ui.close();
            }
            ui.horizontal(|ui| {
                if ui.button("−").clicked() {
                    style.size = (style.size - 1.).max(4.);
                    patch.size = Some(style.size);
                    changed = true;
                }
                if mixed_size {
                    let id = ui.make_persistent_id("mixed-font-size-entry");
                    let mut entry = ui.data(|d| {
                        d.get_temp::<String>(id)
                            .unwrap_or_else(|| "Mixed".to_owned())
                    });
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut entry)
                            .desired_width(72.0)
                            .hint_text("Mixed"),
                    );
                    if r.changed() {
                        if let Ok(size) = entry.trim().parse::<f64>()
                            && (4.0..=512.0).contains(&size)
                        {
                            style.size = size;
                            patch.size = Some(size);
                            changed = true;
                            control = r.id;
                        }
                    } else if r.lost_focus() {
                        entry = "Mixed".to_owned();
                    }
                    ui.data_mut(|d| d.insert_temp(id, entry));
                    ui.label("pt");
                } else {
                    let r = ui.add(
                        egui::DragValue::new(&mut style.size)
                            .range(4.0..=512.)
                            .suffix(" pt"),
                    );
                    if r.changed() {
                        patch.size = Some(style.size);
                        changed = true;
                        control = r.id;
                    }
                }
                if ui.button("+").clicked() {
                    style.size = (style.size + 1.).min(512.);
                    patch.size = Some(style.size);
                    changed = true;
                }
            });
            if !mixed_size {
                let r = crate::ui::drawing_controls::styled_slider(
                    ui,
                    egui::Slider::new(&mut style.size, 4.0..=512.)
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
            }
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Checkbox::new(&mut style.bold, "Bold").indeterminate(mixed_bold))
                    .changed()
                {
                    patch.bold = Some(style.bold);
                    changed = true;
                }
                if ui
                    .add(
                        egui::Checkbox::new(&mut style.italic, "Italic")
                            .indeterminate(mixed_italic),
                    )
                    .changed()
                {
                    patch.italic = Some(style.italic);
                    changed = true;
                }
                if ui
                    .add(
                        egui::Checkbox::new(&mut style.underline, "Underline")
                            .indeterminate(mixed_underline),
                    )
                    .changed()
                {
                    patch.underline = Some(style.underline);
                    changed = true;
                }
            });
            if mixed_color {
                ui.weak("Text color: Mixed");
            }
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
        });
        crate::ui::shape_controls::menu(ui, "Paragraph", |ui| {
            ui.horizontal(|ui| {
                for (label, a) in [
                    ("Left", Alignment::Left),
                    ("Center", Alignment::Center),
                    ("Right", Alignment::Right),
                    ("Justify", Alignment::Justify),
                ] {
                    if ui
                        .selectable_value(&mut paragraph_style.alignment, a, label)
                        .clicked()
                    {
                        alignment_patch = Some(a);
                        changed = true;
                    }
                }
            });
            let p = &mut paragraph_style;
            if ui
                .add(
                    egui::DragValue::new(&mut p.line_spacing)
                        .range(0.5..=4.)
                        .speed(0.05)
                        .prefix("Line spacing "),
                )
                .changed()
            {
                line_spacing = Some(p.line_spacing);
                changed = true;
            }
            if ui
                .add(
                    egui::DragValue::new(&mut p.spacing)
                        .range(0.0..=512.)
                        .prefix("Paragraph spacing "),
                )
                .changed()
            {
                paragraph_spacing = Some(p.spacing);
                changed = true;
            }
            ui.separator();
            ui.label("Text layout · resize behavior");
            let mut mode = texts[0].layout.resize;
            for (label, value) in [
                ("Reflow Text", crate::text::ResizeBehavior::Reflow),
                ("Scale Text (corners)", crate::text::ResizeBehavior::Scale),
            ] {
                if ui.selectable_value(&mut mode, value, label).clicked() {
                    resize_patch = Some(value);
                    changed = true;
                }
            }
            ui.weak("Edge handles always reflow text.");
        });
        if changed {
            let character_changed = patch != crate::text::TextStylePatch::default();
            let mut values = Vec::new();
            for mut t in texts {
                if let Some(mode) = resize_patch {
                    t.layout.resize = mode;
                }
                if let Some(s) = &mut self.text_session
                    && s.value.id == t.id
                {
                    let range = s
                        .cursor
                        .map(|r| {
                            let [a, b] = r.sorted_cursors();
                            a.index.0..b.index.0
                        })
                        .unwrap_or(0..0);
                    if let Some(a) = alignment_patch {
                        t.content_mut()
                            .format_paragraphs(range.clone(), |p| p.alignment = a);
                    }
                    if character_changed {
                        if range.is_empty() {
                            patch.apply(&mut s.typing);
                        } else {
                            t.content_mut().format(range.clone(), &patch);
                            s.typing = t.content.style_at(range.start).clone();
                        }
                    }
                    if line_spacing.is_some() || paragraph_spacing.is_some() {
                        t.content_mut().format_paragraphs(range.clone(), |p| {
                            if let Some(v) = line_spacing {
                                p.line_spacing = v;
                            }
                            if let Some(v) = paragraph_spacing {
                                p.spacing = v;
                            }
                        });
                    }
                    s.focus = true;
                } else {
                    let n = t.content.text.chars().count();
                    if character_changed {
                        t.content_mut().format(0..n, &patch);
                    }
                    if let Some(a) = alignment_patch {
                        t.content_mut().format_paragraphs(0..n, |p| p.alignment = a);
                    }
                    if character_changed {
                        patch.apply(&mut t.content_mut().style);
                    }
                    if line_spacing.is_some() || paragraph_spacing.is_some() {
                        t.content_mut().format_paragraphs(0..n, |p| {
                            if let Some(v) = line_spacing {
                                p.line_spacing = v;
                            }
                            if let Some(v) = paragraph_spacing {
                                p.spacing = v;
                            }
                        });
                    }
                }
                text_layout::measure(ui.ctx(), &mut t);
                self.drawing.fit_text(&mut t);
                values.push(t);
            }
            let merge = self.style_gesture == Some((control, self.drawing.revision()));
            if self.drawing.replace_texts(values, merge) {
                ui.ctx().request_repaint();
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

pub(super) struct FontBrowser {
    sources: Vec<TextObject>,
    pub(super) previews: Vec<TextObject>,
    families: Vec<(String, crate::text::TextFamily)>,
    search: String,
    index: usize,
}
impl FontBrowser {
    fn preview(&mut self, range: Option<std::ops::Range<usize>>) {
        self.previews = self.sources.clone();
        if let Some((_, family)) = self.families.get(self.index) {
            let patch = crate::text::TextStylePatch {
                family: Some(family.clone()),
                ..Default::default()
            };
            for t in &mut self.previews {
                let range = range.clone().unwrap_or(0..t.content.text.chars().count());
                t.content_mut().format(range, &patch);
            }
        }
    }
}
impl Canvas {
    fn open_font_browser(&mut self, texts: &[TextObject]) {
        let families = crate::text_fonts::families();
        let family = self
            .text_session
            .as_ref()
            .map(|s| {
                if let Some(r) = s.cursor
                    && !r.is_empty()
                {
                    &s.value
                        .content
                        .style_at(r.sorted_cursors()[0].index.0)
                        .family
                } else {
                    &s.typing.family
                }
            })
            .unwrap_or(&texts[0].content.style.family);
        let index = families.iter().position(|(_, f)| f == family).unwrap_or(0);
        self.font_browser = Some(FontBrowser {
            sources: texts.to_vec(),
            previews: texts.to_vec(),
            families,
            search: String::new(),
            index,
        });
    }
    fn font_browser_ui(&mut self, ui: &egui::Ui) {
        let Some(mut browser) = self.font_browser.take() else {
            return;
        };
        let mut open = true;
        let mut commit = false;
        let mut cancel = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let up = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
        let down = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
        commit |= ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let visible: Vec<_> = browser
            .families
            .iter()
            .enumerate()
            .filter(|(_, (n, _))| n.to_lowercase().contains(&browser.search.to_lowercase()))
            .map(|(i, _)| i)
            .collect();
        let range = self.text_session.as_ref().map(|s| {
            s.cursor
                .map(|r| {
                    let [a, b] = r.sorted_cursors();
                    a.index.0..b.index.0
                })
                .unwrap_or(0..0)
        });
        let previous = browser.index;
        if (up || down) && !visible.is_empty() {
            let current = visible
                .iter()
                .position(|i| *i == browser.index)
                .unwrap_or(0);
            browser.index = visible[if up {
                current.saturating_sub(1)
            } else {
                (current + 1).min(visible.len() - 1)
            }];
        }
        egui::Window::new("Font browser")
            .id(egui::Id::unique("rich-font-browser"))
            .open(&mut open)
            .default_width(300.)
            .show(ui.ctx(), |ui| {
                ui.label("↑/↓ preview · Enter confirms · Escape cancels");
                ui.text_edit_singleline(&mut browser.search);
                #[cfg(target_arch = "wasm32")]
                ui.weak("Bundled fonts · system font access is unavailable here.");
                let mut list = egui::ScrollArea::vertical().max_height(280.);
                let row_height = ui.text_style_height(&egui::TextStyle::Body) + 6.;
                if up || down {
                    let row = visible
                        .iter()
                        .position(|i| *i == browser.index)
                        .unwrap_or(0);
                    list = list.vertical_scroll_offset((row as f32 * row_height - 100.).max(0.));
                }
                list.show_rows(ui, row_height, visible.len(), |ui, rows| {
                    for row in rows {
                        let i = visible[row];
                        let template = TextContent {
                            style: crate::text::TextStyle {
                                family: browser.families[i].1.clone(),
                                size: 14.,
                                ..Default::default()
                            },
                            ..Default::default()
                        };
                        let label = egui::RichText::new(&browser.families[i].0)
                            .font(text_layout::font(&template));
                        let response = ui.selectable_label(browser.index == i, label);
                        if response.clicked() {
                            browser.index = i;
                            commit = true;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    commit |= ui.button("Apply font").clicked();
                    cancel |= ui.button("Cancel").clicked();
                });
            });
        if previous != browser.index {
            browser.preview(range.clone());
            ui.ctx().request_repaint();
        }
        if cancel || !open {
            if let Some(s) = &mut self.text_session {
                s.focus = true;
            }
        } else if commit {
            browser.preview(range.clone());
            if let Some(s) = &mut self.text_session {
                if let Some((_, family)) = browser.families.get(browser.index) {
                    s.typing.family = family.clone();
                }
                s.focus = true;
                s.merge = false;
            }
            let values = browser
                .previews
                .into_iter()
                .map(|mut t| {
                    text_layout::measure(ui.ctx(), &mut t);
                    self.drawing.fit_text(&mut t);
                    t
                })
                .collect();
            self.drawing.replace_texts(values, false);
            self.selection.refresh(&self.drawing);
        } else {
            self.font_browser = Some(browser);
        }
    }
}

#[cfg(test)]
mod rich_tests {
    use super::*;
    use crate::{
        canvas::selection_ui::tests::{frame, moved, pointer, screen, setup},
        document::DocumentKind,
        text::{TEXT_HANDLES, TextFamily, TextStylePatch},
    };
    pub(super) fn key(k: egui::Key, command: bool) -> Event {
        Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: command,
                command,
                ..Default::default()
            },
        }
    }
    pub(super) fn selected(ctx: &egui::Context, c: &mut Canvas, a: usize, b: usize) {
        let s = c.text_session.as_mut().unwrap();
        let range =
            egui::text::CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b));
        s.cursor = Some(range);
        s.focus = true;
        let id = egui::Id::unique(("text-editor", s.value.id));
        let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(range));
        state.store(ctx, id);
    }
    pub(super) fn start(ctx: &egui::Context, c: &mut Canvas, t: TextObject) {
        let mut output = ctx.run_ui(Default::default(), |ui| c.start_text(ui, t.clone(), false));
        output.textures_delta.clear();
    }
    #[test]
    fn font_preview_navigation_cancel_and_single_commit_preserve_selected_characters() {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        text_layout::install(&ctx);
        let mut t = TextObject::new(c.drawing.owner(), s, [40., 160.], 240.);
        t.content_mut().text = "Hello beautiful world".into();
        t.content_mut().format(
            8..12,
            &TextStylePatch {
                bold: Some(true),
                ..Default::default()
            },
        );
        c.drawing.replace_texts(vec![t.clone()], false);
        start(&ctx, &mut c, t.clone());
        selected(&ctx, &mut c, 6, 15);
        let revision = c.drawing.revision();
        c.open_font_browser(&[t.clone()]);
        let render = |c: &mut Canvas, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    events,
                    focused: true,
                    ..Default::default()
                },
                |ui| c.font_browser_ui(ui),
            );
            out.textures_delta.clear();
        };
        render(&mut c, vec![key(egui::Key::ArrowDown, false)]);
        let b = c.font_browser.as_ref().unwrap();
        assert_ne!(
            b.previews[0].content.style_at(7).family,
            t.content.style_at(7).family
        );
        assert_eq!(b.previews[0].content.style_at(0), t.content.style_at(0));
        assert!(b.previews[0].content.style_at(9).bold);
        assert_eq!(c.drawing.revision(), revision);
        render(&mut c, vec![key(egui::Key::Escape, false)]);
        assert!(c.font_browser.is_none());
        assert_eq!(c.drawing.snapshot(t.id), Some(&t.clone().into()));
        c.open_font_browser(&[t.clone()]);
        render(&mut c, vec![key(egui::Key::ArrowDown, false)]);
        render(&mut c, vec![key(egui::Key::Enter, false)]);
        assert!(c.font_browser.is_none());
        assert_eq!(c.drawing.revision(), revision + 1);
        let formatted = c.drawing.snapshot(t.id).cloned().unwrap();
        c.finish_text();
        c.undo();
        assert_eq!(c.drawing.snapshot(t.id), Some(&t.into()));
        c.redo();
        assert_eq!(c.drawing.snapshot(formatted.id()), Some(&formatted));
    }
    #[test]
    fn real_editor_pastes_rich_runs_and_undo_restores_them() {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        text_layout::install(&ctx);
        let view = crate::canvas::selection_ui::tests::t();
        let mut t = TextObject::new(c.drawing.owner(), s, [40., 160.], 240.);
        t.content_mut().text = "Hello beautiful world".into();
        t.content_mut().format(
            6..15,
            &TextStylePatch {
                size: Some(32.),
                family: Some(TextFamily::Monospace),
                rgb: Some([180, 0, 30]),
                ..Default::default()
            },
        );
        c.drawing.replace_texts(vec![t.clone()], false);
        start(&ctx, &mut c, t.clone());
        selected(&ctx, &mut c, 6, 15);
        frame(&ctx, &mut c, s, view, vec![Event::Copy], 0.2, true);
        let rich = ctx
            .data(|d| d.get_temp::<TextContent>(egui::Id::unique("XiToc-rich-clipboard")))
            .unwrap();
        assert_eq!(rich.text, "beautiful");
        assert_eq!(rich.style_at(0).size, 32.);
        selected(&ctx, &mut c, 21, 21);
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![Event::Paste("beautiful".into())],
            0.3,
            true,
        );
        let Some(ObjectSnapshot::Text(pasted)) = c.drawing.snapshot(t.id) else {
            panic!()
        };
        assert_eq!(pasted.content.text, "Hello beautiful worldbeautiful");
        assert_eq!(pasted.content.style_at(25), t.content.style_at(8));
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![key(egui::Key::Z, true)],
            0.4,
            true,
        );
        assert_eq!(c.drawing.snapshot(t.id), Some(&t.clone().into()));
        selected(&ctx, &mut c, 0, 0);
        let mut navigation = vec![key(egui::Key::ArrowRight, false); 6];
        for _ in 0..9 {
            let mut event = key(egui::Key::ArrowRight, false);
            if let Event::Key { modifiers, .. } = &mut event {
                modifiers.shift = true;
            }
            navigation.push(event);
        }
        frame(&ctx, &mut c, s, view, navigation, 0.5, true);
        assert_eq!(c.text_session.as_ref().unwrap().typing.size, 32.);
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![Event::Text("new".into())],
            0.6,
            true,
        );
        let Some(ObjectSnapshot::Text(replaced)) = c.drawing.snapshot(t.id) else {
            panic!()
        };
        assert_eq!(replaced.content.text, "Hello new world");
        assert_eq!(replaced.content.style_at(7).size, 32.);
        assert_eq!(replaced.content.style_at(1).size, 18.);
    }
    #[test]
    fn all_local_handles_use_one_stable_release_transaction_across_zoom_and_rotation() {
        for scale in [0.5, 1., 2., 4.] {
            for rotation in [0., 0.4] {
                for (handle, h) in TEXT_HANDLES.iter().enumerate() {
                    let (mut c, s) = setup(DocumentKind::Whiteboard);
                    let ctx = egui::Context::default();
                    text_layout::install(&ctx);
                    let view = Transform {
                        scale,
                        origin: [20.; 2],
                        center: [80.; 2],
                    };
                    let mut t = TextObject::new(c.drawing.owner(), s, [100., 100.], 140.);
                    t.height = 60.;
                    t.content_mut().text = "resize".into();
                    t.placement.rotation = rotation;
                    c.drawing.replace_texts(vec![t.clone()], false);
                    c.select(Tool::Select);
                    c.selection.change([t.id], Membership::Replace, &c.drawing);

                    let p = t
                        .placement
                        .apply([t.width * h[0] + 0.5, t.height * h[1] + 0.5]);
                    let end = t
                        .placement
                        .apply([t.width * h[0] + 10.5, t.height * h[1] + 10.5]);
                    let revision = c.drawing.revision();
                    frame(
                        &ctx,
                        &mut c,
                        s,
                        view,
                        vec![
                            moved(view, p),
                            pointer(screen(view, p), true, Default::default()),
                            moved(view, end),
                            Event::PointerGone,
                            moved(view, p),
                            moved(view, end),
                        ],
                        0.1,
                        true,
                    );
                    assert!(c.text_resize.is_some(), "handle {handle}, zoom {scale}");
                    assert_eq!(c.drawing.revision(), revision);
                    frame(
                        &ctx,
                        &mut c,
                        s,
                        view,
                        vec![pointer(screen(view, end), false, Default::default())],
                        0.2,
                        true,
                    );
                    assert!(c.text_resize.is_none());
                    assert_eq!(c.drawing.revision(), revision + 1);
                    c.undo();
                    assert_eq!(c.drawing.snapshot(t.id), Some(&t.into()));
                }
            }
        }
    }
    #[test]
    fn real_format_menu_preserves_word_selection_and_caret_typing_style() {
        use crate::ui::settings::tests::{pointer as ui_pointer, render};
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        text_layout::install(&ctx);
        let mut t = TextObject::new(c.drawing.owner(), s, [40., 160.], 240.);
        t.content_mut().text = "Hello beautiful world".into();
        c.drawing.replace_texts(vec![t.clone()], false);
        start(&ctx, &mut c, t.clone());
        selected(&ctx, &mut c, 6, 15);
        render(&ctx, vec![], |ui| c.text_format_controls(ui));
        let labels = render(&ctx, vec![], |ui| c.text_format_controls(ui));
        let p = labels["Text"];
        render(&ctx, ui_pointer(p, true), |ui| c.text_format_controls(ui));
        render(&ctx, ui_pointer(p, false), |ui| c.text_format_controls(ui));
        let labels = render(&ctx, vec![], |ui| c.text_format_controls(ui));
        assert!(labels.contains_key("Bold"), "{labels:?}");
        let p = labels["Bold"];
        render(&ctx, ui_pointer(p, true), |ui| c.text_format_controls(ui));
        render(&ctx, ui_pointer(p, false), |ui| c.text_format_controls(ui));
        let Some(ObjectSnapshot::Text(value)) = c.drawing.snapshot(t.id) else {
            panic!()
        };
        assert!(value.content.style_at(8).bold);
        assert!(!value.content.style_at(0).bold && !value.content.style_at(18).bold);
        assert_eq!(
            c.text_session
                .as_ref()
                .unwrap()
                .cursor
                .unwrap()
                .sorted_cursors()[0]
                .index
                .0,
            6
        );
        assert!(c.text_editing());
    }
    #[test]
    fn canvas_overlay_allows_context_menu_interaction_while_editing() {
        use crate::ui::settings::tests::{pointer as ui_pointer, render};
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        text_layout::install(&ctx);
        let mut t = TextObject::new(c.drawing.owner(), s, [200., 200.], 240.);
        t.content_mut().text = "First\nHello beautiful world".into();
        t.height = 70.;
        t.content_mut().format_paragraphs(6..27, |p| {
            p.alignment = Alignment::Center;
            p.line_spacing = 1.75;
            p.spacing = 12.;
        });
        c.drawing.replace_texts(vec![t.clone()], false);
        start(&ctx, &mut c, t);
        selected(&ctx, &mut c, 12, 21);
        let mut draw = |events| {
            render(&ctx, events, |ui| {
                c.begin_frame(ui);
                c.surface(
                    ui,
                    Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.)),
                    s,
                    crate::canvas::selection_ui::tests::t(),
                );
                c.end_frame(ui);
            })
        };
        draw(vec![]);
        let labels = draw(vec![]);
        let p = labels["Text"];
        draw(ui_pointer(p, true));
        draw(ui_pointer(p, false));
        let labels = draw(vec![]);
        assert!(labels.contains_key("Bold"), "{labels:?}");
        draw(ui_pointer(p, true));
        draw(ui_pointer(p, false));
        let labels = draw(vec![]);
        let p = labels["Paragraph"];
        draw(ui_pointer(p, true));
        draw(ui_pointer(p, false));
        let labels = draw(vec![]);
        assert!(labels.keys().any(|s| s.contains("1.75")), "{labels:?}");
        assert!(labels.keys().any(|s| s == "12"), "{labels:?}");
    }
    #[test]
    fn caret_formatting_changes_only_subsequent_input() {
        use crate::ui::settings::tests::{pointer as ui_pointer, render};
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        text_layout::install(&ctx);
        let mut t = TextObject::new(c.drawing.owner(), s, [40., 160.], 240.);
        t.content_mut().text = "Hello world".into();
        c.drawing.replace_texts(vec![t.clone()], false);
        start(&ctx, &mut c, t.clone());
        selected(&ctx, &mut c, 6, 6);
        let revision = c.drawing.revision();
        render(&ctx, vec![], |ui| c.text_format_controls(ui));
        let labels = render(&ctx, vec![], |ui| c.text_format_controls(ui));
        let p = labels["Text"];
        render(&ctx, ui_pointer(p, true), |ui| c.text_format_controls(ui));
        render(&ctx, ui_pointer(p, false), |ui| c.text_format_controls(ui));
        let labels = render(&ctx, vec![], |ui| c.text_format_controls(ui));
        let p = labels["Bold"];
        render(&ctx, ui_pointer(p, true), |ui| c.text_format_controls(ui));
        render(&ctx, ui_pointer(p, false), |ui| c.text_format_controls(ui));
        assert_eq!(c.drawing.revision(), revision);
        assert!(c.text_session.as_ref().unwrap().typing.bold);
        egui::Popup::close_all(&ctx);
        frame(
            &ctx,
            &mut c,
            s,
            crate::canvas::selection_ui::tests::t(),
            vec![],
            0.9,
            true,
        );
        frame(
            &ctx,
            &mut c,
            s,
            crate::canvas::selection_ui::tests::t(),
            vec![Event::Text("new ".into())],
            1.,
            true,
        );
        let Some(ObjectSnapshot::Text(v)) = c.drawing.snapshot(t.id) else {
            panic!()
        };
        assert_eq!(v.content.text, "Hello new world");
        assert!(v.content.style_at(7).bold);
        assert!(!v.content.style_at(0).bold && !v.content.style_at(12).bold);
    }
    #[test]
    fn changing_width_reflows_cached_display_without_changing_runs() {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        text_layout::install(&ctx);
        let mut t = TextObject::new(c.drawing.owner(), s, [40., 160.], 240.);
        t.content_mut().text = "alpha beta gamma delta epsilon zeta eta theta".into();
        let content = t.content.clone();
        c.drawing.replace_texts(vec![t.clone()], false);
        c.select(Tool::Select);
        let view = crate::canvas::selection_ui::tests::t();
        frame(&ctx, &mut c, s, view, vec![], 0., true);
        let before = c.text_cache[&t.id].galley.clone();
        t.width = 160.;
        text_layout::measure(&ctx, &mut t);
        c.drawing.replace_texts(vec![t.clone()], false);
        frame(&ctx, &mut c, s, view, vec![], 0.1, true);
        let after = &c.text_cache[&t.id].galley;
        assert!(!Arc::ptr_eq(&before, after));
        assert!(after.rows.len() > before.rows.len());
        assert_eq!(t.content, content);
    }
    #[test]
    fn global_resize_default_is_captured_by_future_boxes_without_changing_existing_objects() {
        let (mut c, s) = setup(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        text_layout::install(&ctx);
        let mut old = TextObject::new(c.drawing.owner(), s, [20., 160.], 100.);
        old.content_mut().text = "Existing".into();
        c.drawing.replace_texts(vec![old.clone()], false);
        let revision = c.drawing.revision();
        let mut prefs = crate::settings::Preferences::default();
        prefs.text_resize = crate::text::ResizeBehavior::Scale;
        prefs.apply_drawing(&mut c);
        assert_eq!(c.drawing.revision(), revision);
        assert_eq!(c.drawing.snapshot(old.id), Some(&old.into()));
        let view = crate::canvas::selection_ui::tests::t();
        c.select(Tool::Text);
        frame(
            &ctx,
            &mut c,
            s,
            view,
            vec![
                moved(view, [300., 200.]),
                pointer([300., 200.], true, Default::default()),
                pointer([300., 200.], false, Default::default()),
            ],
            0.,
            true,
        );
        assert_eq!(
            c.text_session.as_ref().unwrap().value.layout.resize,
            crate::text::ResizeBehavior::Scale
        );
    }
}

#[cfg(test)]
#[path = "text_format_tests.rs"]
mod formatting_tests;
