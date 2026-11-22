//! Canvas placement, configuration and shared preview for coordinate systems.
use super::*;
use crate::{
    coordinate::{
        Arrow, Config, Coordinate, MAX_MARKERS, MAX_PLOTS, Marker, Pattern, Plot, Preset,
    },
    coordinate_geometry::{self as geometry, Color, Geometry},
    object::{Placement, Point},
};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(super) struct CoordinateCache {
    key: Option<(Arc<Config>, Point, usize)>,
    geometry: Geometry,
    marks: Vec<Marks>,
    samples: geometry::SamplingCache,
    math_labels: HashMap<(String, [u8; 3]), CachedMathLabel>,
}
struct CachedMathLabel {
    rendered: Result<crate::formula_render::RenderedFormula, String>,
    texture: crate::formula_render::FormulaTexture,
}
enum Marks {
    Solid,
    Dashes(Vec<Vec<Point>>),
    Dots(Vec<Point>),
}

impl Marks {
    fn new(line: &geometry::Line) -> Self {
        match line.pattern {
            Pattern::Solid => Self::Solid,
            Pattern::Dashed => {
                let path = crate::stroke_pattern::MeasuredPath::new(&line.points, false);
                let step = (path.length() / 2560.).max(1.);
                Self::Dashes(path.dashes(6. * step, 4. * step))
            }
            Pattern::Dotted => {
                let path = crate::stroke_pattern::MeasuredPath::new(&line.points, false);
                Self::Dots(path.dots((path.length() / 256.).max(4.), line.width))
            }
        }
    }
}

