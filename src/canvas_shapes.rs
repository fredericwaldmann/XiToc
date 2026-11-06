//! Shape adapters inside the shared canvas controller and mesh pipeline.
use super::*;
use crate::{
    object::{Placement, Point},
    shape::{BorderStyle, Shape},
};

pub(super) struct ShapeDraft {
    surface: Surface,
    start: Point,
    last: Point,
    view: Transform,
    id: Uuid,
    preview: Option<Shape>,
    defaults: crate::shape::ShapeDefaults,
}
pub(super) struct ShapeCache {
    source: Shape,
    view: Transform,
    delta: Placement,
    ppp: f32,
    pub(super) meshes: Vec<Arc<egui::Mesh>>,
}
pub(super) fn paint_shape(
    ui: &egui::Ui,
    painter: &egui::Painter,
    view: Transform,
    shape: &Shape,
    delta: Placement,
    cache: &mut HashMap<Uuid, ShapeCache>,
) {
    let b = shape
        .geometry_bounds()
        .placed(delta)
        .expand(shape.style.border_width * shape.placement.scale * delta.scale);
    let screen = |p: Point| {
        let p = view.to_screen(p);
        Pos2::new(p[0] as f32, p[1] as f32)
    };
    if !Rect::from_min_max(screen(b.min), screen(b.max)).intersects(painter.clip_rect()) {
        return;
    }
    let ppp = ui.ctx().pixels_per_point();
    let rebuild = cache
        .get(&shape.id)
        .is_none_or(|c| c.source != *shape || c.view != view || c.delta != delta || c.ppp != ppp);
    if rebuild {
        let paths = shape.contours(0.15 / (view.scale * delta.scale * f64::from(ppp)).max(0.001));
        let mut shapes = Vec::new();
        let border = Color32::from_rgba_unmultiplied(
            shape.style.border[0],
            shape.style.border[1],
            shape.style.border[2],
            shape.style.border[3],
        );
        let width =
            (shape.style.border_width * shape.placement.scale * delta.scale * view.scale) as f32;
        for (index, path) in paths.into_iter().enumerate() {
            let points: Vec<_> = path.into_iter().map(|p| screen(delta.apply(p))).collect();
            let head = shape.kind().segment() && index > 0;
            let fill = if head && shape.style.border_enabled {
                Some(border)
            } else if shape.filled() {
                shape.style.fill.map(|c| {
                    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], shape.style.fill_opacity)
                })
            } else {
                None
            };
            if let Some(fill) = fill {
                let mut polygon = points.clone();
                if polygon.first() == polygon.last() {
                    polygon.pop();
                }
                shapes.push(egui::Shape::convex_polygon(polygon, fill, Line::NONE));
            }
            if !shape.style.border_enabled {
                continue;
            }
            match if head {
                BorderStyle::Solid
            } else {
                shape.style.border_style
            } {
                BorderStyle::Solid => {
                    shapes.push(egui::Shape::line(points, Line::new(width, border)))
                }
                BorderStyle::Dashed => shapes.extend(egui::Shape::dashed_line(
                    &points,
                    Line::new(width, border),
                    width * 4.,
                    width * 2.,
                )),
                BorderStyle::Dotted => shapes.extend(egui::Shape::dotted_line(
                    &points,
                    border,
                    width * 2.5,
                    width * 0.5,
                )),
            }
        }
        let primitives = ui.ctx().tessellate(
            shapes
                .into_iter()
                .map(|s| egui::epaint::ClippedShape::new(Rect::EVERYTHING, s))
                .collect(),
            ppp,
        );
        let meshes = primitives
            .into_iter()
            .filter_map(|p| {
                if let egui::epaint::Primitive::Mesh(m) = p.primitive {
                    Some(Arc::new(m))
                } else {
                    None
                }
            })
            .collect();
        cache.insert(
            shape.id,
            ShapeCache {
                source: shape.clone(),
                view,
                delta,
                ppp,
                meshes,
            },
        );
    }
    for mesh in &cache[&shape.id].meshes {
        painter.add(egui::Shape::Mesh(mesh.clone()));
    }
}
impl Canvas {
    fn sample_shape(&mut self, surface: Surface, point: Point, shift: bool) {
        let Some(draft) = &mut self.shape_draft else {
            return;
        };
        if draft.surface != surface {
            return;
        }
        draft.last = point;
        let mut end = point;
        if let Some(page) = self.drawing.bounds(surface) {
            for i in 0..2 {
                end[i] = end[i].clamp(0., page[i]);
            }
        }
        let moved =
            (end[0] - draft.start[0]).hypot(end[1] - draft.start[1]) * draft.view.scale >= 3.;
        if !moved {
            draft.preview = None;
            return;
        }
        if let Some(mut shape) = Shape::create(
            self.drawing.owner(),
            surface,
            self.shape_kind,
            draft.start,
            end,
            shift,
        ) {
            shape.id = draft.id;
            draft.defaults.apply(&mut shape);
            if self
                .drawing
                .valid_object(&ObjectSnapshot::from(shape.clone()))
            {
                draft.preview = Some(shape);
            }
        } else {
            draft.preview = None;
        }
    }
    pub(super) fn shape_surface(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) {
        if self.processed.contains(&surface) {
            return;
        }
        self.processed.push(surface);
        if self
            .shape_draft
            .as_ref()
            .is_some_and(|d| d.surface == surface && d.view != view)
        {
            self.cancel_shape_draft();
        }
        let owns = |p: Pos2| {
            bounds.contains(p)
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
                    ..
                } if owns(pos) && !ui.input(|i| i.key_down(egui::Key::Space)) => {
                    let start = view.to_document([pos.x as f64, pos.y as f64]);
                    if self.drawing.contains(surface, start) {
                        self.shape_draft = Some(ShapeDraft {
                            surface,
                            start,
                            last: start,
                            view,
                            id: Uuid::new_v4(),
                            preview: None,
                            defaults: self.shape_defaults.clone(),
                        });
                    }
                }
                Event::PointerMoved(pos) => self.sample_shape(
                    surface,
                    view.to_document([pos.x as f64, pos.y as f64]),
                    ui.input(|i| i.modifiers.shift),
                ),
                Event::ModifiersChanged(m) => {
                    if let Some(d) = &self.shape_draft {
                        let p = d.last;
                        self.sample_shape(surface, p, m.shift);
                    }
                }
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers,
                } => {
                    if self
                        .shape_draft
                        .as_ref()
                        .is_some_and(|d| d.surface == surface)
                    {
                        self.sample_shape(
                            surface,
                            view.to_document([pos.x as f64, pos.y as f64]),
                            modifiers.shift,
                        );
                        if let Some(draft) = self.shape_draft.take() {
                            if let Some(shape) = draft.preview {
                                let id = shape.id;
                                if self.drawing.add_shape(shape) {
                                    self.select(self.selection_mode);
                                    self.selection.scope(surface);
                                    self.selection.change(
                                        [id],
                                        crate::selection::Membership::Replace,
                                        &self.drawing,
                                    );
                                    ui.ctx().request_repaint();
                                } else {
                                    self.shape_cache.remove(&id);
                                }
                            } else {
                                self.shape_cache.remove(&draft.id);
                            }
                        }
                    }
                }
                Event::PointerGone | Event::WindowFocused(false) => self.cancel_shape_draft(),
                _ => {}
            }
        }
        if self.tool == Tool::Shape && ui.input(|i| i.pointer.hover_pos()).is_some_and(owns) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }
    pub(super) fn paint_shape_draft(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) {
        if let Some(d) = &self.shape_draft
            && d.surface == surface
            && let Some(shape) = &d.preview
        {
            paint_shape(
                ui,
                &ui.painter()
                    .with_clip_rect(bounds.intersect(ui.clip_rect())),
                view,
                shape,
                Placement::default(),
                &mut self.shape_cache,
            );
        }
    }
    pub(super) fn shape_edit_preview(&self) -> Option<&Shape> {
        match &self.selection_gesture {
            Some(SelectionGesture::ShapeEdit { preview, .. }) => Some(preview),
            _ => None,
        }
    }
    pub(super) fn shape_handle(
        &self,
        point: Point,
        scale: f64,
    ) -> Option<(Arc<Shape>, usize, bool)> {
        if self.selection.ids.len() != 1 {
            return None;
        }
        let object = self
            .drawing
            .items()
            .iter()
            .find(|o| self.selection.ids.contains(&o.id()))?;
        let ObjectSnapshot::Shape(shape) = object else {
            return None;
        };
        let corner = shape.endpoints().is_none();
        let ends: Vec<_> = if let Some(ends) = shape.endpoints() {
            ends.to_vec()
        } else {
            shape.box_corners()?.to_vec()
        };
        ends.iter()
            .position(|p| {
                (p[0] - point[0]).hypot(p[1] - point[1]) * scale
                    <= f64::from(crate::ui::style::SELECTION_HIT)
            })
            .map(|i| (shape.clone(), i, corner))
    }
    pub(super) fn single_shape_handles(&self) -> bool {
        self.selection.ids.len() == 1
            && self.drawing.items().iter().any(|o| {
                self.selection.ids.contains(&o.id()) && matches!(o, ObjectSnapshot::Shape(_))
            })
    }
    pub(super) fn cancel_shape_draft(&mut self) {
        if let Some(d) = self.shape_draft.take() {
            self.shape_cache.remove(&d.id);
        }
    }
    pub(super) fn paint_shape_handles(
        &self,
        painter: &egui::Painter,
        view: Transform,
        surface: Surface,
        color: Color32,
    ) {
        if self.selection.surface != surface || self.selection.ids.len() != 1 {
            return;
        }
        let Some(ObjectSnapshot::Shape(shape)) = self
            .drawing
            .items()
            .iter()
            .find(|o| self.selection.ids.contains(&o.id()))
        else {
            return;
        };
        let shape = self.shape_edit_preview().unwrap_or(shape);
        let points: Vec<_> = shape
            .endpoints()
            .map(|p| p.to_vec())
            .or_else(|| shape.box_corners().map(|p| p.to_vec()))
            .unwrap_or_default();
        if !points.is_empty() {
            for point in points {
                let p = view.to_screen(
                    self.selection_preview(surface)
                        .unwrap_or_default()
                        .apply(point),
                );
                painter.circle_filled(Pos2::new(p[0] as f32, p[1] as f32), 4., color);
            }
        }
    }
}
