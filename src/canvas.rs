//! Shared pointer controller and rounded vector renderer for every surface.
use crate::object::hit_stroke_path as pressure_hit;
use crate::{
    document::DocumentData,
    drawing::{self, Drawing, EraserMode, Stroke, Surface, Transform},
};
use egui::{Color32, Event, PointerButton, Pos2, Rect, Stroke as Line};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;
#[path = "canvas_selection.rs"]
mod selection_ui;
use selection_ui::SelectionGesture;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Eraser,
    Pan,
    Select,
    Lasso,
}

struct Erasing {
    surface: Surface,
    previous: [f64; 2],
    ids: HashSet<Uuid>,
    mode: EraserMode,
    partials: HashMap<Uuid, PartialPreview>,
}
struct PartialPreview {
    cut: crate::partial_erase::PartialCut,
    meshes: HashMap<usize, (Transform, f32, Arc<egui::Mesh>)>,
}
enum Gesture {
    Pen(Stroke),
    Eraser(Erasing),
}
struct RotationPreview {
    key: (crate::object::Placement, Transform, f32),
    meshes: Vec<Arc<egui::Mesh>>,
}
struct Cached {
    placement: crate::object::Placement,
    path: Vec<[f64; 2]>,
    pressures: Vec<Option<f32>>,
    view: Option<(Transform, f32)>,
    meshes: Vec<Arc<egui::Mesh>>,
    rotation_preview: Option<RotationPreview>,
    bounds: ([f64; 2], [f64; 2]),
}
impl Cached {
    fn new(stroke: &Stroke) -> Self {
        let path = stroke.path();
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for p in &path {
            for axis in 0..2 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
        Self {
            placement: stroke.placement,
            pressures: crate::partial_erase::path_pressures(stroke, &path)
                .unwrap_or_else(|| vec![stroke.points[0].pressure; path.len()]),
            path,
            view: None,
            meshes: Vec::new(),
            rotation_preview: None,
            bounds: (min, max),
        }
    }
    fn prepare_meshes(&mut self, ui: &egui::Ui, transform: Transform, stroke: &Stroke) {
        let ppp = ui.ctx().pixels_per_point();
        if self.view != Some((transform, ppp)) {
            self.meshes.clear();
            self.rotation_preview = None;
            self.view = Some((transform, ppp));
        }
        let chunks = self
            .path
            .len()
            .saturating_sub(1)
            .div_ceil(MESH_SEGMENTS)
            .max(1);
        for index in self.meshes.len()..chunks {
            let start = index * MESH_SEGMENTS;
            let end = (start + MESH_SEGMENTS + 1).min(self.path.len());
            let shapes = if stroke.color[3] < 255 {
                // Neighboring samples make both sides of a cached chunk seam agree.
                let lo = start.saturating_sub(1);
                let hi = (end + 1).min(self.path.len());
                let points: Vec<_> = self.path[lo..hi]
                    .iter()
                    .map(|point| {
                        let p = transform.to_screen(*point);
                        Pos2::new(p[0] as f32, p[1] as f32)
                    })
                    .collect();
                translucent_shapes(
                    &points,
                    &self.pressures[lo..hi],
                    (stroke.visible_width() * transform.scale) as f32,
                    ink_color(stroke),
                    stroke.pressure_style,
                    start - lo..end - lo,
                )
            } else {
                rounded_shapes(
                    &self.path[start..end],
                    &self.pressures[start..end],
                    transform,
                    stroke,
                    index > 0,
                )
            };
            let primitives = ui.ctx().tessellate(
                shapes
                    .into_iter()
                    .map(|shape| egui::epaint::ClippedShape::new(Rect::EVERYTHING, shape))
                    .collect(),
                ppp,
            );
            let mut mesh = egui::Mesh::default();
            for primitive in primitives {
                if let egui::epaint::Primitive::Mesh(part) = primitive.primitive {
                    mesh.append(part);
                }
            }
            self.meshes.push(Arc::new(mesh));
        }
    }
    fn paint(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        transform: Transform,
        stroke: &Stroke,
    ) {
        self.prepare_meshes(ui, transform, stroke);
        self.rotation_preview = None;
        for mesh in &self.meshes {
            painter.add(egui::Shape::Mesh(mesh.clone()));
        }
    }
    /// egui layer transforms cannot rotate. Reuse tessellated base meshes and
    /// update only vertex positions once per changed preview frame, never per
    /// pointer event. Idle frames share the same preview Arcs. make_mut reuses
    /// capacity when the renderer no longer holds the preceding frame's Arc.
    fn paint_rotation(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        transform: Transform,
        stroke: &Stroke,
        delta: crate::object::Placement,
    ) {
        self.prepare_meshes(ui, transform, stroke);
        let key = (delta, transform, ui.ctx().pixels_per_point());
        let preview = self
            .rotation_preview
            .get_or_insert_with(|| RotationPreview {
                key,
                meshes: Vec::new(),
            });
        if preview.key != key || preview.meshes.len() != self.meshes.len() {
            let origin = transform.to_screen(delta.origin);
            let position = transform.to_screen(delta.position);
            let (sin, cos) = delta.rotation.sin_cos();
            for (i, base) in self.meshes.iter().enumerate() {
                if preview.meshes.len() <= i {
                    preview.meshes.push(Arc::new((**base).clone()));
                }
                let target = Arc::make_mut(&mut preview.meshes[i]);
                for (vertex, raw) in target.vertices.iter_mut().zip(&base.vertices) {
                    let x = (f64::from(raw.pos.x) - origin[0]) * delta.scale;
                    let y = (f64::from(raw.pos.y) - origin[1]) * delta.scale;
                    vertex.pos = egui::pos2(
                        (position[0] + x * cos - y * sin) as f32,
                        (position[1] + x * sin + y * cos) as f32,
                    );
                }
            }
            preview.key = key;
        }
        for mesh in &preview.meshes {
            painter.add(egui::Shape::Mesh(mesh.clone()));
        }
    }
    fn paint_partial(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        transform: Transform,
        stroke: &Stroke,
        partial: &mut PartialPreview,
    ) {
        use crate::partial_erase as cut;
        if partial.cut.cuts.covers(cut::end(&self.path)) {
            return;
        }
        self.prepare_meshes(ui, transform, stroke);
        let dpi = ui.ctx().pixels_per_point();
        for (index, original) in self.meshes.iter().enumerate() {
            let start = (index * MESH_SEGMENTS) as f64;
            let end = ((index + 1) * MESH_SEGMENTS).min(self.path.len().saturating_sub(1)) as f64;
            if !partial.cut.cuts.touches(start, end) {
                painter.add(egui::Shape::Mesh(original.clone()));
                continue;
            }
            // Cache empty chunks too: repeated movement/idle frames must not
            // regenerate geometry that has already been completely erased.
            if partial
                .meshes
                .get(&index)
                .is_none_or(|cached| cached.0 != transform || cached.1 != dpi)
            {
                let mut shapes = Vec::new();
                partial.cut.cuts.visit_visible(start, end, |lo, hi| {
                    let mut points = Vec::new();
                    let mut pressures = Vec::new();
                    cut::visit_piece(&self.path, lo, hi, |p, t| {
                        points.push(p);
                        pressures.push(pressure_at(&self.pressures, t));
                    });
                    let skip_cap =
                        index > 0 && lo == start && partial.cut.cuts.connected_before(start);
                    shapes.extend(rounded_shapes(
                        &points, &pressures, transform, stroke, skip_cap,
                    ));
                });
                let mut mesh = egui::Mesh::default();
                for primitive in ui.ctx().tessellate(
                    shapes
                        .into_iter()
                        .map(|s| egui::epaint::ClippedShape::new(Rect::EVERYTHING, s))
                        .collect(),
                    dpi,
                ) {
                    if let egui::epaint::Primitive::Mesh(part) = primitive.primitive {
                        mesh.append(part);
                    }
                }
                partial
                    .meshes
                    .insert(index, (transform, dpi, Arc::new(mesh)));
            }
            let mesh = &partial.meshes[&index].2;
            if !mesh.vertices.is_empty() {
                painter.add(egui::Shape::Mesh(mesh.clone()));
            }
        }
    }
}

const MESH_SEGMENTS: usize = 128;

pub struct Canvas {
    pub drawing: Drawing,
    pub tool: Tool,
    pub pen_type: crate::pen::PenType,
    pub color: [u8; 4],
    pub width: f64,
    /// Eraser radius in logical screen points, shown at its actual hit-test size.
    pub eraser_radius: f64,
    pub smoothing_strength: u8,
    pub pressure_style: drawing::PressureStyle,
    pub selection: crate::selection::Selection,
    pub notice: Option<String>,
    selection_gesture: Option<SelectionGesture>,
    selection_view: Option<(Surface, Transform, Rect)>,
    pending_paste: Option<f64>,
    input_pressure: Option<f32>,
    touch_id: Option<egui::TouchId>,
    pub eraser_mode: EraserMode,
    preview: crate::smoothing::Preview,
    gesture: Option<Gesture>,
    gesture_transform: Option<Transform>,
    cache: HashMap<Uuid, Cached>,
    eraser_index: crate::eraser_index::EraserIndex,
    frame: Option<u64>,
    processed: Vec<Surface>,
}
impl Canvas {
    pub fn apply_pen(&mut self, style: &crate::pen::PenStyle) {
        self.pen_type = style.pen_type;
        self.color = style.rgba();
        self.width = style.width;
        self.smoothing_strength = style.smoothing_strength;
        self.pressure_style = style.effective_pressure();
    }

