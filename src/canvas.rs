//! Shared pointer controller and rounded vector renderer for every surface.
use crate::{
    document::DocumentData,
    drawing::{self, Drawing, Stroke, Surface, Transform},
};
use egui::{Color32, Event, PointerButton, Pos2, Rect, Stroke as Line};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Pen,
    Eraser,
    Pan,
}

struct Erasing {
    surface: Surface,
    previous: [f64; 2],
    ids: HashSet<Uuid>,
}
enum Gesture {
    Pen(Stroke),
    Eraser(Erasing),
}
struct Cached {
    path: Vec<[f64; 2]>,
    mesh: Option<(Transform, f32, Arc<egui::Mesh>)>,
    bounds: ([f64; 2], [f64; 2]),
}
impl Cached {
    fn new(stroke: &Stroke) -> Self {
        let path = drawing::smooth(&stroke.points);
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for p in &path {
            for axis in 0..2 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
        Self {
            path,
            mesh: None,
            bounds: (min, max),
        }
    }
}

pub struct Canvas {
    pub drawing: Drawing,
    pub tool: Tool,
    pub color: [u8; 4],
    pub width: f64,
    /// Eraser radius in logical screen points, shown at its actual hit-test size.
    pub eraser_radius: f64,
    gesture: Option<Gesture>,
    gesture_transform: Option<Transform>,
    cache: HashMap<Uuid, Cached>,
    frame: Option<u64>,
    processed: Vec<Surface>,
}
impl Canvas {
    pub fn new(data: &DocumentData) -> Self {
        Self {
            drawing: Drawing::new(data),
            tool: Tool::Pen,
            color: [37, 99, 200, 255],
            width: 2.5,
            eraser_radius: 14.0,
            gesture: None,
            gesture_transform: None,
            cache: HashMap::new(),
            frame: None,
            processed: Vec::new(),
        }
    }
    pub fn interacting(&self) -> bool {
        self.gesture.is_some()
    }
    pub fn dirty(&self) -> bool {
        self.drawing.dirty()
            || matches!(&self.gesture, Some(Gesture::Pen(stroke)) if !stroke.points.is_empty())
    }
    pub fn finish(&mut self) {
        match self.gesture.take() {
            Some(Gesture::Pen(stroke)) => {
                self.drawing.add(stroke);
            }
            Some(Gesture::Eraser(erase)) => self.drawing.erase(&erase.ids),
            None => return,
        }
        self.gesture_transform = None;
        self.prune_cache();
    }
    pub fn undo(&mut self) {
        self.finish();
        self.drawing.undo();
        self.prune_cache();
    }
    pub fn redo(&mut self) {
        self.finish();
        self.drawing.redo();
        self.prune_cache();
    }
    pub fn select(&mut self, tool: Tool) {
        self.finish();
        self.tool = tool;
    }