pub(super) struct CoordinateDraft {
    id: Uuid,
    value: Config,
    size: Point,
    tab: CoordinateTab,
    tikz_source: String,
    tikz_error: Option<String>,
    validation: Option<(Arc<Config>, Result<(), String>)>,
    preview: CoordinateCache,
    error: Option<String>,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum CoordinateTab {
    #[default]
    General,
    Functions,
    Tikz,
}

fn color(value: [u8; 3], alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(value[0], value[1], value[2], alpha)
}

fn math_label(source: &str) -> String {
    source.trim().trim_matches('$').to_owned()
}

// Bucket the projected object extent in physical pixels so dragging and
// small zoom changes can reuse geometry. Both preview and canvas use this.
fn sampling_extent(size: Point, scale: f32, dpi: f32) -> usize {
    (size[0].max(size[1]) * f64::from(scale) * f64::from(dpi))
        .clamp(64., 16_384.)
        .log2()
        .ceil()
        .exp2() as usize
}

pub(super) fn paint_coordinate(
    cache: &mut CoordinateCache,
    painter: &egui::Painter,
    object: &Coordinate,
    view: Transform,
    delta: Placement,
    foreground: Color32,
) {
    let placement = object.placement.then(delta, [0.; 2]);
    let screen = |point: Point| {
        let [x, y] = view.to_screen(placement.apply(point));
        egui::pos2(x as f32, y as f32)
    };
    let [w, h] = object.size;
    let corners = [[0., 0.], [w, 0.], [w, h], [0., h]].map(screen);
    if !painter.clip_rect().intersects(Rect::from_points(&corners)) {
        return;
    }
    let scale = (view.scale * placement.scale) as f32;
    let quality = sampling_extent(object.size, scale, painter.ctx().pixels_per_point());
    let key = (object.config.clone(), object.size, quality);
    if cache.key.as_ref() != Some(&key) {
        if cache
            .key
            .as_ref()
            .is_none_or(|(config, size, _)| *config != object.config || *size != object.size)
        {
            cache.math_labels.clear();
        }
        cache.geometry =
            geometry::generate(&object.config, object.size, quality, &mut cache.samples);
        cache.marks = cache.geometry.lines.iter().map(Marks::new).collect();
        cache.key = Some(key);
    }
    let cfg = &object.config;
    let axis = cfg.axis_color.map_or(foreground, |rgb| color(rgb, 255));
    let tint = |value| match value {
        Color::Axis => axis,
        Color::Tick => cfg.tick_color.map_or(axis, |rgb| color(rgb, 255)),
        Color::Custom(rgb, alpha) => color(rgb, alpha),
    };
    if let Some(rgb) = cfg.background {
        painter.add(egui::Shape::convex_polygon(
            corners.to_vec(),
            color(rgb, 255),
            egui::Stroke::NONE,
        ));
    }
    for (line, marks) in cache.geometry.lines.iter().zip(&cache.marks) {
        let width = (line.width as f32 * scale).max(0.15);
        let ink = tint(line.color);
        let stroke = egui::Stroke::new(width, ink);
        match marks {
            Marks::Solid => {
                painter.add(egui::Shape::line(
                    line.points.iter().copied().map(screen).collect(),
                    stroke,
                ));
            }
            Marks::Dashes(dashes) => {
                for dash in dashes {
                    painter.add(egui::Shape::line(
                        dash.iter().copied().map(screen).collect(),
                        stroke,
                    ));
                }
            }
            Marks::Dots(dots) => {
                for &dot in dots {
                    painter.circle_filled(screen(dot), width / 2., ink);
                }
            }
        }
    }
    for arrow in &cache.geometry.arrows {
        let mut polygon = arrow.map(screen).to_vec();
        if placement.reflected[0] != placement.reflected[1] {
            polygon.reverse();
        }
        painter.add(egui::Shape::convex_polygon(
            polygon,
            axis,
            egui::Stroke::NONE,
        ));
    }
    for dot in &cache.geometry.dots {
        let point = screen(dot.at);
        let radius = 3.7 * scale;
        if dot.closed {
            painter.circle_filled(point, radius, tint(dot.color));
        } else {
            painter.circle_stroke(
                point,
                radius,
                egui::Stroke::new((1.3 * scale).max(0.15), tint(dot.color)),
            );
        }
    }
    if scale * 10. < 3. {
        return;
    }
    for label in &cache.geometry.labels {
        if label.math {
            let rgba = axis.to_array();
            let key = (math_label(&label.text), [rgba[0], rgba[1], rgba[2]]);
            let entry = cache
                .math_labels
                .entry(key.clone())
                .or_insert_with(|| CachedMathLabel {
                    rendered: crate::formula_render::render(
                        &key.0,
                        [key.1[0], key.1[1], key.1[2], 255],
                    ),
                    texture: Default::default(),
                });
            if let Ok(formula) = &entry.rendered {
                let size = label.size / crate::formula::FORMULA_FONT_SIZE as f32 * scale;
                let center = screen(label.at);
                let rotation = egui::emath::Rot2::from_angle(placement.rotation as f32);
                let mut map = |p: egui::Pos2| {
                    center + rotation * (p - egui::pos2(formula.width, formula.height) / 2.) * size
                };
                entry.texture.paint(
                    painter,
                    formula,
                    size * painter.ctx().pixels_per_point(),
                    true,
                    &mut map,
                );
            }
        } else {
            // Coordinate labels are all mathematical today; keep the branch
            // for future non-math annotations without changing their font.
            let font = egui::FontId::proportional((label.size * scale).min(256.));
            painter.text(
                screen(label.at),
                egui::Align2::CENTER_CENTER,
                &label.text,
                font,
                axis,
            );
        }
    }
}

impl Canvas {
    pub fn begin_coordinate_placement(&mut self, preset: Preset) {
        self.coordinate_preset = preset;
        self.select(Tool::Coordinate);
        self.notice = Some(format!(
            "Click the canvas to place {}. Escape cancels.",
            preset.label()
        ));
    }

    pub(super) fn coordinate_surface(
        &mut self,
        ui: &egui::Ui,
        bounds: Rect,
        surface: Surface,
        view: Transform,
    ) -> bool {
        if self.tool != Tool::Coordinate {
            return false;
        }
        if !self.processed.contains(&surface) {
            self.processed.push(surface);
            let click = ui.input(|i| {
                i.events.iter().find_map(|event| match event {
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    } if modifiers.is_none()
                        && !i.key_down(egui::Key::Space)
                        && bounds.contains(*pos)
                        && ui.clip_rect().contains(*pos) =>
                    {
                        Some(*pos)
                    }
                    _ => None,
                })
            });
            if ui.is_enabled()
                && let Some(pos) = click
                && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
                && !egui::Popup::is_any_open(ui.ctx())
            {
                self.insert_coordinate(surface, view.to_document([pos.x as f64, pos.y as f64]));
            }
        }
        true
    }