    pub fn new(data: &DocumentData) -> Self {
        Self {
            drawing: Drawing::new(data),
            tool: Tool::Pen,
            pen_type: crate::pen::PenType::Pressure,
            color: [37, 99, 200, 255],
            width: 2.5,
            eraser_radius: 14.0,
            smoothing_strength: 50,
            pressure_style: drawing::PressureStyle {
                enabled: true,
                ..Default::default()
            },
            selection: crate::selection::Selection::new(
                if data.kind() == crate::document::DocumentKind::Page {
                    Surface::Page(0)
                } else {
                    Surface::Canvas
                },
            ),
            notice: None,
            selection_gesture: None,
            selection_view: None,
            pending_paste: None,
            input_pressure: None,
            touch_id: None,
            eraser_mode: EraserMode::Stroke,
            preview: Default::default(),
            gesture: None,
            gesture_transform: None,
            cache: HashMap::new(),
            eraser_index: Default::default(),
            frame: None,
            processed: Vec::new(),
        }
    }
    pub fn interacting(&self) -> bool {
        self.gesture.is_some() || self.selection_gesture.is_some()
    }
    pub fn dirty(&self) -> bool {
        self.drawing.dirty()
            || matches!(&self.gesture, Some(Gesture::Pen(stroke)) if !stroke.points.is_empty())
    }
    pub fn finish(&mut self) {
        self.selection_gesture = None;
        self.pending_paste = None;
        match self.gesture.take() {
            Some(Gesture::Pen(stroke)) => {
                self.update_preview(&stroke);
                self.drawing.add(stroke);
                self.preview = Default::default();
            }
            Some(Gesture::Eraser(erase)) => {
                if erase.ids.is_empty() {
                    self.gesture_transform = None;
                    return;
                }
                if erase.mode == EraserMode::Stroke {
                    self.drawing.erase(&erase.ids);
                } else {
                    let replacements = self
                        .drawing
                        .strokes()
                        .iter()
                        .filter(|s| erase.ids.contains(&s.id))
                        .map(|stroke| {
                            let partial = &erase.partials[&stroke.id];
                            let path = &self.cache[&stroke.id].path;
                            (
                                stroke.id,
                                crate::partial_erase::fragments(stroke, path, &partial.cut.cuts),
                            )
                        })
                        .collect();
                    self.drawing.replace_partial(replacements);
                }
            }
            None => return,
        }
        self.gesture_transform = None;
        self.prune_cache();
    }
    pub fn undo(&mut self) {
        self.finish();
        let ids = self.drawing.undo();
        self.prune_cache();
        if matches!(self.tool, Tool::Select | Tool::Lasso) && !ids.is_empty() {
            self.selection
                .change(ids, crate::selection::Membership::Replace, &self.drawing);
        }
    }
    pub fn redo(&mut self) {
        self.finish();
        let ids = self.drawing.redo();
        self.prune_cache();
        if matches!(self.tool, Tool::Select | Tool::Lasso) && !ids.is_empty() {
            self.selection
                .change(ids, crate::selection::Membership::Replace, &self.drawing);
        }
    }
    pub fn select(&mut self, tool: Tool) {
        self.finish();
        self.tool = tool;
        if !matches!(tool, Tool::Select | Tool::Lasso) {
            self.selection.clear();
        }
    }

    fn prune_cache(&mut self) {
        let live: HashMap<_, _> = self
            .drawing
            .strokes()
            .iter()
            .map(|s| (s.id, s.placement))
            .collect();
        self.cache
            .retain(|id, cached| live.get(id).is_some_and(|p| *p == cached.placement));
        self.selection.refresh(&self.drawing);
    }

