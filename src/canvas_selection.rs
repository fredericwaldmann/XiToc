//! Selection input/preview adapter for the existing shared canvas controller.
use super::*;
use crate::ui::style;
use crate::{
    object::{ObjectBounds, Placement, Point},
    object_clipboard::ObjectBundle,
    selection::{self, Membership, SelectionCommand},
};

/// Immutable gesture ownership, independent of visible membership and clipping.
/// Snapshot clones retain Arcs; no document or point-vector clones per update.
pub(super) struct DragSession {
    pub(super) ids: HashSet<Uuid>,
    sources: Vec<ObjectSnapshot>,
    view: Transform,
    revision: u64,
}

pub(super) enum SelectionGesture {
    ShapeEdit {
        surface: Surface,
        source: Arc<crate::shape::Shape>,
        preview: crate::shape::Shape,
        index: usize,
        corner: bool,
        start: Point,
        moved: bool,
    },
    Rectangle {
        surface: Surface,
        start: Point,
        end: Point,
        mode: Membership,
        moved: bool,
        matches: Vec<Uuid>,
    },
    Lasso {
        surface: Surface,
        points: Vec<Point>,
        mode: Membership,
        overflow: bool,
    },
    Rotate {
        surface: Surface,
        start: Point,
        center: Point,
        geometry: ObjectBounds,
        last_angle: f64,
        angle: f64,
        delta: Placement,
        moved: bool,
    },
    Transform {
        session: DragSession,
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
                self.select(self.selection_mode);
                let ids = self
                    .drawing
                    .objects()
                    .filter(|s| s.surface() == self.selection.surface)
                    .map(|s| s.object_id())
                    .collect::<Vec<_>>();
                self.selection
                    .change(ids, Membership::Replace, &self.drawing);
            }
            Copy | Cut => {
                self.finish();
                if self.copy_selection(ctx) && command == Cut {
                    self.drawing.erase(&self.selection.ids);
                    self.selection.clear();
                    self.prune_cache();
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
                    self.tool = self.selection_mode;
                    self.prune_cache();
                }
            }
            Delete => {
                self.finish();
                self.drawing.erase(&self.selection.ids);
                self.selection.clear();
                self.prune_cache();
            }
            FlipHorizontal | FlipVertical => {
                self.finish();
                if let Some(bounds) = self.selection.bounds {
                    let center = bounds.center();
                    let delta = Placement {
                        origin: center,
                        position: center,
                        reflected: [command == FlipHorizontal, command == FlipVertical],
                        ..Default::default()
                    };
                    if self.drawing.transform_objects(&self.selection.ids, delta) {
                        self.prune_cache();
                    } else {
                        self.notice = Some("Flip would leave this page or exceed supported coordinates. Move the selection inward first.".into());
                    }
                }
            }
            RotateLeft | RotateRight => {
                self.finish();
                if let (Some(bounds), Some(geometry)) =
                    (self.selection.bounds, self.selection.geometry)
                {
                    let angle = std::f64::consts::PI / 12.0
                        * if command == RotateLeft { -1.0 } else { 1.0 };
                    if let Some(delta) = selection::rotation_about(
                        geometry,
                        bounds.center(),
                        angle,
                        self.drawing.bounds(self.selection.surface),
                    ) {
                        self.drawing.transform_objects(&self.selection.ids, delta);
                        self.prune_cache();
                    } else {
                        self.notice = Some(
                            "Rotation would leave this page. Move the selection inward first."
                                .into(),
                        );
                    }
                }
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
    fn copy_selection(&mut self, ctx: &egui::Context) -> bool {
        let Some(bundle) = self.selected_bundle() else {
            return false;
        };
        match bundle.encode() {
            Ok(text) => {
                // Establish a reliable application copy before a Cut can delete ink.
                ctx.data_mut(|d| {
                    d.insert_temp(
                        clipboard_id(),
                        SessionClipboard {
                            bundle: Arc::new(bundle),
                            count: 0,
                        },
                    )
                });
                ctx.copy_text(text);
                self.notice =
                    Some("Copied objects · XiToc session clipboard also available".into());
                true
            }
            Err(e) => {
                self.notice = Some(e);
                false
            }
        }
    }
    fn nudge_selection(&mut self, delta: Point) {
        if let Some(bounds) = self.selection.bounds {
            let placement =
                selection::move_by(bounds, delta, self.drawing.bounds(self.selection.surface));
            if self
                .drawing
                .transform_objects(&self.selection.ids, placement)
            {
                self.prune_cache();
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
        match ObjectBundle::capture_objects(
            self.drawing
                .items()
                .iter()
                .filter(|o| self.selection.ids.contains(&o.id()))
                .cloned(),
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
        self.tool = self.selection_mode;
        self.prune_cache();
    }
    pub(super) fn selection_keys(&mut self, ui: &egui::Ui) {
        if egui::Popup::is_any_open(ui.ctx()) {
            return;
        }
        let events = ui.input(|i| i.events.clone());
        let mut copied = false;
        let mut cut = false;
        let mut pasted = false;
        for event in &events {
            match event {
                Event::Cut => {
                    self.selection_action(SelectionCommand::Cut, ui.ctx());
                    cut = true;
                }
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
            (egui::Key::X, SelectionCommand::Cut),
            (egui::Key::V, SelectionCommand::Paste),
        ] {
            if ui.input_mut(|i| i.consume_shortcut(&shortcut(key)))
                && !(copied && key == egui::Key::C)
                && !(cut && key == egui::Key::X)
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
            self.cancel_shape_draft();
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
        let modifiers = ui.input(|i| i.modifiers);
        if matches!(self.tool, Tool::Select | Tool::Lasso)
            && self.selection_gesture.is_none()
            && !self.selection.ids.is_empty()
            && !modifiers.alt
            && !modifiers.ctrl
            && !modifiers.command
            && !modifiers.mac_cmd
        {
            for (key, direction) in [
                (egui::Key::ArrowLeft, [-1., 0.]),
                (egui::Key::ArrowRight, [1., 0.]),
                (egui::Key::ArrowUp, [0., -1.]),
                (egui::Key::ArrowDown, [0., 1.]),
            ] {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, key)) {
                    self.nudge_selection(direction.map(|d| d * 10.));
                } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                    self.nudge_selection(direction);
                }
            }
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
            Some(
                SelectionGesture::Transform {
                    surface: s,
                    delta,
                    moved: true,
                    ..
                }
                | SelectionGesture::Rotate {
                    surface: s,
                    delta,
                    moved: true,
                    ..
                },
            ) if surface == s => Some(delta),
            _ => None,
        }
    }
    fn pick(&mut self, surface: Surface, point: Point, tolerance: f64) -> Option<Uuid> {
        self.object_index
            .sync_objects(self.drawing.revision(), self.drawing.items());
        let mut top = None;
        self.object_index
            .query(surface, point, point, tolerance, |index| {
                if top.is_some_and(|(old, _)| old > index) {
                    return;
                }
                let object = &self.drawing.items()[index];
                let hit = match object {
                    ObjectSnapshot::Stroke(stroke) => {
                        let cache = self
                            .cache
                            .entry(stroke.id)
                            .or_insert_with(|| Cached::new(stroke));
                        pressure_hit(
                            &cache.path,
                            &cache.pressures,
                            stroke,
                            point,
                            point,
                            tolerance,
                        )
                    }
                    ObjectSnapshot::Shape(shape) => shape.hit_test(point, tolerance),
                };
                if hit {
                    top = Some((index, object.id()));
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
        if !matches!(self.tool, Tool::Select | Tool::Lasso) || self.single_shape_handles() {
            return None;
        }
        Self::display_bounds(self.selection.bounds?, scale)
            .corners()
            .iter()
            .position(|c| {
                (c[0] - point[0]).hypot(c[1] - point[1]) * scale <= f64::from(style::SELECTION_HIT)
            })
    }
    fn rotation_point(&self, scale: f64) -> Option<Point> {
        let bounds = Self::display_bounds(self.selection.bounds?, scale);
        Some([
            bounds.center()[0],
            bounds.min[1] - f64::from(style::ROTATION_OFFSET) / scale,
        ])
    }
    fn rotation_hit(&self, point: Point, scale: f64) -> bool {
        matches!(self.tool, Tool::Select | Tool::Lasso)
            && self.rotation_point(scale).is_some_and(|p| {
                (p[0] - point[0]).hypot(p[1] - point[1]) * scale <= f64::from(style::SELECTION_HIT)
            })
    }
    fn start_rotation(&mut self, surface: Surface, point: Point) {
        if let (Some(bounds), Some(geometry)) = (self.selection.bounds, self.selection.geometry) {
            let center = bounds.center();
            self.selection_gesture = Some(SelectionGesture::Rotate {
                surface,
                start: point,
                center,
                geometry,
                last_angle: (point[1] - center[1]).atan2(point[0] - center[0]),
                angle: 0.,
                delta: Placement::default(),
                moved: false,
            });
        }
    }
    fn start_transform(
        &mut self,
        surface: Surface,
        point: Point,
        corner: Option<usize>,
        scale: f64,
    ) {
        if let (Some(bounds), Some(geometry)) = (self.selection.bounds, self.selection.geometry) {
            let extent = (bounds.max[0] - bounds.min[0]).max(bounds.max[1] - bounds.min[1]);
            let minimum_factor = if extent > 0.0 {
                (1.0 / extent).min(1.0)
            } else {
                1.0
            };
            let bounds = if corner.is_some() {
                Self::display_bounds(bounds, scale)
            } else {
                bounds
            };
            let mut limits = (0.0001_f64, 10000.0_f64);
            for object in self
                .drawing
                .objects()
                .filter(|o| self.selection.ids.contains(&o.object_id()))
            {
                limits.0 = limits.0.max(1e-4 / object.placement().scale);
                limits.1 = limits.1.min(1e4 / object.placement().scale);
            }
            limits.0 = limits.0.max(minimum_factor);
            let Some((_, view, _)) = self.selection_view else {
                return;
            };
            let session = DragSession {
                ids: self.selection.ids.clone(),
                sources: self
                    .drawing
                    .items()
                    .iter()
                    .filter(|o| self.selection.ids.contains(&o.id()))
                    .cloned()
                    .collect(),
                view,
                revision: self.drawing.revision(),
            };
            self.selection_gesture = Some(SelectionGesture::Transform {
                session,
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
    fn selection_sample(&mut self, surface: Surface, point: Point, scale: f64, shift: bool) {
        match &mut self.selection_gesture {
            Some(SelectionGesture::ShapeEdit {
                surface: s,
                source,
                preview,
                index,
                corner,
                start,
                moved,
            }) if *s == surface => {
                if !*moved && (point[0] - start[0]).hypot(point[1] - start[1]) * scale < 3.0 {
                    return;
                }
                *moved = true;
                // Keep the initial grab offset: the hit target is larger than
                // the visible handle, and must not snap its center to the pointer.
                let handle = if *corner {
                    source.box_corners().unwrap()[*index]
                } else {
                    source.endpoints().unwrap()[*index]
                };
                let target = std::array::from_fn(|i| handle[i] + (point[i] - start[i]));
                let shape = if *corner {
                    source.with_corner(*index, target, shift)
                } else {
                    source.with_endpoint(*index, target, shift)
                };
                if let Some(shape) = shape
                    && self
                        .drawing
                        .valid_object(&ObjectSnapshot::from(shape.clone()))
                {
                    *preview = shape;
                }
            }

            Some(SelectionGesture::Rectangle {
                surface: s,
                start,
                end,
                moved,
                matches,
                ..
            }) if *s == surface => {
                *end = point;
                *moved |= (point[0] - start[0]).hypot(point[1] - start[1]) * scale >= 3.0;
                if *moved {
                    matches.clear();
                    let bounds = ObjectBounds::from_points([*start, point]).unwrap();
                    self.object_index
                        .sync_objects(self.drawing.revision(), self.drawing.items());
                    self.object_index
                        .query_rect(surface, bounds.min, bounds.max, |index| {
                            let object = &self.drawing.items()[index];
                            let hit = match object {
                                ObjectSnapshot::Stroke(stroke) => {
                                    let cached = self
                                        .cache
                                        .entry(stroke.id)
                                        .or_insert_with(|| Cached::new(stroke));
                                    selection::rectangle_matches(
                                        bounds,
                                        point[0] < start[0],
                                        &cached.path,
                                        &cached.pressures,
                                        stroke.visible_width(),
                                        stroke.pressure_style,
                                    )
                                }
                                ObjectSnapshot::Shape(shape) => {
                                    shape.rectangle_matches(bounds, point[0] < start[0])
                                }
                            };
                            if hit {
                                matches.push(object.id());
                            }
                        });
                }
            }
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
            Some(SelectionGesture::Rotate {
                surface: s,
                start,
                center,
                geometry,
                last_angle,
                angle,
                delta,
                moved,
            }) if *s == surface => {
                if (point[0] - center[0]).hypot(point[1] - center[1]) * scale < 1.0 {
                    return;
                }
                let next = (point[1] - center[1]).atan2(point[0] - center[0]);
                *angle += (next - *last_angle + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI;
                *last_angle = next;
                if !*moved && (point[0] - start[0]).hypot(point[1] - start[1]) * scale < 3.0 {
                    return;
                }
                let step = std::f64::consts::PI / 12.0;
                let rotation = if shift {
                    (*angle / step).round() * step
                } else {
                    *angle
                };
                if let Some(valid) = selection::rotation_about(
                    *geometry,
                    *center,
                    rotation,
                    self.drawing.bounds(surface),
                ) {
                    *delta = valid;
                    *moved = true;
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
                ..
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
            Some(SelectionGesture::ShapeEdit {
                preview,
                moved: true,
                ..
            }) => {
                self.drawing.replace_shape(preview);
                self.prune_cache();
            }
            Some(SelectionGesture::Rectangle {
                mode: Membership::Replace,
                moved: false,
                ..
            }) => self.selection.clear(),
            Some(SelectionGesture::Rectangle {
                mode,
                moved: true,
                matches,
                ..
            }) => {
                self.selection.change(matches, mode, &self.drawing);
            }
            Some(SelectionGesture::Transform {
                session,
                delta,
                moved: true,
                ..
            }) => {
                // Never apply an old preview to changed content or a new selection.
                if self.drawing.revision() == session.revision
                    && self
                        .drawing
                        .items()
                        .iter()
                        .filter(|o| session.ids.contains(&o.id()))
                        .eq(session.sources.iter())
                {
                    self.drawing.transform_objects(&session.ids, delta);
                    self.prune_cache();
                }
            }
            Some(SelectionGesture::Rotate {
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
                    if mode == Membership::Replace {
                        self.selection.clear();
                    }
                    return;
                };
                let bounds = region.bounds;
                self.object_index
                    .sync_objects(self.drawing.revision(), self.drawing.items());
                let mut selected = Vec::new();
                self.object_index
                    .query_rect(surface, bounds.min, bounds.max, |index| {
                        let object = &self.drawing.items()[index];
                        let hit = match object {
                            ObjectSnapshot::Stroke(stroke) => {
                                let cached = self
                                    .cache
                                    .entry(stroke.id)
                                    .or_insert_with(|| Cached::new(stroke));
                                region.encloses(
                                    &cached.path,
                                    &cached.pressures,
                                    stroke.visible_width(),
                                    stroke.pressure_style,
                                )
                            }
                            ObjectSnapshot::Shape(shape) => shape.lasso_matches(&region),
                        };
                        if hit {
                            selected.push(object.id());
                        }
                    });
                self.selection.change(selected, mode, &self.drawing);
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
            if let Some(SelectionGesture::Transform { session, .. }) = &self.selection_gesture
                && (session.view != transform || session.revision != self.drawing.revision())
            {
                self.selection_gesture = None;
            }
            if self.selection_gesture.is_some()
                && self
                    .selection_view
                    .is_some_and(|(_, old, _)| old != transform)
            {
                self.selection_gesture = None;
            }
            self.selection_view = Some((surface, transform, bounds.intersect(ui.clip_rect())));
        }
        if self.processed.contains(&surface) {
            return;
        }
        self.processed.push(surface);
        let selection_tool = matches!(self.tool, Tool::Select | Tool::Lasso);
        let owns = |p: Pos2| {
            (bounds.contains(p)
                || (selection_tool
                    && bounds
                        .expand(style::ROTATION_OFFSET + style::SELECTION_HIT)
                        .contains(p)))
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
                } if self.selection_gesture.is_none()
                    && owns(pos)
                    && !ui.input(|i| i.key_down(egui::Key::Space)) =>
                {
                    let point = transform.to_document([pos.x as f64, pos.y as f64]);
                    let corner = if self.selection.surface == surface && !modifiers.alt {
                        self.handle(point, transform.scale)
                    } else {
                        None
                    };
                    let rotation = self.selection.surface == surface
                        && !modifiers.alt
                        && self.rotation_hit(point, transform.scale);
                    if !self.drawing.contains(surface, point) && corner.is_none() && !rotation {
                        continue;
                    }
                    self.selection.scope(surface);
                    self.selection_view =
                        Some((surface, transform, bounds.intersect(ui.clip_rect())));
                    if !modifiers.alt
                        && let Some((shape, index, corner)) =
                            self.shape_handle(point, transform.scale)
                    {
                        self.selection_gesture = Some(SelectionGesture::ShapeEdit {
                            surface,
                            source: shape.clone(),
                            preview: (*shape).clone(),
                            index,
                            corner,
                            start: point,
                            moved: false,
                        });
                        continue;
                    }
                    if rotation {
                        self.start_rotation(surface, point);
                        continue;
                    }
                    if let Some(corner) = corner {
                        self.start_transform(surface, point, Some(corner), transform.scale);
                        continue;
                    }
                    let mode = if modifiers.alt {
                        Membership::Subtract
                    } else if modifiers.shift {
                        Membership::Add
                    } else {
                        Membership::Replace
                    };
                    if let Some(id) = self.pick(surface, point, 4.0 / transform.scale) {
                        if mode != Membership::Replace {
                            self.selection.change([id], mode, &self.drawing);
                        } else {
                            if !self.selection.ids.contains(&id) {
                                self.selection.change([id], mode, &self.drawing);
                            }
                            self.start_transform(surface, point, None, transform.scale);
                        }
                    } else {
                        self.selection_gesture = Some(if self.tool == Tool::Lasso {
                            SelectionGesture::Lasso {
                                surface,
                                points: vec![point],
                                mode,
                                overflow: false,
                            }
                        } else {
                            SelectionGesture::Rectangle {
                                surface,
                                start: point,
                                end: point,
                                mode,
                                moved: false,
                                matches: Vec::new(),
                            }
                        });
                    }
                }
                Event::ModifiersChanged(modifiers) => {
                    if matches!(
                        self.selection_gesture,
                        Some(SelectionGesture::Rotate { .. })
                    ) && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
                    {
                        self.selection_sample(
                            surface,
                            transform.to_document([pos.x as f64, pos.y as f64]),
                            transform.scale,
                            modifiers.shift,
                        );
                    }
                }
                Event::PointerMoved(pos) => self.selection_sample(
                    surface,
                    transform.to_document([pos.x as f64, pos.y as f64]),
                    transform.scale,
                    ui.input(|i| i.modifiers.shift),
                ),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers,
                } => {
                    let active = match &self.selection_gesture {
                        Some(
                            SelectionGesture::Lasso { surface: s, .. }
                            | SelectionGesture::ShapeEdit { surface: s, .. }
                            | SelectionGesture::Rectangle { surface: s, .. }
                            | SelectionGesture::Transform { surface: s, .. }
                            | SelectionGesture::Rotate { surface: s, .. },
                        ) => *s == surface,
                        _ => false,
                    };
                    if active {
                        self.selection_sample(
                            surface,
                            transform.to_document([pos.x as f64, pos.y as f64]),
                            transform.scale,
                            modifiers.shift,
                        );
                        self.commit_selection();
                    }
                }
                // eframe sends this on canvas mouseleave, but continues mouse
                // motion/up on document. Hover loss is not contact cancellation.
                Event::PointerGone => {}
                Event::WindowFocused(false)
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
        if let Some(SelectionGesture::Rectangle {
            surface: s,
            start,
            end,
            moved: true,
            matches,
            ..
        }) = &self.selection_gesture
            && *s == surface
        {
            let crossing = end[0] < start[0];
            let color = if crossing { p.crossing } else { p.containment };
            let rect = Rect::from_two_pos(screen(*start), screen(*end));
            for id in matches {
                if let Some(cached) = self.cache.get(id) {
                    painter.rect_stroke(
                        Rect::from_min_max(screen(cached.bounds.0), screen(cached.bounds.1)),
                        1.0,
                        Line::new(1.5, color),
                        egui::StrokeKind::Outside,
                    );
                } else if let Some(object) = self.drawing.object(*id)
                    && let Some(b) = object.world_bounds()
                {
                    painter.rect_stroke(
                        Rect::from_min_max(screen(b.min), screen(b.max)),
                        1.0,
                        Line::new(1.5, color),
                        egui::StrokeKind::Outside,
                    );
                }
            }
            painter.rect_filled(rect, 0.0, color.gamma_multiply(0.10));
            if crossing {
                let corners = [
                    rect.left_top(),
                    rect.right_top(),
                    rect.right_bottom(),
                    rect.left_bottom(),
                    rect.left_top(),
                ];
                painter.extend(egui::Shape::dashed_line(
                    &corners,
                    Line::new(1.25, color),
                    5.0,
                    4.0,
                ));
            } else {
                painter.rect_stroke(rect, 0.0, Line::new(1.25, color), egui::StrokeKind::Inside);
            }
        }
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
        if let Some(group) = self
            .shape_edit_preview()
            .and_then(|s| s.world_bounds())
            .or(self.selection.bounds)
        {
            let group = Self::display_bounds(group, transform.scale);
            let corners = group.corners().map(|point| screen(delta.apply(point)));
            painter.add(egui::Shape::closed_line(
                corners.to_vec(),
                Line::new(1.0, p.accent),
            ));
            if matches!(self.tool, Tool::Select | Tool::Lasso) {
                for corner in corners.into_iter().filter(|_| !self.single_shape_handles()) {
                    painter.rect(
                        Rect::from_center_size(corner, egui::Vec2::splat(style::SELECTION_HANDLE)),
                        1.0,
                        p.surface,
                        Line::new(1.0, p.accent),
                        egui::StrokeKind::Inside,
                    );
                }
                if let Some(handle) = self.rotation_point(transform.scale) {
                    let stem = [group.center()[0], group.min[1]];
                    let handle = screen(delta.apply(handle));
                    let handle_painter = ui.painter().with_clip_rect(ui.clip_rect());
                    handle_painter
                        .line_segment([screen(delta.apply(stem)), handle], Line::new(1., p.accent));
                    handle_painter.circle(
                        handle,
                        style::SELECTION_HANDLE * 0.5,
                        p.surface,
                        Line::new(1., p.accent),
                    );
                }
            }
        }
        self.paint_shape_handles(&painter, transform, surface, p.accent);
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()).filter(|p| {
            bounds
                .expand(style::ROTATION_OFFSET + style::SELECTION_HIT)
                .contains(*p)
        }) {
            if ui.input(|i| i.key_down(egui::Key::Space)) {
                return;
            }
            let point = transform.to_document([pos.x as f64, pos.y as f64]);
            let cursor = if matches!(
                self.selection_gesture,
                Some(SelectionGesture::Rotate { .. })
            ) {
                egui::CursorIcon::Grabbing
            } else if self.shape_handle(point, transform.scale).is_some() {
                egui::CursorIcon::Grab
            } else if let Some(index) = self.handle(point, transform.scale) {
                if index % 2 == 0 {
                    egui::CursorIcon::ResizeNwSe
                } else {
                    egui::CursorIcon::ResizeNeSw
                }
            } else if self.rotation_hit(point, transform.scale) {
                egui::CursorIcon::Grab
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
    pub(super) fn selection_controls(
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
        if matches!(self.tool, Tool::Select | Tool::Lasso)
            && self.selection.surface == surface
            && self.selection_gesture.is_none()
            && let Some(group) = self.selection.bounds
        {
            let display = Self::display_bounds(group, transform.scale);
            let top = transform.to_screen([display.center()[0], display.min[1]]);
            let bottom = transform.to_screen([display.center()[0], display.max[1]]);
            let rect = Rect::from_min_max(
                egui::pos2(transform.to_screen(group.min)[0] as f32, top[1] as f32),
                egui::pos2(transform.to_screen(group.max)[0] as f32, bottom[1] as f32),
            );
            if rect.intersects(viewport) {
                let clearance = style::ROTATION_OFFSET + 44.0;
                let y = if top[1] as f32 - clearance >= viewport.top() {
                    top[1] as f32 - clearance
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
            let point = transform.to_document([pos.x as f64, pos.y as f64]);
            if self.drawing.contains(surface, point)
                && let Some(id) = self.pick(surface, point, 4.0 / transform.scale)
            {
                self.finish();
                self.selection.scope(surface);
                self.select(self.selection_mode);
                if !self.selection.ids.contains(&id) {
                    self.selection
                        .change([id], Membership::Replace, &self.drawing);
                }
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

#[cfg(test)]
#[path = "manipulation_tests.rs"]
mod manipulation_tests;

#[cfg(test)]
#[path = "flip_tests.rs"]
mod flip_tests;

#[cfg(test)]
#[path = "shape_tests.rs"]
mod shape_tests;

#[cfg(test)]
#[path = "shape_drag_tests.rs"]
mod shape_drag_tests;