    fn insert_coordinate(&mut self, surface: Surface, point: Point) {
        let mut object =
            Coordinate::new(self.drawing.owner(), surface, point, self.coordinate_preset);
        if let Some(page) = self.drawing.bounds(surface) {
            for (i, limit) in page.into_iter().enumerate() {
                object.placement.position[i] =
                    object.placement.position[i].clamp(0., (limit - object.size[i]).max(0.));
            }
        }
        let id = object.id;
        if self.drawing.insert_snapshots(vec![object.into()]) {
            self.select(self.selection_mode);
            self.selection.scope(surface);
            self.selection
                .change([id], crate::selection::Membership::Replace, &self.drawing);
            self.notice = Some("Coordinate system inserted. Double-click to configure.".into());
        } else {
            self.notice = Some("Could not place a coordinate system on this canvas.".into());
        }
    }

    pub(super) fn selected_coordinate(&self) -> Option<&Coordinate> {
        if self.selection.ids.len() != 1 {
            return None;
        }
        match self.drawing.snapshot(*self.selection.ids.iter().next()?)? {
            ObjectSnapshot::Coordinate(object) => Some(object),
            _ => None,
        }
    }

    pub(super) fn open_coordinate(&mut self, id: Uuid) {
        let Some(ObjectSnapshot::Coordinate(object)) = self.drawing.snapshot(id) else {
            return;
        };
        let tikz_source = if object.tikz_source.is_empty() {
            crate::coordinate_tikz::generate(&object.config)
        } else {
            object.tikz_source.clone()
        };
        let tikz_error = crate::coordinate_tikz::parse(&tikz_source, &object.config).err();
        self.coordinate_draft = Some(CoordinateDraft {
            id,
            value: (*object.config).clone(),
            size: object.size,
            tab: CoordinateTab::General,
            tikz_source,
            tikz_error,
            validation: None,
            preview: Default::default(),
            error: None,
        });
    }