    pub fn begin_frame(&mut self, ui: &egui::Ui) {
        if self.frame == Some(ui.ctx().cumulative_frame_nr()) {
            return;
        }
        self.frame = Some(ui.ctx().cumulative_frame_nr());
        self.processed.clear();
        if !ui.input(|i| i.focused) || !ui.is_enabled() {
            self.finish();
        }
        if !ui.is_enabled() {
            return;
        }
        self.selection.refresh(&self.drawing);
        if !ui.ctx().text_edit_focused() {
            self.selection_keys(ui);
            let (undo, redo) = ui.input_mut(|i| {
                let redo = i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                ));
                let undo = i.consume_shortcut(&egui::KeyboardShortcut::new(
                    egui::Modifiers::COMMAND,
                    egui::Key::Z,
                ));
                (undo, redo)
            });
            if undo {
                self.undo();
            }
            if redo {
                self.redo();
            }
        }
    }
    pub fn end_frame(&mut self, ui: &egui::Ui) {
        // A release can occur outside the canvas/window or on a hidden page.
        if !ui.input(|i| i.pointer.button_down(PointerButton::Primary)) && self.gesture.is_some() {
            self.finish();
        }
    }

    pub fn surface(&mut self, ui: &egui::Ui, bounds: Rect, surface: Surface, transform: Transform) {
        if matches!(self.tool, Tool::Select | Tool::Lasso) {
            let preview_layer = egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::unique("object-preview").with(self.drawing.owner()),
            );
            if surface == self.selection.surface {
                ui.ctx()
                    .set_transform_layer(preview_layer, egui::emath::TSTransform::IDENTITY);
            }
        }
        if !matches!(self.tool, Tool::Select | Tool::Lasso) && self.selection.surface == surface {
            self.selection_view = Some((surface, transform, bounds.intersect(ui.clip_rect())));
        }
        if matches!(self.tool, Tool::Select | Tool::Lasso) {
            self.selection_surface(ui, bounds, surface, transform);
            self.paint(ui, bounds, surface, transform);
            self.paint_selection(ui, bounds, surface, transform);
            return;
        }
        if !self.processed.contains(&surface) {
            self.processed.push(surface);
            if self.surface_active(surface)
                && self.gesture_transform.is_some_and(|old| old != transform)
            {
                self.finish();
            }
            let events = ui.input(|i| i.events.clone());
            for event in events {
                match event {
                    Event::Touch {
                        id, phase, force, ..
                    } => {
                        if phase == egui::TouchPhase::Start && self.touch_id.is_none() {
                            self.touch_id = Some(id);
                        }
                        if self.touch_id == Some(id) {
                            // End events often report zero after contact; retain the last contact force.
                            if matches!(phase, egui::TouchPhase::Start | egui::TouchPhase::Move) {
                                self.input_pressure = drawing::normalize_pressure(force);
                            } else if phase == egui::TouchPhase::Cancel {
                                self.finish();
                                self.touch_id = None;
                                self.input_pressure = None;
                            }
                        }
                    }
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        ..
                    } if self.gesture.is_none()
                        && self.tool != Tool::Pan
                        && !ui.input(|i| i.key_down(egui::Key::Space))
                        && bounds.contains(pos)
                        && ui.clip_rect().contains(pos)
                        && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
                        && ui.is_enabled() =>
                    {
                        let point = transform.to_document([pos.x as f64, pos.y as f64]);
                        if !self.drawing.contains(surface, point) {
                            continue;
                        }
                        self.selection.scope(surface);
                        self.selection_view =
                            Some((surface, transform, bounds.intersect(ui.clip_rect())));
                        self.gesture_transform = Some(transform);
                        if self.tool == Tool::Pen {
                            let mut stroke =
                                Stroke::new(self.drawing.owner(), surface, self.color, self.width);
                            stroke.smoothing = drawing::Smoothing::AdaptiveV1 {
                                strength: self.smoothing_strength,
                            };
                            stroke.pen_type = self.pen_type;
                            stroke.pressure_style = self.pressure_style;
                            self.preview = Default::default();
                            stroke.push_pressure(point, self.input_pressure);
                            self.gesture = Some(Gesture::Pen(stroke));
                        } else {
                            self.gesture = Some(Gesture::Eraser(Erasing {
                                surface,
                                previous: point,
                                ids: HashSet::new(),
                                mode: self.eraser_mode,
                                partials: HashMap::new(),
                            }));
                            self.sample(surface, point, transform.scale);
                        }
                    }
                    Event::PointerMoved(pos) => self.sample(
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
                        self.sample(
                            surface,
                            transform.to_document([pos.x as f64, pos.y as f64]),
                            transform.scale,
                        );
                        // Only the owning surface can finish this gesture.
                        if self.surface_active(surface) {
                            self.finish();
                            self.input_pressure = None;
                            self.touch_id = None;
                        }
                    }
                    Event::PointerGone => {
                        self.finish();
                        self.input_pressure = None;
                        self.touch_id = None;
                    }
                    _ => {}
                }
            }
        }
        self.paint(ui, bounds, surface, transform);
        if bounds.contains(ui.input(|i| i.pointer.hover_pos()).unwrap_or(Pos2::ZERO))
            && ui
                .ctx()
                .layer_id_at(ui.input(|i| i.pointer.hover_pos()).unwrap_or(Pos2::ZERO))
                == Some(ui.layer_id())
        {
            if self.tool == Tool::Pan || ui.input(|i| i.key_down(egui::Key::Space)) {
                ui.ctx().set_cursor_icon(
                    if ui.input(|i| {
                        i.pointer.primary_down() || i.pointer.button_down(PointerButton::Middle)
                    }) {
                        egui::CursorIcon::Grabbing
                    } else {
                        egui::CursorIcon::Grab
                    },
                );
            } else if self.tool == Tool::Eraser {
                ui.ctx().set_cursor_icon(egui::CursorIcon::None);
                if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let painter = ui
                        .painter()
                        .with_clip_rect(bounds.intersect(ui.clip_rect()));
                    painter.circle_stroke(
                        pos,
                        self.eraser_radius as f32,
                        Line::new(3.0, Color32::WHITE),
                    );
                    painter.circle_stroke(
                        pos,
                        self.eraser_radius as f32,
                        Line::new(1.0, Color32::from_gray(64)),
                    );
                }
            } else if self.tool == Tool::Pen {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }
    }
    fn surface_active(&self, surface: Surface) -> bool {
        match &self.gesture {
            Some(Gesture::Pen(stroke)) => stroke.surface == surface,
            Some(Gesture::Eraser(erase)) => erase.surface == surface,
            None => false,
        }
    }
    fn sample(&mut self, surface: Surface, point: [f64; 2], scale: f64) {
        if !self.surface_active(surface) {
            return;
        }
        match &mut self.gesture {
            Some(Gesture::Pen(stroke)) => {
                if self.drawing.contains(surface, point) {
                    stroke.push_pressure(point, self.input_pressure);
                } else {
                    // End at the A4 edge, never bridge an excursion into margins.
                    if let Some(bounds) = self.drawing.bounds(surface) {
                        let last = stroke.points.last().unwrap().position;
                        let mut t: f64 = 1.0;
                        for i in 0..2 {
                            if point[i] < 0.0 {
                                t = t.min(-last[i] / (point[i] - last[i]));
                            }
                            if point[i] > bounds[i] {
                                t = t.min((bounds[i] - last[i]) / (point[i] - last[i]));
                            }
                        }
                        let pressure = drawing::mix_pressure(
                            stroke.points.last().unwrap().pressure,
                            self.input_pressure,
                            t as f32,
                        );
                        stroke.push_pressure(
                            std::array::from_fn(|i| {
                                (last[i] + t * (point[i] - last[i])).clamp(0.0, bounds[i])
                            }),
                            pressure,
                        );
                    }
                    self.finish();
                }
            }
            Some(Gesture::Eraser(erase)) => {
                self.eraser_index
                    .sync(self.drawing.revision(), self.drawing.strokes());
                let radius = self.eraser_radius / scale;
                self.eraser_index
                    .query(surface, erase.previous, point, radius, |index| {
                        let stroke = &self.drawing.strokes()[index];
                        if erase.mode == EraserMode::Stroke && erase.ids.contains(&stroke.id) {
                            return;
                        }
                        let cached = self
                            .cache
                            .entry(stroke.id)
                            .or_insert_with(|| Cached::new(stroke));
                        if erase.mode == EraserMode::Stroke {
                            if pressure_hit(
                                &cached.path,
                                &cached.pressures,
                                stroke,
                                erase.previous,
                                point,
                                radius,
                            ) {
                                erase.ids.insert(stroke.id);
                            }
                        } else {
                            let partial =
                                erase
                                    .partials
                                    .entry(stroke.id)
                                    .or_insert_with(|| PartialPreview {
                                        cut: crate::partial_erase::PartialCut::with_pressure(
                                            &cached.path,
                                            stroke,
                                            &cached.pressures,
                                        ),
                                        meshes: HashMap::new(),
                                    });
                            let meshes = &mut partial.meshes;
                            if partial.cut.sweep(
                                &cached.path,
                                surface,
                                erase.previous,
                                point,
                                radius,
                                |lo, hi| {
                                    let first = (lo as usize / MESH_SEGMENTS).saturating_sub(1);
                                    let last = hi as usize / MESH_SEGMENTS + 1;
                                    for chunk in first..=last {
                                        meshes.remove(&chunk);
                                    }
                                },
                            ) {
                                erase.ids.insert(stroke.id);
                            }
                        }
                    });
                erase.previous = point;
            }
            None => {}
        }
    }
    fn update_preview(&mut self, stroke: &Stroke) {
        let cached = self.cache.entry(stroke.id).or_insert_with(|| Cached {
            placement: stroke.placement,
            path: Vec::new(),
            pressures: Vec::new(),
            view: None,
            meshes: Vec::new(),
            rotation_preview: None,
            bounds: ([0.0; 2], [0.0; 2]),
        });
        let drawing::Smoothing::AdaptiveV1 { strength } = stroke.smoothing else {
            unreachable!()
        };
        if let Some(changed) = self
            .preview
            .update(&stroke.points, strength, &mut cached.path)
        {
            cached.pressures.truncate(changed);
            cached
                .pressures
                .extend_from_slice(&self.preview.pressures[changed..]);
            cached
                .meshes
                .truncate(changed.saturating_sub(1) / MESH_SEGMENTS);
        }
        // Completion does not change geometry. Bounds are needed only once the
        // stroke joins the committed drawing's normal culling pipeline.
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for p in &cached.path {
            for axis in 0..2 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
        cached.bounds = (min, max);
    }
    fn paint(&mut self, ui: &egui::Ui, bounds: Rect, surface: Surface, transform: Transform) {
        let painter = ui
            .painter()
            .with_clip_rect(bounds.intersect(ui.clip_rect()));
        let selection_preview = self.selection_preview(surface);
        let mut erasing = match &mut self.gesture {
            Some(Gesture::Eraser(e)) => Some(e),
            _ => None,
        };
        for stroke in self
            .drawing
            .strokes()
            .iter()
            .filter(|s| s.surface == surface)
        {
            if erasing
                .as_ref()
                .is_some_and(|e| e.mode == EraserMode::Stroke && e.ids.contains(&stroke.id))
            {
                continue;
            }
            let cached = self
                .cache
                .entry(stroke.id)
                .or_insert_with(|| Cached::new(stroke));
            let delta = selection_preview
                .filter(|_| self.selection.ids.contains(&stroke.id))
                .unwrap_or_default();
            let base_bounds = crate::object::ObjectBounds {
                min: cached.bounds.0,
                max: cached.bounds.1,
            };
            // Ordinary drawing pays no four-corner transform cost for previews.
            let world_bounds = if delta.is_identity() {
                base_bounds
            } else {
                base_bounds.placed(delta)
            };
            let min = transform.to_screen(world_bounds.min);
            let max = transform.to_screen(world_bounds.max);
            let screen_bounds = Rect::from_min_max(
                Pos2::new(min[0] as f32, min[1] as f32),
                Pos2::new(max[0] as f32, max[1] as f32),
            )
            .expand((stroke.visible_width() * delta.scale * transform.scale) as f32);
            if !screen_bounds.intersects(painter.clip_rect()) {
                continue;
            }
            if let Some(partial) = erasing
                .as_mut()
                .and_then(|e| e.partials.get_mut(&stroke.id))
            {
                cached.paint_partial(ui, &painter, transform, stroke, partial);
            } else {
                if let Some(delta) =
                    selection_preview.filter(|_| self.selection.ids.contains(&stroke.id))
                {
                    if delta.rotation != 0.0 {
                        cached.paint_rotation(ui, &painter, transform, stroke, delta);
                        continue;
                    }
                    let origin = transform.to_screen(delta.origin);
                    let position = transform.to_screen(delta.position);
                    let layer_transform = egui::emath::TSTransform {
                        scaling: delta.scale as f32,
                        translation: egui::vec2(
                            (position[0] - origin[0] * delta.scale) as f32,
                            (position[1] - origin[1] * delta.scale) as f32,
                        ),
                    };
                    ui.ctx().set_transform_layer(
                        preview_layer_id(self.drawing.owner()),
                        layer_transform,
                    );
                    let selected_painter = painter
                        .clone()
                        .with_layer_id(preview_layer_id(self.drawing.owner()))
                        .with_clip_rect(layer_transform.inverse() * painter.clip_rect());
                    cached.paint(ui, &selected_painter, transform, stroke);
                } else {
                    cached.paint(ui, &painter, transform, stroke);
                }
            }
        }
        if let Some(Gesture::Pen(stroke)) = &self.gesture
            && stroke.surface == surface
        {
            let cached = self.cache.entry(stroke.id).or_insert_with(|| Cached {
                placement: stroke.placement,
                path: Vec::new(),
                pressures: Vec::new(),
                view: None,
                meshes: Vec::new(),
                rotation_preview: None,
                bounds: ([0.0; 2], [0.0; 2]),
            });
            let drawing::Smoothing::AdaptiveV1 { strength } = stroke.smoothing else {
                unreachable!()
            };
            if let Some(changed) = self
                .preview
                .update(&stroke.points, strength, &mut cached.path)
            {
                cached.pressures.truncate(changed);
                cached
                    .pressures
                    .extend_from_slice(&self.preview.pressures[changed..]);
                cached
                    .meshes
                    .truncate(changed.saturating_sub(1) / MESH_SEGMENTS);
            }
            cached.paint(ui, &painter, transform, stroke);
        }
    }
}