    fn prune_cache(&mut self) {
        let live: HashSet<_> = self
            .drawing
            .strokes()
            .iter()
            .map(|stroke| stroke.id)
            .collect();
        self.cache.retain(|id, _| live.contains(id));
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
        if !ui.ctx().text_edit_focused() {
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
        if !ui.input(|i| i.pointer.button_down(PointerButton::Primary)) {
            self.finish();
        }
    }

    pub fn surface(&mut self, ui: &egui::Ui, bounds: Rect, surface: Surface, transform: Transform) {
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
                        self.gesture_transform = Some(transform);
                        if self.tool == Tool::Pen {
                            let mut stroke =
                                Stroke::new(self.drawing.owner(), surface, self.color, self.width);
                            stroke.push(point);
                            self.gesture = Some(Gesture::Pen(stroke));
                        } else {
                            self.gesture = Some(Gesture::Eraser(Erasing {
                                surface,
                                previous: point,
                                ids: HashSet::new(),
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
                        }
                    }
                    Event::PointerGone => self.finish(),
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
                    stroke.push(point);
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
                        stroke.push(std::array::from_fn(|i| {
                            (last[i] + t * (point[i] - last[i])).clamp(0.0, bounds[i])
                        }));
                    }
                    self.finish();
                }
            }
            Some(Gesture::Eraser(erase)) => {
                for stroke in self
                    .drawing
                    .strokes()
                    .iter()
                    .filter(|s| s.surface == surface)
                {
                    let cached = self
                        .cache
                        .entry(stroke.id)
                        .or_insert_with(|| Cached::new(stroke));
                    if drawing::hit_path(
                        &cached.path,
                        erase.previous,
                        point,
                        self.eraser_radius / scale + stroke.width * 0.5,
                    ) {
                        erase.ids.insert(stroke.id);
                    }
                }
                erase.previous = point;
            }
            None => {}
        }
    }
    fn paint(&mut self, ui: &egui::Ui, bounds: Rect, surface: Surface, transform: Transform) {
        let painter = ui
            .painter()
            .with_clip_rect(bounds.intersect(ui.clip_rect()));
        let erased = match &self.gesture {
            Some(Gesture::Eraser(e)) => Some(&e.ids),
            _ => None,
        };
        for stroke in self
            .drawing
            .strokes()
            .iter()
            .filter(|s| s.surface == surface && !erased.is_some_and(|ids| ids.contains(&s.id)))
        {
            let cached = self
                .cache
                .entry(stroke.id)
                .or_insert_with(|| Cached::new(stroke));
            let min = transform.to_screen(cached.bounds.0);
            let max = transform.to_screen(cached.bounds.1);
            let screen_bounds = Rect::from_min_max(
                Pos2::new(min[0] as f32, min[1] as f32),
                Pos2::new(max[0] as f32, max[1] as f32),
            )
            .expand((stroke.width * transform.scale) as f32);
            if !screen_bounds.intersects(painter.clip_rect()) {
                continue;
            }
            let ppp = ui.ctx().pixels_per_point();
            if cached
                .mesh
                .as_ref()
                .is_none_or(|(old, dpi, _)| *old != transform || *dpi != ppp)
            {
                let shapes = rounded_shapes(&cached.path, transform, stroke);
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
                cached.mesh = Some((transform, ppp, Arc::new(mesh)));
            }
            painter.add(egui::Shape::Mesh(cached.mesh.as_ref().unwrap().2.clone()));
        }
        if let Some(Gesture::Pen(stroke)) = &self.gesture
            && stroke.surface == surface
        {
            painter.extend(rounded_shapes(
                &drawing::smooth(&stroke.points),
                transform,
                stroke,
            ));
        }
    }
}

fn rounded_shapes(path: &[[f64; 2]], transform: Transform, stroke: &Stroke) -> Vec<egui::Shape> {
    let color = Color32::from_rgba_unmultiplied(
        stroke.color[0],
        stroke.color[1],
        stroke.color[2],
        stroke.color[3],
    );
    let width = (stroke.width * transform.scale) as f32;
    let points: Vec<_> = path
        .iter()
        .map(|point| {
            let p = transform.to_screen(*point);
            Pos2::new(p[0] as f32, p[1] as f32)
        })
        .collect();
    let mut shapes = Vec::with_capacity(points.len() * 2);
    for pair in points.windows(2) {
        shapes.push(egui::Shape::line_segment(
            [pair[0], pair[1]],
            Line::new(width, color),
        ));
    }
    for point in points {
        shapes.push(egui::Shape::circle_filled(point, width * 0.5, color));
    }
    shapes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentKind;

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn frame(ctx: &egui::Context, canvas: &mut Canvas, surface: Surface, events: Vec<Event>) {
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
                canvas.surface(
                    ui,
                    rect,
                    surface,
                    Transform {
                        origin: [0.0; 2],
                        center: [0.0; 2],
                        scale: 1.0,
                    },
                );
                canvas.end_frame(ui);
            },
        );
        output.textures_delta.clear();
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