    pub(super) fn coordinate_dialog(&mut self, ctx: &egui::Context) -> bool {
        let Some(mut draft) = self.coordinate_draft.take() else {
            return false;
        };
        let Some(ObjectSnapshot::Coordinate(object)) = self.drawing.snapshot(draft.id).cloned()
        else {
            return false;
        };
        let locked = self.drawing.is_locked(draft.id);
        let mut cancel = false;
        let mut apply = false;
        let result = crate::ui::dialog::configuration(
            ctx,
            egui::Id::unique(("coordinate-system", draft.id)),
            "Coordinate System",
            [1020., 390.],
            |ui| {
                let resize_id = draft.id;
                let mut body = |ui: &mut egui::Ui| {
                    let frame_before = draft.value.clone();
                    ui.horizontal(|ui| {
                        ui.heading("Coordinate System");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.ctx().viewport_id() == egui::ViewportId::ROOT {
                                cancel |= ui.button("×").on_hover_text("Close").clicked();
                            }
                            apply |= ui
                                .add_enabled(
                                    !locked && draft.tikz_error.is_none(),
                                    egui::Button::new("Apply"),
                                )
                                .clicked();
                            cancel |= ui.button("Cancel").clicked();
                        });
                    });
                    ui.separator();
                    let body_height = (ui.available_height() - 65.).max(120.);
                    ui.horizontal_top(|ui| {
                        let left_width = ui.available_width() * 0.50;
                        ui.allocate_ui_with_layout(
                            egui::vec2(left_width, body_height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.horizontal(|ui| {
                                    ui.selectable_value(
                                        &mut draft.tab,
                                        CoordinateTab::General,
                                        "General",
                                    );
                                    ui.selectable_value(
                                        &mut draft.tab,
                                        CoordinateTab::Functions,
                                        "Functions",
                                    );
                                    ui.selectable_value(
                                        &mut draft.tab,
                                        CoordinateTab::Tikz,
                                        "TikZ Editor",
                                    );
                                });
                                ui.separator();
                                egui::ScrollArea::vertical()
                                    .id_salt(("coordinate-tab", draft.id))
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| match draft.tab {
                                        CoordinateTab::General => {
                                            ui.add_enabled_ui(
                                                !locked && draft.tikz_error.is_none(),
                                                |ui| {
                                                    general_fields(
                                                        ui,
                                                        &mut draft.value,
                                                        &mut draft.size,
                                                    )
                                                },
                                            );
                                        }
                                        CoordinateTab::Functions => {
                                            ui.add_enabled_ui(
                                                !locked && draft.tikz_error.is_none(),
                                                |ui| function_fields(ui, &mut draft.value),
                                            );
                                        }
                                        CoordinateTab::Tikz => {
                                            ui.label(
                                                "Supported TikZ subset · edits update the preview",
                                            );
                                            let response = ui.add(
                                                egui::TextEdit::multiline(&mut draft.tikz_source)
                                                    .code_editor()
                                                    .font(egui::TextStyle::Monospace)
                                                    .desired_width(f32::INFINITY)
                                                    .desired_rows(8)
                                                    .lock_focus(true),
                                            );
                                            if response.changed() {
                                                match crate::coordinate_tikz::parse(
                                                    &draft.tikz_source,
                                                    &frame_before,
                                                ) {
                                                    Ok(config) => {
                                                        draft.value = config;
                                                        draft.tikz_error = None;
                                                        draft.error = None;
                                                    }
                                                    Err(error) => draft.tikz_error = Some(error),
                                                }
                                            }
                                        }
                                    });
                            },
                        );
                        ui.separator();
                        let right_width = ui.available_width();
                        ui.allocate_ui_with_layout(
                            egui::vec2(right_width, body_height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.label("Live Preview");
                                let (rect, painter) =
                                    ui.allocate_painter(ui.available_size(), egui::Sense::hover());
                                painter.rect_filled(
                                    rect.rect,
                                    4.,
                                    crate::ui::style::palette(ui).board,
                                );
                                draw_preview(
                                    &mut draft,
                                    &object,
                                    &painter,
                                    rect.rect.shrink(8.),
                                    crate::ui::style::palette(ui).text,
                                );
                            },
                        );
                    });
                    if draft.tab != CoordinateTab::Tikz && draft.value != frame_before {
                        draft.tikz_source = crate::coordinate_tikz::generate_preserving(
                            &draft.value,
                            &draft.tikz_source,
                        );
                        draft.tikz_error = None;
                    }
                    if let Some(error) =
                        draft
                            .tikz_error
                            .as_ref()
                            .or(draft.error.as_ref())
                            .or_else(|| {
                                draft
                                    .validation
                                    .as_ref()
                                    .and_then(|(_, v)| v.as_ref().err())
                            })
                    {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    if locked {
                        ui.weak("Unlock this coordinate system to apply changes.");
                    }
                    cancel |=
                        ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
                };
                if ui.ctx().viewport_id() == egui::ViewportId::ROOT {
                    let max_size = (ui.ctx().content_rect().size() - egui::vec2(40., 40.))
                        .max(egui::vec2(320., 320.));
                    let min_size = egui::vec2(760., 320.).min(max_size);
                    egui::Resize::default()
                        .id_salt(("coordinate-dialog-resize", resize_id))
                        // A practical compact default: the selected left tab scrolls,
                        // while the preview keeps enough height to remain legible.
                        .default_size(egui::vec2(1000., 360.).min(max_size))
                        .min_size(min_size)
                        .max_size(max_size)
                        .show(ui, &mut body);
                } else {
                    body(ui);
                }
            },
        );
        if cancel || result {
            return true;
        }
        if apply {
            match draft.value.validate() {
                Ok(()) => {
                    let mut next = (*object).clone();
                    next.config = Arc::new(draft.value.clone());
                    next.size = draft.size;
                    next.tikz_source = draft.tikz_source.clone();
                    if next == *object || self.drawing.replace_coordinate(next) {
                        self.selection.refresh(&self.drawing);
                        return true;
                    }
                    draft.error = Some(
                        "The frame must remain within the page and supported size limits.".into(),
                    );
                }
                Err(error) => draft.error = Some(error),
            }
        }
        self.coordinate_draft = Some(draft);
        true
    }
}

fn draw_preview(
    draft: &mut CoordinateDraft,
    object: &Coordinate,
    painter: &egui::Painter,
    rect: Rect,
    ink: Color32,
) {
    if draft
        .validation
        .as_ref()
        .is_none_or(|(config, _)| **config != draft.value)
    {
        draft.validation = Some((Arc::new(draft.value.clone()), draft.value.validate()));
        draft.error = None;
    }
    let (config, valid) = draft.validation.as_ref().unwrap();
    let mut preview = object.clone();
    preview.placement = Placement::default();
    if valid.is_ok() {
        preview.config = config.clone();
    }
    preview.size = draft.size;
    let scale = (f64::from(rect.width() - 8.) / preview.size[0])
        .min(f64::from(rect.height() - 8.) / preview.size[1]);
    let origin = std::array::from_fn(|i| {
        let center = [rect.center().x, rect.center().y][i];
        f64::from(center) - preview.size[i] * scale / 2.
    });
    paint_coordinate(
        &mut draft.preview,
        painter,
        &preview,
        Transform {
            origin,
            center: [0.; 2],
            scale,
        },
        Placement::default(),
        ink,
    );
}