fn preview_layer_id(owner: Uuid) -> egui::LayerId {
    egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::unique("object-preview").with(owner),
    )
}

fn pressure_at(pressures: &[Option<f32>], t: f64) -> Option<f32> {
    let i = (t.floor() as usize).min(pressures.len() - 1);
    drawing::mix_pressure(
        pressures[i],
        pressures[(i + 1).min(pressures.len() - 1)],
        (t - i as f64) as f32,
    )
}

fn ink_color(stroke: &Stroke) -> Color32 {
    Color32::from_rgba_unmultiplied(
        stroke.color[0],
        stroke.color[1],
        stroke.color[2],
        stroke.color[3],
    )
}

fn rounded_shapes(
    path: &[[f64; 2]],
    pressures: &[Option<f32>],
    transform: Transform,
    stroke: &Stroke,
    skip_first_cap: bool,
) -> Vec<egui::Shape> {
    let color = ink_color(stroke);
    let width = (stroke.visible_width() * transform.scale) as f32;
    let points: Vec<_> = path
        .iter()
        .map(|point| {
            let p = transform.to_screen(*point);
            Pos2::new(p[0] as f32, p[1] as f32)
        })
        .collect();
    if stroke.color[3] < 255 {
        return translucent_shapes(
            &points,
            pressures,
            width,
            color,
            stroke.pressure_style,
            0..points.len(),
        );
    }
    let mut shapes = Vec::with_capacity(points.len() * 2);
    if !stroke.pressure_style.enabled || pressures.iter().all(Option::is_none) {
        for pair in points.windows(2) {
            shapes.push(egui::Shape::line_segment(
                [pair[0], pair[1]],
                Line::new(width, color),
            ));
        }
        for point in points.into_iter().skip(usize::from(skip_first_cap)) {
            shapes.push(egui::Shape::circle_filled(point, width * 0.5, color));
        }
        return shapes;
    }
    let radii: Vec<_> = pressures
        .iter()
        .map(|p| width * stroke.pressure_style.factor(*p) as f32 * 0.5)
        .collect();
    for (i, pair) in points.windows(2).enumerate() {
        let delta = pair[1] - pair[0];
        if delta.length_sq() == 0.0 {
            continue;
        }
        let length = delta.length();
        let slope = (radii[i] - radii[i + 1]) / length;
        if slope.abs() >= 1.0 {
            continue;
        } // one disk contains the other
        let unit = delta / length;
        let normal = egui::vec2(-unit.y, unit.x) * (1.0 - slope * slope).sqrt();
        let left = unit * slope + normal;
        let right = unit * slope - normal;
        shapes.push(egui::Shape::convex_polygon(
            vec![
                pair[0] + left * radii[i],
                pair[1] + left * radii[i + 1],
                pair[1] + right * radii[i + 1],
                pair[0] + right * radii[i],
            ],
            color,
            Line::NONE,
        ));
    }
    for (i, point) in points
        .into_iter()
        .enumerate()
        .skip(usize::from(skip_first_cap))
    {
        shapes.push(egui::Shape::circle_filled(point, radii[i], color));
    }
    shapes
}

/// A joined ribbon avoids compositing every segment and cap on top of itself.
/// Separate strokes (and true self-crossings) still accumulate opacity normally.
/// Translucent ink has a flat nib; the opaque rounded renderer stays unchanged.
fn translucent_shapes(
    points: &[Pos2],
    pressures: &[Option<f32>],
    width: f32,
    color: Color32,
    pressure: drawing::PressureStyle,
    visible: std::ops::Range<usize>,
) -> Vec<egui::Shape> {
    if points.is_empty() {
        return Vec::new();
    }
    if points.len() == 1 || points.iter().all(|p| *p == points[0]) {
        let radius = pressures
            .iter()
            .map(|p| width * pressure.factor(*p) as f32 * 0.5)
            .fold(0.0_f32, f32::max);
        return vec![egui::Shape::circle_filled(points[0], radius, color)];
    }
    let directions: Vec<_> = points
        .windows(2)
        .map(|p| (p[1] - p[0]).normalized())
        .collect();
    let mut mesh = egui::Mesh::default();
    let mut previous = directions
        .iter()
        .copied()
        .find(|v| v.length_sq() > 0.0)
        .unwrap_or(egui::Vec2::X);
    for (i, point) in points.iter().enumerate() {
        let next = directions
            .get(i)
            .copied()
            .filter(|v| v.length_sq() > 0.0)
            .unwrap_or(previous);
        let normal = |v: egui::Vec2| egui::vec2(-v.y, v.x);
        let average = (normal(previous) + normal(next)).normalized();
        let miter = if average.length_sq() > 0.0 {
            average / average.dot(normal(next)).max(0.5)
        } else {
            normal(next)
        };
        let radius = width * pressure.factor(pressures[i]) as f32 * 0.5;
        // One feather on each outside edge; adjacent path samples share vertices.
        for (distance, ink) in [
            (radius + 0.5, Color32::TRANSPARENT),
            ((radius - 0.5).max(0.0), color),
            (-(radius - 0.5).max(0.0), color),
            (-radius - 0.5, Color32::TRANSPARENT),
        ] {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: *point + miter * distance,
                uv: egui::epaint::WHITE_UV,
                color: ink,
            });
        }
        if i > visible.start && i < visible.end {
            let start = (i as u32 - 1) * 4;
            for edge in 0..3 {
                let a = start + edge;
                mesh.indices
                    .extend_from_slice(&[a, a + 4, a + 1, a + 1, a + 4, a + 5]);
            }
        }
        previous = next;
    }
    vec![egui::Shape::mesh(mesh)]
}

