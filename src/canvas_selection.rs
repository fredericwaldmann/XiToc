//! Selection input/preview adapter for the existing shared canvas controller.
use super::*;
use crate::ui::style;
use crate::{
    object::{ObjectBounds, Placement, Point},
    object_clipboard::ObjectBundle,
    selection::{self, Membership, SelectionCommand},
};

pub(super) enum SelectionGesture {
    Lasso {
        surface: Surface,
        points: Vec<Point>,
        mode: Membership,
        overflow: bool,
    },
    Transform {
        surface: Surface,
        start: Point,
        bounds: ObjectBounds,
        geometry: ObjectBounds,
        corner: Option<usize>,
        delta: Placement,
        moved: bool,
        limits: (f64, f64),
    },
}
#[derive(Clone)]
struct SessionClipboard {
    bundle: Arc<ObjectBundle>,
    count: u32,
}
fn clipboard_id() -> egui::Id {
    egui::Id::unique("XiToc-object-clipboard")
}

impl Canvas {
    pub fn selection_action(&mut self, command: SelectionCommand, ctx: &egui::Context) {
        use SelectionCommand::*;
        match command {
            Undo => self.undo(),
            Redo => self.redo(),
            SelectAll => {
                self.select(Tool::Select);
                let ids = self
                    .drawing
                    .strokes()
                    .iter()
                    .filter(|s| s.surface == self.selection.surface)
                    .map(|s| s.id)
                    .collect::<Vec<_>>();
                self.selection
                    .change(ids, Membership::Replace, &self.drawing);
            }
            Copy => {
                self.finish();
                if let Some(bundle) = self.selected_bundle() {
                    match bundle.encode() {
                        Ok(text) => {
                            ctx.copy_text(text);
                            ctx.data_mut(|d| {
                                d.insert_temp(
                                    clipboard_id(),
                                    SessionClipboard {
                                        bundle: Arc::new(bundle),
                                        count: 0,
                                    },
                                )
                            });
                            self.notice = Some(
                                "Copied objects · XiToc session clipboard also available".into(),
                            );
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
            }
            Paste => {
                self.request_paste(ctx);
            }
            Duplicate => {
                self.finish();
                if let Some(bundle) = self.selected_bundle() {
                    let ids = bundle.duplicate(&mut self.drawing, self.selection.surface);
                    self.selection
                        .change(ids, Membership::Replace, &self.drawing);
                    self.tool = Tool::Select;
                    self.prune_cache();
                }
            }
            Delete => {
                self.finish();
                self.drawing.erase(&self.selection.ids);
                self.selection.clear();
                self.prune_cache();
            }
            Smaller | Larger => {
                self.finish();
                if let (Some(bounds), Some(geometry)) =
                    (self.selection.bounds, self.selection.geometry)
                {
                    let p = selection::scale_about(
                        geometry,
                        bounds.center(),
                        if command == Smaller { 0.9 } else { 1.1 },
                        self.drawing.bounds(self.selection.surface),
                    );
                    self.drawing.transform_objects(&self.selection.ids, p);
                    self.prune_cache();
                }
            }
        }
    }
    fn request_paste(&mut self, ctx: &egui::Context) {
        self.finish();
        // Web has no RequestPaste viewport implementation. DOM Paste events
        // take precedence in selection_keys; otherwise use our explicit local
        // clipboard synchronously, rather than waiting on an unsupported API.
        #[cfg(target_arch = "wasm32")]
        self.paste_session(ctx);
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pending_paste = Some(ctx.input(|i| i.time) + 0.2);
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            ctx.request_repaint_after(std::time::Duration::from_millis(210));
        }
    }
    fn paste_session(&mut self, ctx: &egui::Context) {
        if let Some(mut clipboard) = ctx.data(|d| d.get_temp::<SessionClipboard>(clipboard_id())) {
            self.notice = Some("Pasted objects from XiToc session clipboard".into());
            self.paste_bundle(clipboard.bundle.clone(), clipboard.count);
            clipboard.count += 1;
            ctx.data_mut(|d| d.insert_temp(clipboard_id(), clipboard));
        } else {
            self.notice = Some("No XiToc session copy available. Copy a selection first, or use Ctrl/Cmd+V for a system clipboard payload.".into());
        }
    }
    fn selected_bundle(&mut self) -> Option<ObjectBundle> {
        if self.selection.ids.is_empty() {
            return None;
        }
        let selected = self
            .drawing
            .strokes()
            .iter()
            .filter(|s| self.selection.ids.contains(&s.id));
        let mut count = 0;
        let mut points = 0;
        for s in selected {
            count += 1;
            points += s.points.len();
            if count > crate::object_clipboard::MAX_OBJECTS
                || points > crate::object_clipboard::MAX_POINTS
            {
                self.notice = Some(
                    "Selection exceeds clipboard limit (512 objects / 200,000 points).".into(),
                );
                return None;
            }
        }
        match ObjectBundle::capture(
            self.drawing
                .strokes()
                .iter()
                .filter(|s| self.selection.ids.contains(&s.id))
                .map(|s| (**s).clone()),
        ) {
            Ok(bundle) => Some(bundle),
            Err(e) => {
                self.notice = Some(e);
                None
            }
        }
    }
    fn paste_bundle(&mut self, bundle: Arc<ObjectBundle>, count: u32) {
        self.finish();
        let target = self
            .selection_view
            .filter(|(s, _, _)| *s == self.selection.surface)
            .map(|(_, t, b)| t.to_document([b.center().x as f64, b.center().y as f64]))
            .unwrap_or([80.0, 80.0]);
        let offset = f64::from(count.min(20)) * 12.0;
        let ids = bundle.paste(
            &mut self.drawing,
            self.selection.surface,
            target.map(|v| v + offset),
        );
        if ids.is_empty() {
            self.notice = Some(
                "Cannot fit these objects into the active scope within the supported scale range."
                    .into(),
            );
            return;
        }
        self.selection
            .change(ids, Membership::Replace, &self.drawing);
        self.tool = Tool::Select;
        self.prune_cache();
    }
    pub(super) fn selection_keys(&mut self, ui: &egui::Ui) {
        if egui::Popup::is_any_open(ui.ctx()) {
            return;
        }
        let events = ui.input(|i| i.events.clone());
        let mut copied = false;
        let mut pasted = false;
        for event in &events {
            match event {
                Event::Copy => {
                    self.selection_action(SelectionCommand::Copy, ui.ctx());
                    copied = true;
                }
                Event::Paste(text) => {
                    pasted = true;
                    self.pending_paste = None;
                    match ObjectBundle::decode(text) {
                        Ok(bundle) => {
                            let bundle = Arc::new(bundle);
                            self.paste_bundle(bundle.clone(), 0);
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(clipboard_id(), SessionClipboard { bundle, count: 1 })
                            });
                        }
                        Err(e) => self.notice = Some(e),
                    }
                }
                _ => {}
            }
        }
        let shortcut = |key| egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, key);
        for (key, command) in [
            (egui::Key::A, SelectionCommand::SelectAll),
            (egui::Key::D, SelectionCommand::Duplicate),
            (egui::Key::C, SelectionCommand::Copy),
            (egui::Key::V, SelectionCommand::Paste),
        ] {
            if ui.input_mut(|i| i.consume_shortcut(&shortcut(key)))
                && !(copied && key == egui::Key::C)
                && !(pasted && key == egui::Key::V)
            {
                if command == SelectionCommand::Paste {
                    self.request_paste(ui.ctx());
                } else {
                    self.selection_action(command, ui.ctx());
                }
            }
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            // Cancel first so a later pointer release cannot commit a preview.
            self.selection_gesture = None;
            self.selection.clear();
        }
        if matches!(self.tool, Tool::Select | Tool::Lasso)
            && ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)
            })
        {
            self.selection_action(SelectionCommand::Delete, ui.ctx());
        }
        if self
            .pending_paste
            .is_some_and(|deadline| ui.input(|i| i.time) >= deadline)
        {
            self.pending_paste = None;
            self.paste_session(ui.ctx());
        }
    }
    pub(super) fn selection_preview(&self, surface: Surface) -> Option<Placement> {
        match self.selection_gesture {
            Some(SelectionGesture::Transform {
                surface: s,
                delta,
                moved: true,
                ..
            }) if surface == s => Some(delta),
            _ => None,
        }
    }
    fn pick(&mut self, surface: Surface, point: Point, tolerance: f64) -> Option<Uuid> {
        self.eraser_index
            .sync(self.drawing.revision(), self.drawing.strokes());
        let mut top = None;
        self.eraser_index
            .query(surface, point, point, tolerance, |index| {
                if top.is_some_and(|(old, _)| old > index) {
                    return;
                }
                let stroke = &self.drawing.strokes()[index];
                let cache = self
                    .cache
                    .entry(stroke.id)
                    .or_insert_with(|| Cached::new(stroke));
                if pressure_hit(
                    &cache.path,
                    &cache.pressures,
                    stroke,
                    point,
                    point,
                    tolerance,
                ) {
                    top = Some((index, stroke.id));
                }
            });
        top.map(|(_, id)| id)
    }
    fn display_bounds(mut bounds: ObjectBounds, scale: f64) -> ObjectBounds {
        let center = bounds.center();
        for (i, c) in center.into_iter().enumerate() {
            if (bounds.max[i] - bounds.min[i]) * scale < f64::from(style::SELECTION_MIN) {
                bounds.min[i] = c - f64::from(style::SELECTION_MIN) * 0.5 / scale;
                bounds.max[i] = c + f64::from(style::SELECTION_MIN) * 0.5 / scale;
            }
        }
        bounds
    }
    fn handle(&self, point: Point, scale: f64) -> Option<usize> {
        if self.tool != Tool::Select {
            return None;
        }
        Self::display_bounds(self.selection.bounds?, scale)
            .corners()
            .iter()
            .position(|c| {
                (c[0] - point[0]).hypot(c[1] - point[1]) * scale <= f64::from(style::SELECTION_HIT)
            })
    }
    fn start_transform(
        &mut self,
        surface: Surface,
        point: Point,
        corner: Option<usize>,
        scale: f64,
    ) {
        if let (Some(bounds), Some(geometry)) = (self.selection.bounds, self.selection.geometry) {
            let bounds = if corner.is_some() {
                Self::display_bounds(bounds, scale)
            } else {
                bounds
            };
            let mut limits = (0.0001_f64, 10000.0_f64);
            for s in self
                .drawing
                .strokes()
                .iter()
                .filter(|s| self.selection.ids.contains(&s.id))
            {
                limits.0 = limits.0.max(1e-4 / s.placement.scale);
                limits.1 = limits.1.min(1e4 / s.placement.scale);
            }
            self.selection_gesture = Some(SelectionGesture::Transform {
                surface,
                start: point,
                bounds,
                geometry,
                corner,
                delta: Placement::default(),
                moved: false,
                limits,
            });
        }
    }
    fn selection_sample(&mut self, surface: Surface, point: Point, scale: f64) {
        match &mut self.selection_gesture {
            Some(SelectionGesture::Lasso {
                surface: s,
                points,
                overflow,
                ..
            }) if *s == surface => {
                if points
                    .last()
                    .is_none_or(|p| (p[0] - point[0]).hypot(p[1] - point[1]) * scale >= 1.5)
                {
                    if points.len() < 4096 {
                        points.push(point);
                    } else {
                        *overflow = true;
                    }
                }
            }
            Some(SelectionGesture::Transform {
                surface: s,
                start,
                bounds,
                geometry,
                corner,
                delta,
                moved,
                limits,
            }) if *s == surface => {
                let movement = [point[0] - start[0], point[1] - start[1]];
                if !*moved && movement[0].hypot(movement[1]) * scale < 3.0 {
                    return;
                }
                *moved = true;
                *delta = if let Some(index) = corner {
                    let corners = bounds.corners();
                    let anchor = corners[(*index + 2) % 4];
                    let corner = corners[*index];
                    let direction = [corner[0] - anchor[0], corner[1] - anchor[1]];
                    let factor = (1.0
                        + (movement[0] * direction[0] + movement[1] * direction[1])
                            / (direction[0].powi(2) + direction[1].powi(2)).max(1e-12))
                    .clamp(limits.0, limits.1);
                    selection::scale_about(*geometry, anchor, factor, self.drawing.bounds(surface))
                } else {
                    selection::move_by(*bounds, movement, self.drawing.bounds(surface))
                };
            }
            _ => {}
        }
    }
    fn commit_selection(&mut self) {
        match self.selection_gesture.take() {
            Some(SelectionGesture::Transform {
                delta, moved: true, ..
            }) => {
                self.drawing.transform_objects(&self.selection.ids, delta);
                self.prune_cache();
            }
            Some(SelectionGesture::Lasso {
                surface,
                points,
                mode,
                overflow,
            }) => {
                if overflow {
                    self.notice = Some(
                        "Lasso is too complex; retry with a shorter outline (4096 points maximum)."
                            .into(),
                    );
                    return;
                }
                let Some(region) = selection::LassoRegion::new(points) else {
                    return;
                };
                let bounds = region.bounds;
                self.eraser_index
                    .sync(self.drawing.revision(), self.drawing.strokes());
                let mut selected = Vec::new();
                self.eraser_index
                    .query_rect(surface, bounds.min, bounds.max, |index| {
                        let stroke = &self.drawing.strokes()[index];
                        let cached = self
                            .cache
                            .entry(stroke.id)
                            .or_insert_with(|| Cached::new(stroke));
                        if region.encloses(
                            &cached.path,
                            &cached.pressures,
                            stroke.visible_width(),
                            stroke.pressure_style,
                        ) {
                            selected.push(stroke.id);
                        }
                    });
                self.selection.change(selected, mode, &self.drawing);
                self.tool = Tool::Select;
            }
            _ => {}
        }
    }
    pub(super) fn selection_surface(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        transform: Transform,
    ) {
        if self.selection.surface == surface {
            if self.selection_gesture.is_some()
                && self
                    .selection_view
                    .is_some_and(|(_, old, _)| old != transform)
            {
                self.selection_gesture = None;
            }
            self.selection_view = Some((surface, transform, bounds.intersect(ui.clip_rect())));
        }
        self.selection_controls(ui, bounds, surface, transform);
        if self.processed.contains(&surface) {
            return;
        }
        self.processed.push(surface);
        let selection_tool = self.tool == Tool::Select;
        let owns = |p: Pos2| {
            (bounds.contains(p)
                || (selection_tool && bounds.expand(style::SELECTION_HIT).contains(p)))
                && ui.clip_rect().contains(p)
                && ui.is_enabled()
                && ui.input(|i| i.focused)
                && ui.ctx().layer_id_at(p) == Some(ui.layer_id())
        };
        for event in ui.input(|i| i.events.clone()) {
            match event {
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } if owns(pos) && !ui.input(|i| i.key_down(egui::Key::Space)) => {
                    let point = transform.to_document([pos.x as f64, pos.y as f64]);
                    let corner = if self.selection.surface == surface {
                        self.handle(point, transform.scale)
                    } else {
                        None
                    };
                    if !self.drawing.contains(surface, point) && corner.is_none() {
                        continue;
                    }
                    self.selection.scope(surface);
                    self.selection_view =
                        Some((surface, transform, bounds.intersect(ui.clip_rect())));
                    if let Some(corner) = corner {
                        self.start_transform(surface, point, Some(corner), transform.scale);
                        continue;
                    }
                    let mode = if modifiers.alt {
                        Membership::Subtract
                    } else if modifiers.shift {
                        Membership::Toggle
                    } else {
                        Membership::Replace
                    };
                    if self.tool == Tool::Lasso {
                        self.selection_gesture = Some(SelectionGesture::Lasso {
                            surface,
                            points: vec![point],
                            mode: if mode == Membership::Toggle {
                                Membership::Add
                            } else {
                                mode
                            },
                            overflow: false,
                        });
                    } else if let Some(id) = self.pick(surface, point, 4.0 / transform.scale) {
                        if mode != Membership::Replace {
                            self.selection.change([id], mode, &self.drawing);
                        } else {
                            if !self.selection.ids.contains(&id) {
                                self.selection.change([id], mode, &self.drawing);
                            }
                            self.start_transform(surface, point, None, transform.scale);
                        }
                    } else if mode == Membership::Replace {
                        self.selection.clear();
                    }
                }
                Event::PointerMoved(pos) => self.selection_sample(
                    surface,
                    transform.to_document([pos.x as f64, pos.y as f64]),
                    transform.scale,
                ),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                } => {
                    let active = match &self.selection_gesture {
                        Some(
                            SelectionGesture::Lasso { surface: s, .. }
                            | SelectionGesture::Transform { surface: s, .. },
                        ) => *s == surface,
                        _ => false,
                    };
                    if active {
                        self.selection_sample(
                            surface,
                            transform.to_document([pos.x as f64, pos.y as f64]),
                            transform.scale,
                        );
                        self.commit_selection();
                    }
                }
                Event::PointerGone
                | Event::Touch {
                    phase: egui::TouchPhase::Cancel,
                    ..
                } => self.selection_gesture = None,
                _ => {}
            }
        }
    }
    pub(super) fn paint_selection(
        &self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        transform: Transform,
    ) {
        let painter = ui
            .painter()
            .with_clip_rect(bounds.intersect(ui.clip_rect()));
        let p = crate::ui::style::palette(ui);
        let screen = |point: Point| {
            let point = transform.to_screen(point);
            egui::pos2(point[0] as f32, point[1] as f32)
        };
        if let Some(SelectionGesture::Lasso {
            surface: s, points, ..
        }) = &self.selection_gesture
            && *s == surface
            && points.len() > 1
        {
            let mut line: Vec<_> = points.iter().copied().map(screen).collect();
            line.push(line[0]);
            painter.add(egui::Shape::line(line, Line::new(1.25, p.accent)));
        }
        if self.selection.surface != surface {
            return;
        }
        let delta = self.selection_preview(surface).unwrap_or_default();
        for (_, object) in &self.selection.objects {
            let object = object.placed(delta);
            painter.rect_stroke(
                Rect::from_min_max(screen(object.min), screen(object.max)),
                2.0,
                Line::new(0.75, p.accent.gamma_multiply(0.55)),
                egui::StrokeKind::Outside,
            );
        }
        if let Some(group) = self.selection.bounds {
            let group = Self::display_bounds(group.placed(delta), transform.scale);
            painter.rect_stroke(
                Rect::from_min_max(screen(group.min), screen(group.max)),
                0.0,
                Line::new(1.0, p.accent),
                egui::StrokeKind::Outside,
            );
            for corner in group
                .corners()
                .into_iter()
                .filter(|_| self.tool == Tool::Select)
            {
                painter.rect(
                    Rect::from_center_size(
                        screen(corner),
                        egui::Vec2::splat(style::SELECTION_HANDLE),
                    ),
                    1.0,
                    p.surface,
                    Line::new(1.0, p.accent),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if let Some(pos) = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| bounds.contains(*p))
        {
            if ui.input(|i| i.key_down(egui::Key::Space)) {
                return;
            }
            let point = transform.to_document([pos.x as f64, pos.y as f64]);
            let cursor = if let Some(index) = self.handle(point, transform.scale) {
                if index % 2 == 0 {
                    egui::CursorIcon::ResizeNwSe
                } else {
                    egui::CursorIcon::ResizeNeSw
                }
            } else if self.selection_preview(surface).is_some() {
                egui::CursorIcon::Grabbing
            } else if self.tool == Tool::Lasso {
                egui::CursorIcon::Crosshair
            } else {
                egui::CursorIcon::Default
            };
            if ui.ctx().layer_id_at(pos) == Some(ui.layer_id()) {
                ui.ctx().set_cursor_icon(cursor);
            }
        }
    }
    fn selection_controls(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        transform: Transform,
    ) {
        use crate::ui::{
            icons::{self, Icon},
            style,
        };
        let viewport = bounds.intersect(ui.clip_rect());
        let p = style::palette(ui);
        if self.tool == Tool::Select
            && self.selection.surface == surface
            && self.selection_gesture.is_none()
            && let Some(group) = self.selection.bounds
        {
            let top = transform.to_screen([group.center()[0], group.min[1]]);
            let bottom = transform.to_screen([group.center()[0], group.max[1]]);
            let rect = Rect::from_min_max(
                egui::pos2(transform.to_screen(group.min)[0] as f32, top[1] as f32),
                egui::pos2(transform.to_screen(group.max)[0] as f32, bottom[1] as f32),
            );
            if rect.intersects(viewport) {
                let y = if top[1] as f32 - 36.0 >= viewport.top() {
                    top[1] as f32 - 36.0
                } else {
                    (bottom[1] as f32 + 8.0).min(viewport.bottom() - 34.0)
                };
                egui::Area::new(
                    egui::Id::unique("selection-context")
                        .with(self.drawing.owner())
                        .with(surface),
                )
                .order(egui::Order::Foreground)
                .movable(false)
                .enabled(ui.is_enabled())
                .constrain_to(viewport)
                .pivot(egui::Align2::CENTER_TOP)
                .fixed_pos(egui::pos2(top[0] as f32, y))
                .show(ui.ctx(), |ui| {
                    style::frame(p.surface, 4, 3)
                        .stroke(Line::new(1.0, p.border))
                        .corner_radius(6)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                for (icon, label, cmd) in [
                                    (
                                        Icon::Duplicate,
                                        "Duplicate · Ctrl/Cmd+D",
                                        SelectionCommand::Duplicate,
                                    ),
                                    (Icon::Delete, "Delete selection", SelectionCommand::Delete),
                                    (
                                        Icon::Minus,
                                        "Scale selection 90%",
                                        SelectionCommand::Smaller,
                                    ),
                                    (Icon::Plus, "Scale selection 110%", SelectionCommand::Larger),
                                ] {
                                    if icons::button(ui, icon, label, false).clicked() {
                                        self.selection_action(cmd, ui.ctx());
                                    }
                                }
                            });
                        });
                });
            }
        }
        let response = ui.interact(
            viewport,
            egui::Id::unique("canvas-context")
                .with(self.drawing.owner())
                .with(surface),
            egui::Sense::click(),
        );
        if response.secondary_clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.selection.scope(surface);
            let point = transform.to_document([pos.x as f64, pos.y as f64]);
            if let Some(id) = self.pick(surface, point, 4.0 / transform.scale)
                && !self.selection.ids.contains(&id)
            {
                self.selection
                    .change([id], Membership::Replace, &self.drawing);
            }
        }
        response.context_menu(|ui| {
            let state = crate::ui::selection_controls::MenuState::for_canvas(self);
            if let Some(command) = crate::ui::selection_controls::menu(ui, state) {
                self.selection_action(command, ui.ctx());
                ui.close();
            }
        });
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