fn number_field(
    ui: &mut egui::Ui,
    name: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    speed: f64,
) {
    ui.horizontal(|ui| {
        ui.label(name);
        ui.add(egui::DragValue::new(value).range(range).speed(speed));
    });
}
fn range_fields(ui: &mut egui::Ui, name: &str, range: &mut Point, positive: bool) {
    ui.horizontal(|ui| {
        ui.label(name);
        if positive {
            ui.label("0");
        } else {
            ui.add(
                egui::DragValue::new(&mut range[0])
                    .range(-1e6..=1e6)
                    .speed(0.1),
            );
        }
        ui.label("to");
        ui.add(
            egui::DragValue::new(&mut range[1])
                .range(-1e6..=1e6)
                .speed(0.1),
        );
    });
}
fn optional_color(ui: &mut egui::Ui, label: &str, value: &mut Option<[u8; 3]>, initial: [u8; 3]) {
    let mut enabled = value.is_some();
    ui.horizontal(|ui| {
        if ui.checkbox(&mut enabled, label).changed() {
            *value = enabled.then_some(initial);
        }
        if let Some(rgb) = value {
            crate::ui::shape_controls::color_control(ui, rgb, 255);
        }
    });
}
fn config_fields(ui: &mut egui::Ui, config: &mut Config, size: &mut Point, functions_only: bool) {
    let mut dimension = config.preset.dimension();
    if !functions_only {
        ui.label("Preset");
        let mut preset = config.preset;
        egui::ComboBox::from_id_salt("coordinate-preset")
            .selected_text(preset.label())
            .show_ui(ui, |ui| {
                for option in Preset::ALL {
                    ui.selectable_value(&mut preset, option, option.label());
                }
            });
        if preset != config.preset {
            // Preserve the chosen frame. A new dimension reuses compatible authored values.
            config.set_preset(preset);
        }
        dimension = config.preset.dimension();
        egui::CollapsingHeader::new("Axes")
            .default_open(true)
            .show(ui, |ui| {
                for (i, axis) in config.axes.iter_mut().enumerate().take(dimension) {
                    ui.push_id(i, |ui| {
                        let name = ["X", "Y", "Z"][i];
                        range_fields(
                            ui,
                            &format!("{name} range"),
                            &mut axis.range,
                            !matches!(preset, Preset::Signed | Preset::FourQuadrants),
                        );
                        ui.horizontal_wrapped(|ui| {
                            ui.checkbox(&mut axis.visible, format!("Show {name} axis"));
                            ui.checkbox(&mut axis.show_label, "Label");
                            ui.add(
                                egui::TextEdit::singleline(&mut axis.label)
                                    .desired_width(72.)
                                    .char_limit(32),
                            );
                        });
                    });
                }
                ui.horizontal(|ui| {
                    ui.checkbox(&mut config.show_zero, "Show 0 Label");
                    config.origin = config.show_zero;
                    ui.add_enabled(
                        config.show_zero,
                        egui::TextEdit::singleline(&mut config.origin_label)
                            .desired_width(64.)
                            .char_limit(32),
                    );
                });
                ui.checkbox(&mut config.numbers, "Show Numeric Labels");
                if dimension == 2 {
                    ui.checkbox(&mut config.equal_units, "Equal X and Y units");
                }
                optional_color(
                    ui,
                    "Custom axis color",
                    &mut config.axis_color,
                    [35, 43, 53],
                );
                number_field(ui, "Axis thickness", &mut config.axis_width, 0.15..=8., 0.1);
                egui::ComboBox::from_id_salt("coordinate-arrow")
                    .selected_text(format!("{:?} arrowheads", config.arrow))
                    .show_ui(ui, |ui| {
                        for (option, name) in [
                            (Arrow::Scientific, "Scientific"),
                            (Arrow::Open, "Open"),
                            (Arrow::None, "No arrowheads"),
                        ] {
                            ui.selectable_value(&mut config.arrow, option, name);
                        }
                    });
            });
        ui.collapsing("Grid and ticks", |ui| {
            if dimension >= 2 {
                ui.checkbox(
                    &mut config.grid,
                    if dimension == 3 {
                        "Show XY reference grid"
                    } else {
                        "Show grid"
                    },
                );
                if config.grid {
                    number_field(
                        ui,
                        "Major grid spacing",
                        &mut config.grid_step,
                        1e-6..=1e6,
                        0.1,
                    );
                    ui.horizontal(|ui| {
                        ui.label("Grid color");
                        crate::ui::shape_controls::color_control(ui, &mut config.grid_color, 255);
                        ui.label("Opacity");
                        ui.add(egui::DragValue::new(&mut config.grid_opacity).range(0..=255));
                    });
                    number_field(ui, "Grid thickness", &mut config.grid_width, 0.15..=8., 0.1);
                    ui.horizontal(|ui| {
                        ui.label("Minor divisions per major interval");
                        ui.add(egui::DragValue::new(&mut config.grid_subdivisions).range(1..=8));
                    });
                }
            }
            ui.checkbox(&mut config.ticks, "Show tick marks");
            if config.ticks || config.numbers {
                number_field(ui, "Tick spacing", &mut config.tick_step, 1e-6..=1e6, 0.1);
            }
            if config.ticks {
                number_field(ui, "Tick length", &mut config.tick_length, 1. ..=24., 0.2);
                number_field(ui, "Tick thickness", &mut config.tick_width, 0.15..=8., 0.1);
                optional_color(
                    ui,
                    "Custom tick color",
                    &mut config.tick_color,
                    [35, 43, 53],
                );
            }
        });
        if dimension == 3 {
            ui.collapsing("Orthographic view", |ui| {
                number_field(ui, "Azimuth (°)", &mut config.azimuth, -180. ..=180., 1.);
                number_field(ui, "Elevation (°)", &mut config.elevation, 5. ..=85., 1.);
            });
        }
    }
    if functions_only {
        match dimension {
            1 => {
                ui.collapsing("Points and intervals", |ui| {
                    markers(ui, &mut config.markers)
                });
            }
            2 => {
                egui::CollapsingHeader::new("Functions")
                    .default_open(true)
                    .show(ui, |ui| plots(ui, &mut config.curves, false));
            }
            _ => {
                egui::CollapsingHeader::new("Surfaces")
                    .default_open(true)
                    .show(ui, |ui| plots(ui, &mut config.surfaces, true));
            }
        }
    }
    if !functions_only {
        ui.collapsing("Frame", |ui| {
            number_field(ui, "Width", &mut size[0], 48. ..=1e6, 1.);
            number_field(ui, "Height", &mut size[1], 48. ..=1e6, 1.);
            optional_color(ui, "Background fill", &mut config.background, [255; 3]);
        });
    }
}

