//! Magnetic handle/endpoint gestures inside the existing canvas controller.
use super::*;
use crate::selection;
use crate::ui::style;
use crate::{
    connector::{Connector, Endpoint, Port},
    object::{CanvasObject, Placement, Point},
    shape::Shape,
};

pub(super) struct ConnectionGesture {
    source: Option<Arc<Connector>>,
    pub(super) preview: Connector,
    moving: usize,
    start: Point,
    view: Transform,
    revision: u64,
    moved: bool,
    target: Option<Uuid>,
}
impl Canvas {
    fn port_display(shape: &Shape, port: Port, scale: f64) -> Option<Point> {
        let point = port.point(shape)?;
        let center = shape.geometry_bounds().center();
        let delta = [point[0] - center[0], point[1] - center[1]];
        let length = delta[0].hypot(delta[1]);
        Some(if length > 0. {
            std::array::from_fn(|i| {
                point[i] + delta[i] / length * f64::from(style::CONNECTOR_OFFSET) / scale
            })
        } else {
            point
        })
    }
    fn connection_handle(
        &self,
        point: Point,
        scale: f64,
    ) -> Option<(Option<Arc<Connector>>, Connector, usize)> {
        if self.selection.ids.len() == 1
            && let Some(ObjectSnapshot::Connector(c)) =
                self.drawing.snapshot(*self.selection.ids.iter().next()?)
        {
            for (index, e) in c.ends().into_iter().enumerate() {
                let p = e.position();
                if (p[0] - point[0]).hypot(p[1] - point[1]) * scale
                    <= f64::from(style::CONNECTOR_HIT)
                {
                    return Some((Some(c.clone()), (**c).clone(), index));
                }
            }
        }
        for id in &self.selection.ids {
            let Some(ObjectSnapshot::Shape(shape)) = self.drawing.snapshot(*id) else {
                continue;
            };
            for port in Port::ALL {
                let Some(p) = Self::port_display(shape, port, scale) else {
                    continue;
                };
                if (p[0] - point[0]).hypot(p[1] - point[1]) * scale
                    <= f64::from(style::CONNECTOR_HIT)
                {
                    return Some((
                        None,
                        Connector::new(
                            self.drawing.owner(),
                            shape.surface,
                            Endpoint::attached(shape, port)?,
                            Endpoint::Free {
                                position: port.point(shape)?,
                            },
                            self.shape_defaults.style.clone(),
                        ),
                        1,
                    ));
                }
            }
        }
        None
    }
    fn snap_connection(
        &mut self,
        surface: Surface,
        point: Point,
        scale: f64,
        exclude: Option<Uuid>,
    ) -> (Endpoint, Option<Uuid>) {
        let tolerance = f64::from(style::CONNECTOR_SNAP) / scale;
        self.object_index
            .sync_objects(self.drawing.revision(), self.drawing.items());
        let mut best: Option<(f64, usize, Endpoint, Uuid)> = None;
        self.object_index
            .query(surface, point, point, tolerance, |index| {
                let ObjectSnapshot::Shape(shape) = &self.drawing.items()[index] else {
                    return;
                };
                if shape.kind().segment() || Some(shape.id) == exclude {
                    return;
                }
                if !shape.hit_test(point, tolerance)
                    && !selection::inside(point, &shape.contours(0.025)[0])
                {
                    return;
                }
                for port in Port::ALL {
                    let p = port.point(shape).unwrap();
                    let distance = (point[0] - p[0]).hypot(point[1] - p[1]);
                    if best.as_ref().is_none_or(|(d, old, _, _)| {
                        distance < *d || (distance == *d && index > *old)
                    }) {
                        best = Some((
                            distance,
                            index,
                            Endpoint::attached(shape, port).unwrap(),
                            shape.id,
                        ));
                    }
                }
            });
        best.map(|(_, _, end, id)| (end, Some(id)))
            .unwrap_or((Endpoint::Free { position: point }, None))
    }
    fn connection_sample(&mut self, pointer: Point) {
        let Some(g) = self.connection_gesture.as_ref() else {
            return;
        };
        let surface = g.preview.surface;
        let scale = g.view.scale;
        let source = g.source.as_ref().map(|c| c.ends()[g.moving].position());
        let start = g.start;
        let mut point = source
            .map(|p| std::array::from_fn(|i| p[i] + pointer[i] - start[i]))
            .unwrap_or(pointer);
        if let Some(bounds) = self.drawing.bounds(surface) {
            for i in 0..2 {
                point[i] = point[i].clamp(0., bounds[i]);
            }
        }
        let exclude = g.preview.ends()[1 - g.moving].target();
        let moved = (pointer[0] - start[0]).hypot(pointer[1] - start[1]) * scale >= 3.;
        if !moved && !g.moved {
            return;
        }
        let (end, target) = self.snap_connection(surface, point, scale, exclude);
        let g = self.connection_gesture.as_mut().unwrap();
        if g.moving == 0 {
            g.preview.source = end;
        } else {
            g.preview.target = end;
        }
        g.target = target;
        g.moved = moved;
    }
    pub(super) fn connector_surface(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) -> bool {
        if !matches!(self.tool, Tool::Select | Tool::Lasso) {
            self.connection_gesture = None;
            return false;
        }
        if self.connection_gesture.as_ref().is_some_and(|g| {
            g.preview.surface == surface
                && (g.view != view
                    || g.revision != self.drawing.revision()
                    || !ui.is_enabled()
                    || !ui.input(|i| i.focused))
        }) {
            self.connection_gesture = None;
        }
        let mut consumed = self
            .connection_gesture
            .as_ref()
            .is_some_and(|g| g.preview.surface == surface);
        for event in ui.input(|i| i.events.clone()) {
            match event {
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } if self.connection_gesture.is_none()
                    && self.selection_gesture.is_none()
                    && self.selection.surface == surface
                    && !modifiers.alt
                    && !modifiers.shift
                    && !ui.input(|i| i.key_down(egui::Key::Space))
                    && bounds
                        .expand(style::CONNECTOR_OFFSET + style::CONNECTOR_HIT)
                        .intersect(ui.clip_rect())
                        .contains(pos)
                    && ui.is_enabled()
                    && ui.input(|i| i.focused)
                    && ui.ctx().layer_id_at(pos) == Some(ui.layer_id()) =>
                {
                    let point = view.to_document([pos.x as f64, pos.y as f64]);
                    if let Some((source, preview, moving)) =
                        self.connection_handle(point, view.scale)
                    {
                        self.connection_gesture = Some(ConnectionGesture {
                            source,
                            preview,
                            moving,
                            start: point,
                            view,
                            revision: self.drawing.revision(),
                            moved: false,
                            target: None,
                        });
                        consumed = true;
                    }
                }
                Event::PointerMoved(pos)
                    if self
                        .connection_gesture
                        .as_ref()
                        .is_some_and(|g| g.preview.surface == surface) =>
                {
                    self.connection_sample(view.to_document([pos.x as f64, pos.y as f64]))
                }
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                } if self
                    .connection_gesture
                    .as_ref()
                    .is_some_and(|g| g.preview.surface == surface) =>
                {
                    self.connection_sample(view.to_document([pos.x as f64, pos.y as f64]));
                    let g = self.connection_gesture.take().unwrap();
                    if g.moved {
                        let blocked=g.source.is_none() && g.target.is_none() && g.preview.source.target().is_some_and(|id|matches!(self.drawing.snapshot(id),Some(ObjectSnapshot::Shape(s)) if s.hit_test(g.preview.target.position(),0.) || selection::inside(g.preview.target.position(),&s.contours(0.025)[0])));
                        if !blocked {
                            let id = g.preview.id;
                            let changed = if g.source.is_some() {
                                self.drawing.replace_connector(g.preview)
                            } else {
                                self.drawing.add_connector(g.preview)
                            };
                            if changed {
                                self.selection.change(
                                    [id],
                                    selection::Membership::Replace,
                                    &self.drawing,
                                );
                            }
                        }
                    }
                    consumed = true;
                }
                Event::WindowFocused(false)
                | Event::Touch {
                    phase: egui::TouchPhase::Cancel,
                    ..
                } => self.connection_gesture = None,
                _ => (),
            }
        }
        consumed
    }
    pub(super) fn paint_connection_controls(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) {
        if self.selection.surface != surface || !matches!(self.tool, Tool::Select | Tool::Lasso) {
            return;
        }
        let painter = ui.painter().with_clip_rect(ui.clip_rect());
        let drawing_painter = painter.with_clip_rect(bounds.intersect(ui.clip_rect()));
        let palette = style::palette(ui);
        let screen = |p: Point| {
            let p = view.to_screen(p);
            egui::pos2(p[0] as f32, p[1] as f32)
        };
        if let Some(g) = &self.connection_gesture {
            if g.moved {
                shapes_ui::paint_shape(
                    ui,
                    &drawing_painter,
                    view,
                    &g.preview.as_shape(),
                    Placement::default(),
                    &mut self.shape_cache,
                );
            }
            if let Some(id) = g.target
                && let Some(ObjectSnapshot::Shape(shape)) = self.drawing.snapshot(id)
            {
                painter.add(egui::Shape::closed_line(
                    shape.contours(0.1)[0].iter().copied().map(screen).collect(),
                    Line::new(2., palette.accent),
                ));
                for port in Port::ALL {
                    painter.circle_stroke(
                        screen(port.point(shape).unwrap()),
                        6.,
                        Line::new(1.5, palette.accent),
                    );
                }
            }
            painter.text(
                bounds.intersect(ui.clip_rect()).left_top() + egui::vec2(12., 12.),
                egui::Align2::LEFT_TOP,
                if g.target.is_some() {
                    "Snap to shape · release to connect · Escape cancels"
                } else {
                    "Free endpoint · drag near a shape to snap · Escape cancels"
                },
                egui::FontId::proportional(12.),
                palette.text,
            );
        }
        let delta = self.selection_preview(surface).unwrap_or_default();
        for id in &self.selection.ids {
            match self.drawing.snapshot(*id) {
                Some(ObjectSnapshot::Shape(source)) if !source.kind().segment() => {
                    let mut shape = self
                        .shape_edit_preview()
                        .filter(|p| p.id == *id)
                        .unwrap_or(source)
                        .clone();
                    shape.placement = shape.placement.then(delta, [0.; 2]);
                    for port in Port::ALL {
                        let p = Self::port_display(&shape, port, view.scale).unwrap();
                        painter.circle(
                            screen(p),
                            style::CONNECTOR_RADIUS,
                            palette.surface,
                            Line::new(1.5, palette.accent),
                        );
                        let center = screen(p);
                        painter.line_segment(
                            [center - egui::vec2(2., 0.), center + egui::vec2(2., 0.)],
                            Line::new(1., palette.accent),
                        );
                        painter.line_segment(
                            [center - egui::vec2(0., 2.), center + egui::vec2(0., 2.)],
                            Line::new(1., palette.accent),
                        );
                    }
                }
                Some(ObjectSnapshot::Connector(c)) if self.selection.ids.len() == 1 => {
                    let points = self
                        .connection_gesture
                        .as_ref()
                        .filter(|g| g.preview.id == c.id)
                        .map(|g| g.preview.ends().map(|e| e.position()))
                        .unwrap_or_else(|| {
                            preview_shape(
                                c,
                                &self.drawing,
                                &self.selection.ids,
                                self.selection_preview(surface),
                                self.shape_edit_preview(),
                            )
                            .endpoints()
                            .unwrap()
                        });
                    for point in points {
                        painter.circle(
                            screen(point),
                            5.,
                            palette.surface,
                            Line::new(2., palette.accent),
                        );
                    }
                }
                _ => (),
            }
        }
        if self.connection_gesture.is_none()
            && self.selection_gesture.is_none()
            && let Some(pos) = ui.input(|i| i.pointer.hover_pos())
            && bounds.intersect(ui.clip_rect()).contains(pos)
            && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
            && self
                .connection_handle(view.to_document([pos.x as f64, pos.y as f64]), view.scale)
                .is_some()
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }
    pub(super) fn connect_selected_shapes(&mut self) {
        let shapes: Vec<_> = self
            .drawing
            .items()
            .iter()
            .filter_map(|o| match o {
                ObjectSnapshot::Shape(s)
                    if self.selection.ids.contains(&s.id) && !s.kind().segment() =>
                {
                    Some(s)
                }
                _ => None,
            })
            .collect();
        if shapes.len() != 2 {
            return;
        }
        let mut ports = (Port::Right, Port::Left);
        let mut best = f64::INFINITY;
        for source in Port::ALL {
            for target in Port::ALL {
                let a = source.point(shapes[0]).unwrap();
                let b = target.point(shapes[1]).unwrap();
                let d = (a[0] - b[0]).hypot(a[1] - b[1]);
                if d < best {
                    best = d;
                    ports = (source, target);
                }
            }
        }
        let c = Connector::new(
            self.drawing.owner(),
            shapes[0].surface,
            Endpoint::attached(shapes[0], ports.0).unwrap(),
            Endpoint::attached(shapes[1], ports.1).unwrap(),
            self.shape_defaults.style.clone(),
        );
        if shapes[0].surface == shapes[1].surface {
            let id = c.id;
            if self.drawing.add_connector(c) {
                self.selection
                    .change([id], selection::Membership::Replace, &self.drawing);
            }
        }
    }
}
/// Derive just two endpoints from current preview transforms. Unrelated target
/// geometry is never recalculated, and the canonical graph remains unchanged.
pub(super) fn preview_shape(
    c: &crate::connector::Connector,
    drawing: &crate::drawing::Drawing,
    ids: &HashSet<Uuid>,
    delta: Option<Placement>,
    edit: Option<&Shape>,
) -> Shape {
    let mut preview = if ids.contains(&c.id) {
        delta
            .map(|d| c.transformed(d, ids))
            .unwrap_or_else(|| c.clone())
    } else {
        c.clone()
    };
    for end in preview.ends_mut() {
        if let Endpoint::Attached {
            object_id, anchor, ..
        } = end
        {
            let point = if let Some(shape) = edit.filter(|s| s.id == *object_id) {
                anchor.point(shape)
            } else if ids.contains(object_id)
                && let Some(d) = delta
                && let Some(ObjectSnapshot::Shape(shape)) = drawing.snapshot(*object_id)
            {
                let mut shape = (**shape).clone();
                shape.placement = shape.placement.then(d, [0.; 2]);
                anchor.point(&shape)
            } else {
                None
            };
            if let Some(point) = point {
                end.set_position(point);
            }
        }
    }
    preview.as_shape()
}