/// Ordered pen input shared by the browser bridge and synthetic controller tests.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn pen_events(
    phase: u8,
    pos: Pos2,
    pressure: f32,
    modifiers: egui::Modifiers,
) -> Vec<Event> {
    let touch_phase = match phase {
        0 => egui::TouchPhase::Start,
        1 => egui::TouchPhase::Move,
        2 => egui::TouchPhase::End,
        _ => egui::TouchPhase::Cancel,
    };
    let mut events = vec![Event::Touch {
        device_id: egui::TouchDeviceId(0),
        id: egui::TouchId(0),
        phase: touch_phase,
        pos,
        force: drawing::normalize_pressure(Some(pressure)),
    }];
    match phase {
        0 | 2 => events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: phase == 0,
            modifiers,
        }),
        1 => events.push(Event::PointerMoved(pos)),
        _ => events.push(Event::PointerGone),
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentKind;

    #[test]
    fn pen_presets_capture_style_and_survive_partial_erase_history() {
        for (kind, surface) in [
            (DocumentKind::Page, Surface::Page(0)),
            (DocumentKind::Whiteboard, Surface::Canvas),
        ] {
            for pen_type in crate::pen::PenType::ALL {
                let data = DocumentData::blank(kind);
                let ctx = egui::Context::default();
                let mut canvas = Canvas::new(&data);
                let style = crate::pen::PenStyle {
                    pen_type,
                    width: 18.0,
                    opacity: 35,
                    smoothing_strength: 0,
                    ..Default::default()
                };
                canvas.apply_pen(&style);
                let mut events = pen_events(0, egui::pos2(30.0, 60.0), 0.1, egui::Modifiers::NONE);
                events.extend(pen_events(
                    1,
                    egui::pos2(100.0, 60.0),
                    0.5,
                    egui::Modifiers::NONE,
                ));
                events.extend(pen_events(
                    2,
                    egui::pos2(170.0, 60.0),
                    1.0,
                    egui::Modifiers::NONE,
                ));
                frame(&ctx, &mut canvas, surface, events);
                let source = canvas.drawing.strokes()[0].clone();
                assert_eq!(source.pen_type, pen_type);
                assert_eq!(source.color, style.rgba());
                assert_eq!(source.width, 18.0);
                assert_eq!(
                    source.pressure_style.enabled,
                    pen_type == crate::pen::PenType::Pressure
                );
                assert_eq!(
                    serde_json::from_str::<Stroke>(&serde_json::to_string(&*source).unwrap())
                        .unwrap(),
                    *source
                );
                canvas.apply_pen(&crate::pen::PenStyle::default());
                assert_eq!(canvas.drawing.strokes()[0], source);
                canvas.select(Tool::Eraser);
                canvas.eraser_mode = EraserMode::Partial;
                canvas.eraser_radius = 4.0;
                let at = egui::pos2(100.0, 60.0);
                frame(
                    &ctx,
                    &mut canvas,
                    surface,
                    vec![button(at, true), button(at, false)],
                );
                assert_eq!(canvas.drawing.strokes().len(), 2);
                let fragments = canvas.drawing.strokes().to_vec();
                for fragment in &fragments {
                    assert_eq!(fragment.pen_type, source.pen_type);
                    assert_eq!(fragment.color, source.color);
                    assert_eq!(fragment.pressure_style, source.pressure_style);
                }
                canvas.undo();
                assert_eq!(canvas.drawing.strokes(), &[source]);
                canvas.redo();
                assert_eq!(canvas.drawing.strokes(), fragments);
            }
        }
    }

    #[test]
    fn translucent_ribbons_do_not_overpaint_joins_and_reuse_cached_chunks() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let ctx = egui::Context::default();
        let mut canvas = Canvas::new(&data);
        let mut stroke = Stroke::new(data.id(), Surface::Canvas, [255, 200, 0, 89], 18.0);
        stroke.pen_type = crate::pen::PenType::Highlighter;
        stroke.smoothing = drawing::Smoothing::PolylineV1;
        for i in 0..400 {
            stroke.push([20.0 + i as f64, 100.0]);
        }
        let id = stroke.id;
        assert!(canvas.drawing.add(stroke));
        frame(&ctx, &mut canvas, Surface::Canvas, vec![]);
        let meshes = canvas.cache[&id].meshes.clone();
        // Independent strict triangle inclusion: one opaque-interior alpha sample
        // per position, even near sample joins and both sides of chunk boundaries.
        for x in [40.31, 147.91, 148.11, 200.37, 275.91, 276.11] {
            let point = egui::pos2(x, 100.17);
            let mut hits = 0;
            for mesh in &meshes {
                for tri in mesh.indices.as_chunks::<3>().0 {
                    let a = mesh.vertices[tri[0] as usize];
                    let b = mesh.vertices[tri[1] as usize];
                    let c = mesh.vertices[tri[2] as usize];
                    let cross = |u: egui::Vec2, v: egui::Vec2| u.x * v.y - u.y * v.x;
                    let sides = [
                        cross(b.pos - a.pos, point - a.pos),
                        cross(c.pos - b.pos, point - b.pos),
                        cross(a.pos - c.pos, point - c.pos),
                    ];
                    if sides.iter().all(|v| *v > 0.0) || sides.iter().all(|v| *v < 0.0) {
                        hits += 1;
                        assert_eq!([a.color.a(), b.color.a(), c.color.a()], [89; 3]);
                    }
                }
            }
            assert_eq!(hits, 1, "sample {x} must not darken at joins");
        }
        frame(&ctx, &mut canvas, Surface::Canvas, vec![]);
        assert!(
            meshes
                .iter()
                .zip(&canvas.cache[&id].meshes)
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
    }

    #[test]
    fn force_changes_at_rest_secondary_contacts_and_cancellation() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let mut canvas = Canvas::new(&data);
        let ctx = egui::Context::default();
        let pos = Pos2::new(40.0, 40.0);
        let mut events = pen_events(0, pos, 0.1, egui::Modifiers::NONE);
        events.extend(pen_events(1, pos, 0.8, egui::Modifiers::NONE));
        events.push(Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(9),
            phase: egui::TouchPhase::Start,
            pos,
            force: Some(0.0),
        });
        events.push(Event::PointerMoved(Pos2::new(80.0, 40.0)));
        events.extend(pen_events(3, pos, 0.0, egui::Modifiers::NONE));
        frame(&ctx, &mut canvas, Surface::Canvas, events);
        let stroke = &canvas.drawing.strokes()[0];
        assert_eq!(
            stroke.points.iter().map(|p| p.pressure).collect::<Vec<_>>(),
            vec![Some(0.1), Some(0.8), Some(0.8)]
        );
        assert!(!canvas.interacting());
        frame(
            &ctx,
            &mut canvas,
            Surface::Canvas,
            vec![button(pos, true), button(pos, false)],
        );
        assert!(
            canvas.drawing.strokes()[1]
                .points
                .iter()
                .all(|p| p.pressure.is_none())
        );
    }

    #[test]
    fn pressure_input_geometry_erasers_history_and_zoom() {
        for (kind, surface) in [
            (DocumentKind::Page, Surface::Page(0)),
            (DocumentKind::Whiteboard, Surface::Canvas),
        ] {
            for scale in [0.25, 1.0, 4.0] {
                let ctx = egui::Context::default();
                let data = DocumentData::blank(kind);
                let mut canvas = Canvas::new(&data);
                canvas.width = 12.0;
                canvas.smoothing_strength = 75;
                let transform = Transform {
                    origin: [20.0; 2],
                    center: [0.0; 2],
                    scale,
                };
                let pos = |x| {
                    let p = transform.to_screen([x, 40.0]);
                    Pos2::new(p[0] as f32, p[1] as f32)
                };
                let mut events = pen_events(0, pos(10.0), 0.0, egui::Modifiers::NONE);
                for (x, pressure) in [(30.0, 0.25), (50.0, 0.5), (70.0, 0.75), (90.0, 1.0)] {
                    events.extend(pen_events(1, pos(x), pressure, egui::Modifiers::NONE));
                }
                frame_transform(&ctx, &mut canvas, surface, events, transform);
                let preview = match canvas.gesture.as_ref().unwrap() {
                    Gesture::Pen(s) => s.clone(),
                    _ => panic!(),
                };
                let cached_preview = canvas.cache[&preview.id].pressures.clone();
                frame_transform(
                    &ctx,
                    &mut canvas,
                    surface,
                    pen_events(2, pos(90.0), 0.0, egui::Modifiers::NONE),
                    transform,
                );
                let original = canvas.drawing.strokes()[0].clone();
                assert_eq!(*original, preview);
                assert_eq!(original.points[0].pressure, Some(0.0));
                assert_eq!(original.points.last().unwrap().pressure, Some(1.0));
                let cached = &canvas.cache[&original.id];
                assert_eq!(cached.pressures, cached_preview);
                assert_eq!(cached.path, original.path());
                assert_eq!(cached.pressures.len(), cached.path.len());
                assert!(cached.pressures.windows(2).all(|p| p[0] <= p[1]));
                assert_eq!(cached.pressures, Cached::new(&original).pressures);
                let shapes =
                    rounded_shapes(&cached.path, &cached.pressures, transform, &original, false);
                let radii: Vec<_> = shapes
                    .iter()
                    .filter_map(|s| {
                        if let egui::Shape::Circle(c) = s {
                            Some(c.radius)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!((f64::from(radii[0]) / scale - 1.2).abs() < 1e-5);
                assert!((f64::from(*radii.last().unwrap()) / scale - 6.0).abs() < 1e-5);
                // Thin ink must not hit the nominal full-width envelope.
                assert!(!pressure_hit(
                    &cached.path,
                    &cached.pressures,
                    &original,
                    [10.0, 44.0],
                    [10.0, 44.0],
                    0.1
                ));
                assert!(pressure_hit(
                    &cached.path,
                    &cached.pressures,
                    &original,
                    [90.0, 44.0],
                    [90.0, 44.0],
                    0.1
                ));
                canvas.tool = Tool::Eraser;
                canvas.eraser_mode = EraserMode::Partial;
                canvas.eraser_radius = 2.0 * scale;
                let mut erase = vec![button(pos(50.0), true)];
                erase.push(button(pos(50.0), false));
                frame_transform(&ctx, &mut canvas, surface, erase, transform);
                assert_eq!(canvas.drawing.strokes().len(), 2);
                let fragments = canvas.drawing.strokes().to_vec();
                for fragment in &fragments {
                    assert_eq!(fragment.pressure_style, original.pressure_style);
                    for point in &fragment.points {
                        let expected = (point.position[0] - 10.0) / 80.0;
                        assert!((f64::from(point.pressure.unwrap()) - expected).abs() < 1e-6);
                    }
                }
                canvas.undo();
                assert_eq!(canvas.drawing.strokes(), &[original]);
                canvas.redo();
                assert_eq!(canvas.drawing.strokes(), fragments);
                canvas.eraser_mode = EraserMode::Stroke;
                frame_transform(
                    &ctx,
                    &mut canvas,
                    surface,
                    vec![button(pos(15.0), true), button(pos(85.0), false)],
                    transform,
                );
                assert!(canvas.drawing.strokes().is_empty());
                canvas.undo();
                assert_eq!(canvas.drawing.strokes(), fragments);
                canvas.redo();
                assert!(canvas.drawing.strokes().is_empty());
            }
        }
    }

    #[test]
    fn mouse_fallback_and_pressure_disabled_keep_constant_width() {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let mut canvas = Canvas::new(&data);
        let ctx = egui::Context::default();
        frame(
            &ctx,
            &mut canvas,
            Surface::Canvas,
            vec![
                button(Pos2::new(30.0, 30.0), true),
                Event::PointerMoved(Pos2::new(90.0, 30.0)),
                button(Pos2::new(90.0, 30.0), false),
            ],
        );
        let stroke = &canvas.drawing.strokes()[0];
        assert!(stroke.points.iter().all(|p| p.pressure.is_none()));
        assert!(
            canvas.cache[&stroke.id]
                .pressures
                .iter()
                .all(Option::is_none)
        );
        for pressure in [None, Some(0.0), Some(0.5), Some(1.0)] {
            let disabled = drawing::PressureStyle::default();
            assert_eq!(disabled.factor(pressure), 1.0);
        }
    }

    #[test]
    #[ignore = "manual release-mode performance comparison"]
    fn eraser_performance() {
        use std::time::Instant;
        for count in [100, 10_000] {
            let data = DocumentData::blank(DocumentKind::Whiteboard);
            let mut canvas = Canvas::new(&data);
            for i in 0..count {
                let mut stroke = Stroke::new(data.id(), Surface::Canvas, [0, 0, 0, 255], 2.5);
                for j in 0..100 {
                    stroke.push([
                        (i % 100) as f64 * 200.0 + j as f64,
                        (i / 100) as f64 * 200.0 + (j as f64 * 0.2).sin(),
                    ]);
                }
                assert!(canvas.drawing.add(stroke));
            }
            // Match a rendered document's warm logical geometry cache.
            for stroke in canvas.drawing.strokes() {
                canvas.cache.insert(stroke.id, Cached::new(stroke));
            }
            let start = Instant::now();
            let mut previous = [-100.0, -100.0];
            let mut ids = HashSet::new();
            for i in 0..200 {
                let point = [i as f64 * 0.25, -50.0];
                // Original M1 hot path: full scan, including already hit strokes.
                for stroke in canvas.drawing.strokes() {
                    if drawing::reference_hit_path(
                        &canvas.cache[&stroke.id].path,
                        previous,
                        point,
                        14.0 + stroke.visible_width() * 0.5,
                    ) {
                        ids.insert(stroke.id);
                    }
                }
                previous = point;
            }
            std::hint::black_box(ids);
            eprintln!(
                "{count} strokes, 200 original samples: {:?}",
                start.elapsed()
            );
            let start = Instant::now();
            canvas
                .eraser_index
                .sync(canvas.drawing.revision(), canvas.drawing.strokes());
            eprintln!("{count} strokes, index build: {:?}", start.elapsed());
            canvas.gesture = Some(Gesture::Eraser(Erasing {
                surface: Surface::Canvas,
                previous: [-100.0, -100.0],
                ids: HashSet::new(),
                mode: EraserMode::Stroke,
                partials: HashMap::new(),
            }));
            let start = Instant::now();
            for i in 0..200 {
                canvas.sample(Surface::Canvas, [i as f64 * 0.25, -50.0], 1.0);
            }
            eprintln!("{count} strokes, 200 warm samples: {:?}", start.elapsed());
            // Separately compare bulk command application/undo with the old
            // vector shifting. The old loop excludes ID selection; the new
            // public command includes it. Both exclude fixture setup.
            let removed: Vec<_> = canvas
                .drawing
                .strokes()
                .iter()
                .enumerate()
                .filter(|(i, _)| i % 2 == 0)
                .map(|(i, s)| (i, s.clone()))
                .collect();
            let mut old = canvas.drawing.strokes().to_vec();
            let start = Instant::now();
            for (i, _) in removed.iter().rev() {
                old.remove(*i);
            }
            for (i, s) in &removed {
                old.insert(*i, s.clone());
            }
            std::hint::black_box(old);
            eprintln!(
                "{count} strokes, original half erase + undo: {:?}",
                start.elapsed()
            );
            let ids = removed.iter().map(|(_, s)| s.id).collect();
            let start = Instant::now();
            canvas.drawing.erase(&ids);
            canvas.drawing.undo();
            eprintln!(
                "{count} strokes, batched half erase + undo: {:?}",
                start.elapsed()
            );
            let start = Instant::now();
            canvas.sample(Surface::Canvas, [20_000.0, 20_000.0], 1.0);
            canvas.finish();
            eprintln!(
                "{count} strokes, long sweep + commit: {:?}",
                start.elapsed()
            );
        }
    }

    #[test]
    fn indexed_eraser_matches_full_scan_at_zoom_and_restores_order() {
        for (kind, surface) in [
            (DocumentKind::Page, Surface::Page(0)),
            (DocumentKind::Whiteboard, Surface::Canvas),
        ] {
            for count in [1, 100, 10_000] {
                for scale in [0.1, 1.0, 8.0] {
                    let data = DocumentData::blank(kind);
                    let mut canvas = Canvas::new(&data);
                    for i in 0..count {
                        let mut stroke =
                            Stroke::new(data.id(), surface, [0, 0, 0, 255], 0.1 + (i % 24) as f64);
                        if i % 6 != 0 {
                            stroke.smoothing = drawing::Smoothing::AdaptiveV1 {
                                strength: ((i % 5) * 25) as u8,
                            };
                        }
                        let x = (i % 100) as f64 * 5.0;
                        let y = (i / 100) as f64 * 5.0;
                        stroke.push([x, y]);
                        if i % 3 != 0 {
                            stroke.push([x + 2.0, y + 10.0]);
                            stroke.push([x + 4.0, y]);
                        }
                        assert!(canvas.drawing.add(stroke));
                    }
                    let original = canvas.drawing.strokes().to_vec();
                    let sweep = [[-10.0, 30.0], [300.0, 30.0], [300.0, 400.0], [0.0, 0.0]];
                    let mut expected = HashSet::new();
                    for pair in sweep.windows(2) {
                        for stroke in &original {
                            if drawing::reference_hit_path(
                                &stroke.path(),
                                pair[0],
                                pair[1],
                                14.0 / scale + stroke.visible_width() * 0.5,
                            ) {
                                expected.insert(stroke.id);
                            }
                        }
                    }
                    canvas.gesture = Some(Gesture::Eraser(Erasing {
                        surface,
                        previous: sweep[0],
                        ids: HashSet::new(),
                        mode: EraserMode::Stroke,
                        partials: HashMap::new(),
                    }));
                    for point in &sweep[1..] {
                        canvas.sample(surface, *point, scale);
                    }
                    let Some(Gesture::Eraser(erase)) = &canvas.gesture else {
                        panic!()
                    };
                    assert_eq!(erase.ids, expected);
                    canvas.finish();
                    assert_eq!(
                        canvas
                            .drawing
                            .strokes()
                            .iter()
                            .map(|s| s.id)
                            .collect::<Vec<_>>(),
                        original
                            .iter()
                            .filter(|s| !expected.contains(&s.id))
                            .map(|s| s.id)
                            .collect::<Vec<_>>()
                    );
                    canvas.undo();
                    assert_eq!(canvas.drawing.strokes(), original);
                    canvas.redo();
                    assert!(
                        canvas
                            .drawing
                            .strokes()
                            .iter()
                            .all(|s| !expected.contains(&s.id))
                    );
                    // Rebuild after redo/undo, then erase a restored dot.
                    canvas.undo();
                    canvas.gesture = Some(Gesture::Eraser(Erasing {
                        surface,
                        previous: [0.0, 0.0],
                        ids: HashSet::new(),
                        mode: EraserMode::Stroke,
                        partials: HashMap::new(),
                    }));
                    canvas.sample(surface, [0.0, 0.0], scale);
                    let Some(Gesture::Eraser(erase)) = &canvas.gesture else {
                        panic!()
                    };
                    assert!(erase.ids.contains(&original[0].id));
                }
            }
        }
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn frame(ctx: &egui::Context, canvas: &mut Canvas, surface: Surface, events: Vec<Event>) {
        frame_transform(
            ctx,
            canvas,
            surface,
            events,
            Transform {
                origin: [0.0; 2],
                center: [0.0; 2],
                scale: 1.0,
            },
        );
    }
    fn frame_transform(
        ctx: &egui::Context,
        canvas: &mut Canvas,
        surface: Surface,
        events: Vec<Event>,
        transform: Transform,
    ) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(700.0, 500.0))),
                focused: true,
                events,
                ..Default::default()
            },
            |ui| {
                canvas.begin_frame(ui);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(600.0, 400.0), egui::Sense::click_and_drag());
                canvas.surface(ui, rect, surface, transform);
                canvas.end_frame(ui);
            },
        );
        output.textures_delta.clear();
    }
    #[test]
    fn partial_gestures_replace_multiple_smoothed_strokes_once_and_restore_exact_history() {
        for (kind, surface) in [
            (DocumentKind::Page, Surface::Page(0)),
            (DocumentKind::Whiteboard, Surface::Canvas),
        ] {
            for scale in [0.25, 1.0, 4.0] {
                let data = DocumentData::blank(kind);
                let mut canvas = Canvas::new(&data);
                let offset = if kind == DocumentKind::Whiteboard {
                    [1e12, -1e12]
                } else {
                    [0.0; 2]
                };
                let world = |x, y| [offset[0] + x, offset[1] + y];
                for (i, width) in [0.25, 2.0, 24.0].into_iter().enumerate() {
                    let y = 20.0 + i as f64 * 20.0;
                    let mut stroke =
                        Stroke::new(data.id(), surface, [20 + i as u8, 60, 180, 255], width);
                    stroke.smoothing = drawing::Smoothing::AdaptiveV1 { strength: 50 };
                    for (x, dy) in [
                        (10.0, 0.0),
                        (40.0, 0.2),
                        (70.0, -0.2),
                        (100.0, 0.2),
                        (130.0, 0.0),
                    ] {
                        stroke.push(world(x, y + dy));
                    }
                    assert!(canvas.drawing.add(stroke));
                }
                let originals = canvas.drawing.strokes().to_vec();
                let revision = canvas.drawing.revision();
                let transform = Transform {
                    origin: [20.0; 2],
                    center: offset,
                    scale,
                };
                let screen = |p| {
                    let s = transform.to_screen(p);
                    Pos2::new(s[0] as f32, s[1] as f32)
                };
                let ctx = egui::Context::default();
                frame_transform(&ctx, &mut canvas, surface, vec![], transform);
                canvas.select(Tool::Eraser);
                canvas.eraser_mode = EraserMode::Partial;
                canvas.eraser_radius = 4.0;
                let a = screen(world(70.0, 5.0));
                let b = screen(world(70.0, 75.0));
                frame_transform(
                    &ctx,
                    &mut canvas,
                    surface,
                    vec![button(a, true), Event::PointerMoved(b)],
                    transform,
                );
                assert_eq!(canvas.drawing.revision(), revision);
                assert_eq!(canvas.drawing.strokes(), originals);
                let Some(Gesture::Eraser(erase)) = &canvas.gesture else {
                    panic!()
                };
                assert_eq!(erase.ids.len(), 3);
                assert!(erase.partials.values().all(|p| p.cut.cuts.0.len() == 1));
                frame_transform(
                    &ctx,
                    &mut canvas,
                    surface,
                    vec![button(b, false)],
                    transform,
                );
                assert_eq!(canvas.drawing.revision(), revision + 1);
                let pieces = canvas.drawing.strokes().to_vec();
                assert_eq!(pieces.len(), 6);
                for (i, source) in originals.iter().enumerate() {
                    let (left, right) = (&pieces[2 * i], &pieces[2 * i + 1]);
                    assert_eq!(left.width, source.width);
                    assert_eq!(right.color, source.color);
                    assert_eq!(left.points.first().unwrap().position, source.path()[0]);
                    assert_eq!(
                        right.points.last().unwrap().position,
                        *source.path().last().unwrap()
                    );
                    let radius = 4.0 / scale + source.width * 0.5;
                    assert!(
                        (left.points.last().unwrap().position[0] - (offset[0] + 70.0 - radius))
                            .abs()
                            < 0.001
                    );
                    assert!(
                        (right.points[0].position[0] - (offset[0] + 70.0 + radius)).abs() < 0.001
                    );
                    assert_ne!(left.id, source.id);
                    assert_ne!(right.id, source.id);
                }
                canvas.undo();
                assert_eq!(canvas.drawing.strokes(), originals);
                assert!(
                    canvas
                        .drawing
                        .strokes()
                        .iter()
                        .zip(&originals)
                        .all(|(a, b)| Arc::ptr_eq(a, b))
                );
                frame_transform(&ctx, &mut canvas, surface, vec![], transform);
                for source in &originals {
                    assert_eq!(canvas.cache[&source.id].path, source.path());
                }
                canvas.redo();
                assert_eq!(canvas.drawing.strokes(), pieces);
                // Whole-stroke erasing remains functional on generated polylines.
                canvas.eraser_mode = EraserMode::Stroke;
                let p = screen(world(20.0, 20.0));
                frame_transform(
                    &ctx,
                    &mut canvas,
                    surface,
                    vec![button(p, true), button(p, false)],
                    transform,
                );
                assert!(canvas.drawing.strokes().len() < pieces.len());
                canvas.undo();
                assert_eq!(canvas.drawing.strokes(), pieces);
                canvas.undo();
                assert_eq!(canvas.drawing.strokes(), originals);
                let mut dot = Stroke::new(data.id(), surface, [0, 0, 0, 255], 2.0);
                dot.push(world(10.0, 10.0));
                canvas.drawing.add(dot);
                assert!(!canvas.drawing.can_redo());
            }
        }
    }

    #[test]
    fn partial_preview_reuses_untouched_meshes_and_repeated_samples_do_not_rebuild() {
        let ctx = egui::Context::default();
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let mut canvas = Canvas::new(&data);
        let mut stroke = Stroke::new(data.id(), Surface::Canvas, [0, 0, 0, 255], 2.0);
        stroke.smoothing = drawing::Smoothing::PolylineV1;
        for i in 0..1000 {
            stroke.push([30.0 + i as f64 * 0.5, 100.0]);
        }
        let id = stroke.id;
        assert!(canvas.drawing.add(stroke));
        frame(&ctx, &mut canvas, Surface::Canvas, vec![]);
        let original_mesh = canvas.cache[&id].meshes[0].clone();
        canvas.select(Tool::Eraser);
        canvas.eraser_mode = EraserMode::Partial;
        canvas.eraser_radius = 4.0;
        let p = Pos2::new(300.0, 100.0);
        frame(&ctx, &mut canvas, Surface::Canvas, vec![button(p, true)]);
        assert!(Arc::ptr_eq(&original_mesh, &canvas.cache[&id].meshes[0]));
        let Some(Gesture::Eraser(e)) = &canvas.gesture else {
            panic!()
        };
        let meshes: Vec<_> = e.partials[&id]
            .meshes
            .iter()
            .map(|(i, entry)| (*i, entry.2.clone()))
            .collect();
        assert!(!meshes.is_empty());
        frame(
            &ctx,
            &mut canvas,
            Surface::Canvas,
            vec![Event::PointerMoved(p)],
        );
        let Some(Gesture::Eraser(e)) = &canvas.gesture else {
            panic!()
        };
        for (i, mesh) in meshes {
            assert!(Arc::ptr_eq(&mesh, &e.partials[&id].meshes[&i].2));
        }
        frame(&ctx, &mut canvas, Surface::Canvas, vec![button(p, false)]);
        assert_eq!(canvas.drawing.strokes().len(), 2);
    }

    #[test]
    #[ignore = "manual release-mode large-document partial-erasing benchmark"]
    fn partial_document_performance() {
        for count in [100, 10_000] {
            let data = DocumentData::blank(DocumentKind::Whiteboard);
            let mut canvas = Canvas::new(&data);
            for i in 0..count {
                let mut stroke = Stroke::new(data.id(), Surface::Canvas, [0, 0, 0, 255], 2.5);
                for j in 0..100 {
                    stroke.push([
                        (i % 100) as f64 * 200.0 + j as f64,
                        (i / 100) as f64 * 200.0,
                    ]);
                }
                assert!(canvas.drawing.add(stroke));
            }
            for stroke in canvas.drawing.strokes() {
                canvas.cache.insert(stroke.id, Cached::new(stroke));
            }
            let start = std::time::Instant::now();
            canvas
                .eraser_index
                .sync(canvas.drawing.revision(), canvas.drawing.strokes());
            eprintln!("{count} strokes, document index: {:?}", start.elapsed());
            canvas.eraser_radius = 4.0;
            canvas.gesture = Some(Gesture::Eraser(Erasing {
                surface: Surface::Canvas,
                previous: [20.0, 0.0],
                ids: HashSet::new(),
                mode: EraserMode::Partial,
                partials: HashMap::new(),
            }));
            let start = std::time::Instant::now();
            for i in 0..2000 {
                canvas.sample(Surface::Canvas, [20.0 + i as f64 * 0.01, 0.0], 1.0);
            }
            eprintln!(
                "{count} strokes, 2000 partial samples: {:?}",
                start.elapsed()
            );
            let start = std::time::Instant::now();
            canvas.finish();
            eprintln!("{count} strokes, commit: {:?}", start.elapsed());
        }
    }

    #[test]
    fn smoothing_is_captured_on_press_matches_preview_and_survives_history() {
        for (kind, surface) in [
            (DocumentKind::Page, Surface::Page(0)),
            (DocumentKind::Whiteboard, Surface::Canvas),
        ] {
            for scale in [0.1, 1.0, 8.0] {
                for strength in [0, 50, 100] {
                    let data = DocumentData::blank(kind);
                    let mut canvas = Canvas::new(&data);
                    canvas.smoothing_strength = strength;
                    let ctx = egui::Context::default();
                    let transform = Transform {
                        origin: [100.0; 2],
                        center: [0.0; 2],
                        scale,
                    };
                    let screen = |p| {
                        let s = transform.to_screen(p);
                        Pos2::new(s[0] as f32, s[1] as f32)
                    };
                    let a = screen([10.0, 10.0]);
                    let b = screen([20.0, 10.3]);
                    let c = screen([30.0, 9.7]);
                    let d = screen([40.0, 10.0]);
                    frame_transform(&ctx, &mut canvas, surface, vec![], transform);
                    frame_transform(
                        &ctx,
                        &mut canvas,
                        surface,
                        vec![button(a, true), Event::PointerMoved(b)],
                        transform,
                    );
                    // A setting change during drawing affects only the next stroke.
                    canvas.smoothing_strength = 100 - strength;
                    frame_transform(
                        &ctx,
                        &mut canvas,
                        surface,
                        vec![Event::PointerMoved(c), Event::PointerMoved(d)],
                        transform,
                    );
                    let Some(Gesture::Pen(live)) = &canvas.gesture else {
                        panic!()
                    };
                    let id = live.id;
                    let preview = canvas.cache[&id].path.clone();
                    frame_transform(
                        &ctx,
                        &mut canvas,
                        surface,
                        vec![button(d, false)],
                        transform,
                    );
                    let stroke = canvas.drawing.strokes()[0].clone();
                    assert_eq!(stroke.id, id);
                    assert_eq!(
                        stroke.smoothing,
                        drawing::Smoothing::AdaptiveV1 { strength }
                    );
                    assert_eq!(stroke.points.len(), 4);
                    assert_eq!(preview, stroke.path());
                    assert_eq!(canvas.cache[&id].path, preview);
                    let json = serde_json::to_vec(stroke.as_ref()).unwrap();
                    let decoded: Stroke = serde_json::from_slice(&json).unwrap();
                    assert_eq!(&decoded, stroke.as_ref());
                    assert_eq!(decoded.path(), preview);
                    canvas.undo();
                    assert!(canvas.drawing.strokes().is_empty());
                    canvas.redo();
                    assert_eq!(canvas.drawing.strokes()[0], stroke);
                    frame_transform(&ctx, &mut canvas, surface, vec![], transform);
                    assert_eq!(canvas.cache[&id].path, preview);
                    frame_transform(
                        &ctx,
                        &mut canvas,
                        surface,
                        vec![button(a, true), button(a, false)],
                        transform,
                    );
                    assert_eq!(
                        canvas.drawing.strokes()[1].smoothing,
                        drawing::Smoothing::AdaptiveV1 {
                            strength: 100 - strength
                        }
                    );
                    assert_eq!(canvas.drawing.strokes()[0], stroke);
                }
            }
        }
    }

    #[test]
    fn long_live_strokes_reuse_stable_meshes_and_idle_frames_allocate_no_new_meshes() {
        let ctx = egui::Context::default();
        let mut canvas = Canvas::new(&DocumentData::blank(DocumentKind::Whiteboard));
        frame(&ctx, &mut canvas, Surface::Canvas, vec![]);
        let point = |i| Pos2::new(50.0 + i as f32 * 0.3, 50.0 + (i as f32 * 0.1).sin());
        let mut events = vec![button(point(0), true)];
        events.extend((1..1000).map(|i| Event::PointerMoved(point(i))));
        frame(&ctx, &mut canvas, Surface::Canvas, events);
        let Some(Gesture::Pen(live)) = &canvas.gesture else {
            panic!()
        };
        let id = live.id;
        let first = canvas.cache[&id].meshes[0].clone();
        frame(
            &ctx,
            &mut canvas,
            Surface::Canvas,
            (1000..1200)
                .map(|i| Event::PointerMoved(point(i)))
                .collect(),
        );
        assert!(Arc::ptr_eq(&first, &canvas.cache[&id].meshes[0]));
        let meshes = canvas.cache[&id].meshes.clone();
        frame(&ctx, &mut canvas, Surface::Canvas, vec![]);
        assert!(
            meshes
                .iter()
                .zip(&canvas.cache[&id].meshes)
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
        frame(
            &ctx,
            &mut canvas,
            Surface::Canvas,
            vec![button(point(1199), false)],
        );
        assert!(
            meshes
                .iter()
                .zip(&canvas.cache[&id].meshes)
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
    }

    #[test]
    fn ordered_events_draw_erase_and_keyboard_undo_redo_on_both_surfaces() {
        for (kind, surface) in [
            (DocumentKind::Page, Surface::Page(0)),
            (DocumentKind::Whiteboard, Surface::Canvas),
        ] {
            let ctx = egui::Context::default();
            let mut canvas = Canvas::new(&DocumentData::blank(kind));
            frame(&ctx, &mut canvas, surface, vec![]);
            let a = egui::pos2(50.0, 50.0);
            let b = egui::pos2(150.0, 50.0);
            frame(
                &ctx,
                &mut canvas,
                surface,
                vec![
                    Event::PointerMoved(a),
                    button(a, true),
                    Event::PointerMoved(egui::pos2(100.0, 70.0)),
                    Event::PointerMoved(b),
                    button(b, false),
                ],
            );
            assert_eq!(canvas.drawing.strokes().len(), 1);
            assert_eq!(canvas.drawing.strokes()[0].points.len(), 3);
            assert!(canvas.dirty());
            let stroke = canvas.drawing.strokes()[0].clone();
            frame(&ctx, &mut canvas, surface, vec![]);
            assert!(!canvas.cache.is_empty());
            canvas.select(Tool::Eraser);
            let c = egui::pos2(100.0, 10.0);
            let d = egui::pos2(100.0, 120.0);
            frame(
                &ctx,
                &mut canvas,
                surface,
                vec![
                    Event::PointerMoved(c),
                    button(c, true),
                    Event::PointerMoved(d),
                    button(d, false),
                ],
            );
            assert!(canvas.drawing.strokes().is_empty());
            let shortcut = |shift| Event::Key {
                key: egui::Key::Z,
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
            frame(&ctx, &mut canvas, surface, vec![shortcut(false)]);
            assert_eq!(canvas.drawing.strokes(), &[stroke]);
            frame(&ctx, &mut canvas, surface, vec![shortcut(true)]);
            assert!(canvas.drawing.strokes().is_empty());
        }
    }
    #[test]
    fn page_drag_ends_at_boundary_and_does_not_bridge_margin_excursions() {
        let ctx = egui::Context::default();
        let mut canvas = Canvas::new(&DocumentData::blank(DocumentKind::Page));
        let surface = Surface::Page(0);
        frame(&ctx, &mut canvas, surface, vec![]);
        let a = egui::pos2(10.0, 20.0);
        let outside = egui::pos2(-30.0, 30.0);
        let reentry = egui::pos2(20.0, 40.0);
        frame(
            &ctx,
            &mut canvas,
            surface,
            vec![
                Event::PointerMoved(a),
                button(a, true),
                Event::PointerMoved(outside),
                Event::PointerMoved(reentry),
                button(reentry, false),
            ],
        );
        assert_eq!(canvas.drawing.strokes().len(), 1);
        let points = &canvas.drawing.strokes()[0].points;
        assert_eq!(points.last().unwrap().position[0], 0.0);
        assert!(
            points
                .iter()
                .all(|p| canvas.drawing.contains(surface, p.position))
        );
    }
    #[test]
    fn mouse_tap_creates_a_dot_and_hand_never_draws() {
        let ctx = egui::Context::default();
        let mut canvas = Canvas::new(&DocumentData::blank(DocumentKind::Whiteboard));
        let surface = Surface::Canvas;
        let a = egui::pos2(50.0, 50.0);
        frame(&ctx, &mut canvas, surface, vec![]);
        frame(
            &ctx,
            &mut canvas,
            surface,
            vec![Event::PointerMoved(a), button(a, true), button(a, false)],
        );
        assert_eq!(canvas.drawing.strokes()[0].points.len(), 1);
        canvas.select(Tool::Pan);
        frame(
            &ctx,
            &mut canvas,
            surface,
            vec![
                button(a, true),
                Event::PointerMoved(egui::pos2(150.0, 150.0)),
                button(a, false),
            ],
        );
        assert_eq!(canvas.drawing.strokes().len(), 1);
    }
}