fn general_fields(ui: &mut egui::Ui, config: &mut Config, size: &mut Point) {
    config_fields(ui, config, size, false);
}
fn function_fields(ui: &mut egui::Ui, config: &mut Config) {
    config_fields(ui, config, &mut [0., 0.], true);
}
fn plots(ui: &mut egui::Ui, plots: &mut Vec<Plot>, surface: bool) {
    let mut remove = None;
    let mut swap = None;
    let count = plots.len();
    for (index, plot) in plots.iter_mut().enumerate() {
        ui.push_id(index, |ui| {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut plot.visible, format!("Plot {}", index + 1));
                crate::ui::shape_controls::color_control(ui, &mut plot.color, 255);
                if ui
                    .add_enabled(index > 0, egui::Button::new("↑"))
                    .on_hover_text("Move up")
                    .clicked()
                {
                    swap = Some((index, index - 1));
                }
                if ui
                    .add_enabled(index + 1 < count, egui::Button::new("↓"))
                    .on_hover_text("Move down")
                    .clicked()
                {
                    swap = Some((index, index + 1));
                }
                if ui.button("Remove").clicked() {
                    remove = Some(index);
                }
            });
            ui.add(
                egui::TextEdit::singleline(&mut plot.expression)
                    .hint_text(if surface { "z = f(x, y)" } else { "y = f(x)" })
                    .char_limit(256)
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                ui.label("Thickness");
                ui.add(
                    egui::DragValue::new(&mut plot.width)
                        .range(0.15..=8.)
                        .speed(0.1),
                );
                egui::ComboBox::from_id_salt("pattern")
                    .selected_text(format!("{:?}", plot.pattern))
                    .show_ui(ui, |ui| {
                        for p in [Pattern::Solid, Pattern::Dashed, Pattern::Dotted] {
                            ui.selectable_value(&mut plot.pattern, p, format!("{p:?}"));
                        }
                    });
            });
            plot_domain(ui, "X", &mut plot.domain);
            if surface {
                plot_domain(ui, "Y", &mut plot.y_domain);
            }
        });
    }
    if let Some(i) = remove {
        plots.remove(i);
    } else if let Some((a, b)) = swap {
        plots.swap(a, b);
    }
    if ui
        .add_enabled(
            plots.len() < MAX_PLOTS,
            egui::Button::new(if surface {
                "Add Surface"
            } else {
                "Add Function"
            }),
        )
        .clicked()
    {
        plots.push(Plot::new(plots.len(), surface));
    }
}
fn plot_domain(ui: &mut egui::Ui, label: &str, domain: &mut Option<Point>) {
    let mut custom = domain.is_some();
    if ui
        .checkbox(&mut custom, format!("Custom {label} domain"))
        .changed()
    {
        *domain = custom.then_some([0., 5.]);
    }
    if let Some(range) = domain {
        range_fields(ui, &format!("{label} domain"), range, false);
    }
}
fn markers(ui: &mut egui::Ui, markers: &mut Vec<Marker>) {
    let mut remove = None;
    for (index, marker) in markers.iter_mut().enumerate() {
        ui.push_id(index, |ui| {
            ui.separator();
            number_field(ui, "Start / point", &mut marker.start, -1e6..=1e6, 0.1);
            if let Some(end) = &mut marker.end {
                number_field(ui, "End", end, -1e6..=1e6, 0.1);
            }
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut marker.closed[0], "Closed start");
                if marker.end.is_some() {
                    ui.checkbox(&mut marker.closed[1], "Closed end");
                }
                crate::ui::shape_controls::color_control(ui, &mut marker.color, 255);
                if ui.button("Remove").clicked() {
                    remove = Some(index);
                }
            });
        });
    }
    if let Some(i) = remove {
        markers.remove(i);
    }
    ui.horizontal(|ui| {
        for (name, end) in [("Add Point", None), ("Add Interval", Some(3.))] {
            if ui
                .add_enabled(markers.len() < MAX_MARKERS, egui::Button::new(name))
                .clicked()
            {
                markers.push(Marker {
                    start: 1.,
                    end,
                    closed: [true, true],
                    color: [46, 112, 210],
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        canvas::selection_ui::tests::{click, setup, t},
        document::DocumentKind,
        object_clipboard::ObjectBundle,
        ui::settings::tests::{pointer, render},
    };

    #[test]
    fn placement_and_selection_preserve_semantic_plots_through_transform_and_clipboard() {
        let ctx = egui::Context::default();
        let (mut board, surface) = setup(DocumentKind::Whiteboard);
        board.begin_coordinate_placement(Preset::FourQuadrants);
        click(
            &ctx,
            &mut board,
            surface,
            t(),
            [350., 250.],
            egui::Modifiers::NONE,
        );
        let object = board.selected_coordinate().unwrap().clone();
        let id = object.id;
        let mut configured = object.clone();
        Arc::make_mut(&mut configured.config).curves = vec![
            Plot::new(0, false),
            Plot {
                expression: "sin(x)".into(),
                domain: Some([-2., 3.]),
                pattern: Pattern::Dashed,
                ..Plot::new(1, false)
            },
        ];
        configured.tikz_source = crate::coordinate_tikz::generate(&configured.config);
        assert!(board.drawing.replace_coordinate(configured.clone()));
        let snapshot = board.drawing.snapshot(id).unwrap().clone();
        let moved = snapshot.transformed(Placement {
            position: [11., 15.],
            scale: 1.2,
            rotation: 0.35,
            ..Default::default()
        });
        let ObjectSnapshot::Coordinate(moved) = moved else {
            panic!("missing coordinate");
        };
        assert!(Arc::ptr_eq(&moved.config, &configured.config));
        assert_ne!(moved.corners(), configured.corners());
        assert!(board.drawing.replace_coordinate((*moved).clone()));
        board.undo();
        assert_eq!(board.drawing.snapshot(id), Some(&snapshot));
        board.redo();
        let bundle = ObjectBundle::capture_objects(std::iter::once(
            board.drawing.snapshot(id).unwrap().clone(),
        ))
        .unwrap();
        let (mut page, page_surface) = setup(DocumentKind::Page);
        let copy = ObjectBundle::decode(&bundle.encode().unwrap())
            .unwrap()
            .paste(&mut page.drawing, page_surface, [295., 395.]);
        let [new_id] = copy.as_slice() else {
            panic!("one pasted coordinate expected");
        };
        let Some(ObjectSnapshot::Coordinate(copy)) = page.drawing.snapshot(*new_id) else {
            panic!("missing pasted coordinate");
        };
        assert_ne!(*new_id, id);
        assert_eq!(copy.config, configured.config);
        assert_eq!(copy.tikz_source, configured.tikz_source);
        assert_eq!(copy.placement.rotation, moved.placement.rotation);
        assert_eq!(copy.surface, page_surface);

        let second = Coordinate::new(board.drawing.owner(), surface, [550., 350.], Preset::Signed);
        let other_id = second.id;
        assert!(board.drawing.insert_snapshots(vec![second.into()]));
        let group = board
            .drawing
            .group_selection(&HashSet::from([id, other_id]))
            .unwrap();
        assert!(board.drawing.lock_objects(&HashSet::from([group]), true));
        assert!(board.drawing.is_locked(id));
        let mut changed = (*moved).clone();
        Arc::make_mut(&mut changed.config).grid = true;
        assert!(!board.drawing.replace_coordinate(changed));
    }

    #[test]
    fn unified_dialog_drafts_are_transactional_and_preview_invalid_input_safely() {
        let ctx = egui::Context::default();
        let (mut board, surface) = setup(DocumentKind::Whiteboard);
        board.coordinate_preset = Preset::FourQuadrants;
        board.insert_coordinate(surface, [320., 250.]);
        let id = board.selected_coordinate().unwrap().id;
        let before = board.drawing.snapshot(id).unwrap().clone();
        board.open_coordinate(id);
        {
            let draft = board.coordinate_draft.as_mut().unwrap();
            draft.value.set_preset(Preset::Octant);
            draft.value.surfaces.push(Plot::new(0, true));
            draft.value.surfaces[0].expression = "sqrt(".into();
        }
        let mut frame = |events| {
            render(&ctx, events, |ui| {
                board.coordinate_dialog(ui.ctx());
            })
        };
        frame(vec![]);
        let visible = frame(vec![]);
        assert!(visible.contains_key("General"));
        assert!(visible.contains_key("Functions"));
        assert!(visible.contains_key("TikZ Editor"));
        assert!(visible.contains_key("Live Preview"));
        let labels = frame(vec![]);
        let cancel = *labels
            .get("Cancel")
            .unwrap_or_else(|| panic!("dialog buttons missing: {:?}", labels.keys()));
        frame(pointer(cancel, true));
        frame(pointer(cancel, false));
        assert!(board.coordinate_draft.is_none());
        assert_eq!(board.drawing.snapshot(id), Some(&before));

        let baseline = board.selected_coordinate().unwrap().config.clone();
        board.open_coordinate(id);
        {
            let draft = board.coordinate_draft.as_mut().unwrap();
            draft.tab = CoordinateTab::Tikz;
            let range = draft.value.axes[0].range;
            let old = format!("({},0) -- ({},0)", range[0], range[1]);
            draft.tikz_source =
                crate::coordinate_tikz::generate(&draft.value).replace(&old, "(-9,0) -- (9,0)");
            draft.value = crate::coordinate_tikz::parse(&draft.tikz_source, &draft.value).unwrap();
        }
        let mut frame = |events| {
            render(&ctx, events, |ui| {
                board.coordinate_dialog(ui.ctx());
            })
        };
        frame(vec![]);
        let apply = frame(vec![])["Apply"];
        frame(pointer(apply, true));
        frame(pointer(apply, false));
        assert!(board.coordinate_draft.is_none());
        let committed = board.selected_coordinate().unwrap();
        assert_eq!(committed.id, id);
        assert_eq!(committed.config.preset, baseline.preset);
        assert_eq!(committed.config.axes[0].range, [-9., 9.]);
        assert!(committed.tikz_source.contains("(-9,0) -- (9,0)"));
        board.undo();
        assert_eq!(board.drawing.snapshot(id), Some(&before));
    }
}
