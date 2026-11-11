//! Semantic vector shapes in local document points. No UI, storage or PDF APIs.
use crate::{
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectId, ObjectType, Placement, Point},
    text::TextContent,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rectangle,
    RoundedRectangle,
    Circle,
    Ellipse,
    Triangle,
    Diamond,
    Line,
    Arrow,
}
impl ShapeKind {
    pub const ALL: [Self; 8] = [
        Self::Rectangle,
        Self::RoundedRectangle,
        Self::Circle,
        Self::Ellipse,
        Self::Triangle,
        Self::Diamond,
        Self::Line,
        Self::Arrow,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::RoundedRectangle => "Rounded rectangle",
            Self::Circle => "Circle",
            Self::Ellipse => "Ellipse",
            Self::Triangle => "Triangle",
            Self::Diamond => "Diamond",
            Self::Line => "Straight line",
            Self::Arrow => "Arrow",
        }
    }
    pub fn segment(self) -> bool {
        matches!(self, Self::Line | Self::Arrow)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderStyle {
    Solid,
    Dashed,
    Dotted,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeStyle {
    #[serde(default)]
    pub start_head: bool,
    #[serde(default = "border_enabled")]
    pub border_enabled: bool,
    pub fill: Option<[u8; 3]>,
    pub fill_opacity: u8,
    pub border: [u8; 4],
    pub border_width: f64,
    pub border_style: BorderStyle,
    #[serde(default)]
    pub pattern: crate::stroke_pattern::PatternSettings,
}
fn border_enabled() -> bool {
    true
}
impl ShapeStyle {
    pub fn envelope_width(&self) -> f64 {
        if self.border_style == BorderStyle::Dotted {
            self.border_width.max(self.pattern.dot_size)
        } else {
            self.border_width
        }
    }
    pub fn valid(&self) -> bool {
        self.border_width.is_finite()
            && (0.1..=24.).contains(&self.border_width)
            && self.pattern.valid()
    }
}
impl Default for ShapeStyle {
    fn default() -> Self {
        Self {
            start_head: false,
            border_enabled: true,
            fill: Some([90, 150, 220]),
            fill_opacity: 35,
            border: [37, 99, 200, 255],
            border_width: 2.,
            border_style: BorderStyle::Solid,
            pattern: Default::default(),
        }
    }
}
/// Application preferences copied into new objects, never looked up by existing shapes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShapeDefaults {
    pub style: ShapeStyle,
    /// None retains the adaptive creation radius; configured radii clamp to bounds.
    pub corner_radius: Option<f64>,
    pub arrow_end: bool,
}
impl Default for ShapeDefaults {
    fn default() -> Self {
        Self {
            style: ShapeStyle::default(),
            corner_radius: None,
            arrow_end: true,
        }
    }
}
impl ShapeDefaults {
    pub fn valid(&self) -> bool {
        self.style.valid()
            && self
                .corner_radius
                .is_none_or(|v| v.is_finite() && (0.0..=1e9).contains(&v))
    }
    pub fn apply(&self, shape: &mut Shape) {
        let kind = shape.kind();
        shape.style = self.style.clone();
        if let Some(radius) = self.corner_radius {
            StylePatch::Radius(radius).apply(shape);
        }
        if kind == ShapeKind::Arrow {
            StylePatch::Heads(self.style.start_head, self.arrow_end).apply(shape);
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arrowhead {
    pub length: f64,
    pub width: f64,
}
impl Default for Arrowhead {
    fn default() -> Self {
        Self {
            length: 12.,
            width: 10.,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "geometry", deny_unknown_fields)]
pub enum ShapeGeometry {
    Box {
        kind: ShapeKind,
        size: Point,
        corner_radius: f64,
    },
    /// Derived connector display path; canonical routing lives on Connector.
    #[serde(skip)]
    Path {
        points: Vec<Point>,
        tangents: [Point; 2],
        head: Option<Arrowhead>,
    },
    Segment {
        start: Point,
        end: Point,
        head: Option<Arrowhead>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shape {
    pub id: ObjectId,
    pub document_id: Uuid,
    pub surface: Surface,
    pub placement: Placement,
    pub geometry: ShapeGeometry,
    pub style: ShapeStyle,
    #[serde(default)]
    pub text: Option<ShapeText>,
    /// Present only for first-class sticky-note shapes; colors use `style.fill`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticky_note: Option<StickyNoteOptions>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StickyNoteOptions {
    pub shadow: bool,
    pub auto_height: bool,
}
pub const STICKY_NOTE_PALETTE: [(&str, [u8; 3]); 8] = [
    ("Yellow", [255, 244, 177]),
    ("Pink", [255, 218, 226]),
    ("Blue", [205, 231, 255]),
    ("Green", [215, 241, 205]),
    ("Purple", [231, 218, 255]),
    ("Orange", [255, 224, 190]),
    ("White", [255, 255, 255]),
    ("Custom", [0, 0, 0]),
];
fn relative_luminance(color: [u8; 3]) -> f64 {
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color[0]) + 0.7152 * channel(color[1]) + 0.0722 * channel(color[2])
}
impl Default for StickyNoteOptions {
    fn default() -> Self {
        Self {
            shadow: true,
            auto_height: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerticalAlignment {
    Top,
    #[default]
    Middle,
    Bottom,
}

/// Rich text belongs to the shape snapshot and shares the standalone text model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeText {
    pub content: std::sync::Arc<TextContent>,
    #[serde(default)]
    pub vertical_alignment: VerticalAlignment,
}
impl Default for ShapeText {
    fn default() -> Self {
        let mut content = TextContent::default();
        content.style.alignment = crate::text::Alignment::Center;
        Self {
            content: std::sync::Arc::new(content),
            vertical_alignment: VerticalAlignment::Middle,
        }
    }
}
impl ShapeText {
    pub fn valid(&self) -> bool {
        self.content.valid()
            && self.content.text.len() <= crate::text::MAX_TEXT_BYTES
            && !self.content.text.contains('\0')
    }
}
impl Shape {
    pub fn create(
        owner: Uuid,
        surface: Surface,
        kind: ShapeKind,
        start: Point,
        mut end: Point,
        constrain: bool,
    ) -> Option<Self> {
        let mut delta = [end[0] - start[0], end[1] - start[1]];
        if kind.segment() && constrain {
            let length = delta[0].hypot(delta[1]);
            let angle = (delta[1].atan2(delta[0]) / std::f64::consts::FRAC_PI_4).round()
                * std::f64::consts::FRAC_PI_4;
            delta = [length * angle.cos(), length * angle.sin()];
            end = std::array::from_fn(|i| start[i] + delta[i]);
        } else if !kind.segment() && (constrain || kind == ShapeKind::Circle) {
            let extent = delta[0].abs().max(delta[1].abs());
            end = std::array::from_fn(|i| start[i] + if delta[i] < 0. { -extent } else { extent });
        }
        let min = std::array::from_fn(|i| start[i].min(end[i]));
        let size: Point = std::array::from_fn(|i| (end[i] - start[i]).abs());
        let geometry = if kind.segment() {
            if size[0].hypot(size[1]) < 0.01 {
                return None;
            }
            ShapeGeometry::Segment {
                start: std::array::from_fn(|i| start[i] - min[i]),
                end: std::array::from_fn(|i| end[i] - min[i]),
                head: (kind == ShapeKind::Arrow).then(Arrowhead::default),
            }
        } else {
            if size.iter().any(|x| *x < 0.01) {
                return None;
            }
            ShapeGeometry::Box {
                kind,
                size,
                corner_radius: 12.0_f64.min(size[0].min(size[1]) * 0.2),
            }
        };
        let shape = Self {
            id: Uuid::new_v4(),
            document_id: owner,
            surface,
            placement: Placement {
                origin: [0.; 2],
                position: min,
                ..Default::default()
            },
            geometry,
            style: ShapeStyle::default(),
            text: None,
            sticky_note: None,
        };
        shape.valid().then_some(shape)
    }
    pub fn create_sticky_note(
        owner: Uuid,
        surface: Surface,
        position: Point,
        size: Point,
        color: [u8; 3],
    ) -> Option<Self> {
        let mut note = Self::create(
            owner,
            surface,
            ShapeKind::RoundedRectangle,
            position,
            [position[0] + size[0], position[1] + size[1]],
            false,
        )?;
        note.style.fill = Some(color);
        note.style.fill_opacity = 255;
        note.style.border = [174, 150, 81, 255];
        note.style.border_width = 0.75;
        note.geometry = match note.geometry {
            ShapeGeometry::Box { corner_radius, .. } => ShapeGeometry::Box {
                kind: ShapeKind::RoundedRectangle,
                size,
                corner_radius: corner_radius.min(8.),
            },
            _ => unreachable!(),
        };
        let mut text = ShapeText {
            vertical_alignment: VerticalAlignment::Top,
            ..ShapeText::default()
        };
        let content = std::sync::Arc::make_mut(&mut text.content);
        content.style.alignment = crate::text::Alignment::Left;
        content.style.size = 16.;
        content.style.color = if relative_luminance(color) < 0.42 {
            [255, 255, 255, 255]
        } else {
            [42, 39, 31, 255]
        };
        note.text = Some(text);
        note.sticky_note = Some(StickyNoteOptions::default());
        note.valid().then_some(note)
    }
    pub fn is_sticky_note(&self) -> bool {
        self.sticky_note.is_some()
    }
    pub fn text_background_contrast_ok(&self) -> bool {
        let Some(note) = &self.text else { return true };
        let Some(background) = self.style.fill else {
            return true;
        };
        let bg = relative_luminance(background);
        std::iter::once(&note.content.style)
            .chain(note.content.runs.iter().map(|run| &run.style))
            .all(|style| {
                let text = relative_luminance([style.color[0], style.color[1], style.color[2]]);
                let (light, dark) = if bg >= text { (bg, text) } else { (text, bg) };
                (light + 0.05) / (dark + 0.05) >= 4.5
            })
    }
    pub fn kind(&self) -> ShapeKind {
        match self.geometry {
            ShapeGeometry::Box { kind, .. } => kind,
            ShapeGeometry::Segment { head, .. } | ShapeGeometry::Path { head, .. } => {
                if head.is_some() {
                    ShapeKind::Arrow
                } else {
                    ShapeKind::Line
                }
            }
        }
    }
    pub fn valid(&self) -> bool {
        !self.id.is_nil()
            && !self.document_id.is_nil()
            && self.placement.valid()
            && self
                .placement
                .origin
                .iter()
                .chain(&self.placement.position)
                .all(|v| v.abs() <= 1e15)
            && self.style.valid()
            && self.text.as_ref().is_none_or(ShapeText::valid)
            && self.sticky_note.is_none_or(|_| {
                matches!(
                    self.geometry,
                    ShapeGeometry::Box {
                        kind: ShapeKind::RoundedRectangle,
                        ..
                    }
                ) && self.text.is_some()
            })
            && match self.geometry {
                ShapeGeometry::Path {
                    ref points,
                    tangents,
                    head,
                } => {
                    tangents.iter().flatten().all(|v| v.is_finite())
                        && head.is_none_or(|h| {
                            h.length.is_finite()
                                && h.width.is_finite()
                                && (0.1..=1e4).contains(&h.length)
                                && (0.1..=1e4).contains(&h.width)
                        })
                        && points.len() >= 2
                        && points.len() <= 16385
                        && points
                            .iter()
                            .flatten()
                            .all(|v| v.is_finite() && v.abs() <= 1e15)
                }
                ShapeGeometry::Box {
                    kind,
                    size,
                    corner_radius,
                } => {
                    !kind.segment()
                        && size
                            .iter()
                            .all(|x| x.is_finite() && (0.01..=1e9).contains(x))
                        && corner_radius.is_finite()
                        && (0.0..=1e9).contains(&corner_radius)
                        && (kind != ShapeKind::Circle || (size[0] - size[1]).abs() < 1e-8)
                }
                ShapeGeometry::Segment { start, end, head } => {
                    start
                        .iter()
                        .chain(&end)
                        .all(|x| x.is_finite() && x.abs() <= 1e9)
                        && (end[0] - start[0]).hypot(end[1] - start[1]) >= 0.01
                        && head.is_none_or(|h| {
                            h.length.is_finite()
                                && h.width.is_finite()
                                && (0.1..=1e4).contains(&h.length)
                                && (0.1..=1e4).contains(&h.width)
                        })
                }
            }
    }
    /// Conservative interior rectangle, inscribed within each supported form.
    /// Sloped/curved forms use their narrowest horizontal chord across the text
    /// region so every wrapped row remains safely inside the visible contour.
    pub fn text_region(&self) -> Option<ObjectBounds> {
        let ShapeGeometry::Box {
            kind,
            size: [w, h],
            corner_radius,
        } = self.geometry
        else {
            return None;
        };
        let pad = 8.0_f64.min(w * 0.1).min(h * 0.1);
        if self.is_sticky_note() {
            let pad = 14.0_f64.min(w * 0.2).min(h * 0.2);
            return Some(ObjectBounds {
                min: [pad, pad],
                max: [(w - pad).max(pad), (h - pad).max(pad)],
            });
        }
        let (x, y, width, height) = match kind {
            ShapeKind::Rectangle => (pad, pad, w - 2. * pad, h - 2. * pad),
            ShapeKind::RoundedRectangle => {
                let inset = pad.max(corner_radius.min(w.min(h) * 0.5) * 0.45);
                (inset, inset, w - 2. * inset, h - 2. * inset)
            }
            ShapeKind::Circle | ShapeKind::Ellipse => {
                let y = h * 0.2;
                let height = h * 0.6;
                let chord = w * 0.8;
                let width = chord - 2. * pad;
                ((w - width) * 0.5, y, width, height)
            }
            ShapeKind::Diamond => {
                let y = h * 0.25;
                let height = h * 0.5;
                let width = w * 0.5 - 2. * pad;
                ((w - width) * 0.5, y, width, height)
            }
            ShapeKind::Triangle => {
                let y = h * 0.3;
                let height = h * 0.55;
                let width = w * 0.3 - 2. * pad;
                ((w - width) * 0.5, y, width, height)
            }
            ShapeKind::Line | ShapeKind::Arrow => return None,
        };
        Some(ObjectBounds {
            min: [x.max(0.), y.max(0.)],
            max: [(x + width).max(x.max(0.)), (y + height).max(y.max(0.))],
        })
    }
    /// Local note grips ordered TL, T, TR, R, BR, B, BL, L.
    pub fn note_resize_handles(&self) -> Option<[Point; 8]> {
        let ShapeGeometry::Box { size: [w, h], .. } = self.geometry else {
            return None;
        };
        self.is_sticky_note().then(|| {
            [
                [0., 0.],
                [w * 0.5, 0.],
                [w, 0.],
                [w, h * 0.5],
                [w, h],
                [w * 0.5, h],
                [0., h],
                [0., h * 0.5],
            ]
            .map(|point| self.placement.apply(point))
        })
    }
    pub fn with_note_handle(&self, index: usize, point: Point) -> Option<Self> {
        let handles = self.note_resize_handles()?;
        let opposite = self.placement.inverse(handles[(index + 4) % 8]);
        let target = self.placement.inverse(point);
        let ShapeGeometry::Box {
            size: [w, h],
            corner_radius,
            ..
        } = self.geometry
        else {
            return None;
        };
        let (mut min, mut max) = ([0., 0.], [w, h]);
        if index.is_multiple_of(2) {
            min = std::array::from_fn(|i| target[i].min(opposite[i]));
            max = std::array::from_fn(|i| target[i].max(opposite[i]));
        } else if index % 4 == 1 {
            min[1] = target[1].min(opposite[1]);
            max[1] = target[1].max(opposite[1]);
        } else {
            min[0] = target[0].min(opposite[0]);
            max[0] = target[0].max(opposite[0]);
        }
        let size = [max[0] - min[0], max[1] - min[1]];
        if size.iter().any(|v| !v.is_finite() || *v < 36.) {
            return None;
        }
        let mut out = self.clone();
        out.geometry = ShapeGeometry::Box {
            kind: ShapeKind::RoundedRectangle,
            size,
            corner_radius: corner_radius.min(8.).min(size[0].min(size[1]) * 0.2),
        };
        out.placement.origin = [0.; 2];
        out.placement.position = self.placement.apply(min);
        out.sticky_note.as_mut()?.auto_height = false;
        out.valid().then_some(out)
    }
    /// Closed convex boundaries or independent shaft/head paths. Curves use a
    /// bounded chord-error tolerance, shared by hits/regions and display caches.
    pub fn contours(&self, tolerance: f64) -> Vec<Vec<Point>> {
        let local = match self.geometry {
            ShapeGeometry::Path {
                ref points,
                tangents,
                head,
            } => {
                let mut paths = vec![points.clone()];
                if points.len() > 1 {
                    let length = points
                        .windows(2)
                        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                        .sum::<f64>()
                        .max(0.01);
                    if let Some(h) = head {
                        let end = *points.last().unwrap();
                        let p = segment_points(
                            [
                                end[0] - tangents[1][0] * length,
                                end[1] - tangents[1][1] * length,
                            ],
                            end,
                            Some(h),
                        );
                        paths.push(vec![p[1], p[2], p[3], p[1]]);
                    }
                    if self.style.start_head {
                        let p = segment_points(
                            [
                                points[0][0] + tangents[0][0] * length,
                                points[0][1] + tangents[0][1] * length,
                            ],
                            points[0],
                            Some(Arrowhead::default()),
                        );
                        paths.push(vec![p[1], p[2], p[3], p[1]]);
                    }
                }
                paths
            }
            ShapeGeometry::Segment { start, end, head } => {
                let mut paths = vec![vec![start, end]];
                if head.is_some() {
                    let points = segment_points(start, end, head);
                    paths.push(vec![points[1], points[2], points[3], points[1]]);
                }
                if self.style.start_head {
                    let points = segment_points(end, start, Some(Arrowhead::default()));
                    paths.push(vec![points[1], points[2], points[3], points[1]]);
                }
                paths
            }
            ShapeGeometry::Box {
                kind,
                size: [w, h],
                corner_radius,
            } => {
                let mut points = match kind {
                    ShapeKind::Triangle => vec![[w * 0.5, 0.], [w, h], [0., h]],
                    ShapeKind::Diamond => {
                        vec![[w * 0.5, 0.], [w, h * 0.5], [w * 0.5, h], [0., h * 0.5]]
                    }
                    ShapeKind::Circle | ShapeKind::Ellipse => {
                        let n = ((std::f64::consts::PI
                            * (w.max(h) * self.placement.scale / tolerance.max(0.001)).sqrt())
                        .ceil() as usize)
                            .clamp(32, 2048);
                        (0..n)
                            .map(|i| {
                                let a = i as f64 / n as f64 * std::f64::consts::TAU;
                                [w * 0.5 * (1. + a.cos()), h * 0.5 * (1. + a.sin())]
                            })
                            .collect()
                    }
                    ShapeKind::RoundedRectangle => {
                        let r = corner_radius.min(w.min(h) * 0.5);
                        let n = ((r * self.placement.scale / tolerance.max(0.001))
                            .sqrt()
                            .ceil() as usize)
                            .clamp(4, 512);
                        let mut points = Vec::new();
                        for (i, center) in [[w - r, h - r], [r, h - r], [r, r], [w - r, r]]
                            .into_iter()
                            .enumerate()
                        {
                            for j in 0..=n {
                                let a =
                                    (i as f64 + j as f64 / n as f64) * std::f64::consts::FRAC_PI_2;
                                points.push([center[0] + r * a.cos(), center[1] + r * a.sin()]);
                            }
                        }
                        points
                    }
                    _ => vec![[0., 0.], [w, 0.], [w, h], [0., h]],
                };
                points.push(points[0]);
                vec![points]
            }
        };
        local
            .into_iter()
            .map(|p| p.into_iter().map(|v| self.placement.apply(v)).collect())
            .collect()
    }
    pub fn geometry_bounds(&self) -> ObjectBounds {
        match self.geometry {
            ShapeGeometry::Box {
                kind,
                size: [w, h],
                corner_radius,
            } => {
                if matches!(
                    kind,
                    ShapeKind::Circle | ShapeKind::Ellipse | ShapeKind::RoundedRectangle
                ) {
                    let center = self.placement.apply([w * 0.5, h * 0.5]);
                    let (sin, cos) = self.placement.rotation.sin_cos();
                    let scale = self.placement.scale;
                    let extent = if kind == ShapeKind::RoundedRectangle {
                        let r = corner_radius.min(w.min(h) * 0.5);
                        [
                            (cos.abs() * (w * 0.5 - r) + sin.abs() * (h * 0.5 - r) + r) * scale,
                            (sin.abs() * (w * 0.5 - r) + cos.abs() * (h * 0.5 - r) + r) * scale,
                        ]
                    } else {
                        [
                            (w * 0.5 * cos).hypot(h * 0.5 * sin) * scale,
                            (w * 0.5 * sin).hypot(h * 0.5 * cos) * scale,
                        ]
                    };
                    ObjectBounds {
                        min: std::array::from_fn(|i| center[i] - extent[i]),
                        max: std::array::from_fn(|i| center[i] + extent[i]),
                    }
                } else {
                    let vertices = match kind {
                        ShapeKind::Triangle => [[w * 0.5, 0.], [w, h], [0., h], [w * 0.5, 0.]],
                        ShapeKind::Diamond => {
                            [[w * 0.5, 0.], [w, h * 0.5], [w * 0.5, h], [0., h * 0.5]]
                        }
                        _ => [[0., 0.], [w, 0.], [w, h], [0., h]],
                    };
                    ObjectBounds::from_points(vertices.map(|p| self.placement.apply(p))).unwrap()
                }
            }
            ShapeGeometry::Segment { .. } | ShapeGeometry::Path { .. } => {
                ObjectBounds::from_points(self.contours(0.025).into_iter().flatten()).unwrap()
            }
        }
    }
    pub fn filled(&self) -> bool {
        !self.kind().segment() && self.style.fill.is_some() && self.style.fill_opacity > 0
    }
    pub fn rectangle_matches(&self, bounds: ObjectBounds, crossing: bool) -> bool {
        let paths = self.contours(0.025);
        let width = self.style.envelope_width() * self.placement.scale;
        if !crossing {
            return paths.iter().all(|p| {
                crate::selection::rectangle_matches(
                    bounds,
                    false,
                    p,
                    &[],
                    width,
                    Default::default(),
                )
            });
        }
        paths.iter().any(|p| {
            crate::selection::rectangle_matches(bounds, true, p, &[], width, Default::default())
        }) || (self.filled()
            && bounds
                .corners()
                .iter()
                .any(|p| crate::selection::inside(*p, &paths[0])))
            || (self.kind().segment()
                && paths.iter().skip(1).any(|head| {
                    bounds
                        .corners()
                        .iter()
                        .any(|p| crate::selection::inside(*p, head))
                }))
    }
    pub fn lasso_matches(&self, region: &crate::selection::LassoRegion) -> bool {
        let paths = self.contours(0.025);
        if self.filled()
            && region
                .boundary()
                .iter()
                .any(|p| crate::selection::inside(*p, &paths[0]))
        {
            return false;
        }
        paths.iter().all(|p| {
            region.encloses(
                p,
                &[],
                self.style.envelope_width() * self.placement.scale,
                Default::default(),
            )
        })
    }
    pub fn box_corners(&self) -> Option<[Point; 4]> {
        if let ShapeGeometry::Box { size, .. } = self.geometry {
            Some(
                ObjectBounds {
                    min: [0.; 2],
                    max: size,
                }
                .corners()
                .map(|p| self.placement.apply(p)),
            )
        } else {
            None
        }
    }
    pub fn with_corner(&self, index: usize, point: Point, constrain: bool) -> Option<Self> {
        let ShapeGeometry::Box {
            kind,
            size,
            corner_radius,
        } = self.geometry
        else {
            return None;
        };
        let corners = ObjectBounds {
            min: [0.; 2],
            max: size,
        }
        .corners();
        let anchor = corners[(index + 2) % 4];
        let mut target = self.placement.inverse(point);
        if constrain || kind == ShapeKind::Circle {
            let delta: Point = std::array::from_fn(|i| target[i] - anchor[i]);
            let factor = (delta[0].abs() / size[0]).max(delta[1].abs() / size[1]);
            target = std::array::from_fn(|i| {
                anchor[i] + size[i] * factor * if delta[i] < 0. { -1. } else { 1. }
            });
        }
        let min: Point = std::array::from_fn(|i| anchor[i].min(target[i]));
        let new_size: Point = std::array::from_fn(|i| (target[i] - anchor[i]).abs());
        let mut out = self.clone();
        out.geometry = ShapeGeometry::Box {
            kind,
            size: new_size,
            corner_radius,
        };
        out.placement.origin = [0.; 2];
        out.placement.position = self.placement.apply(min);
        out.valid().then_some(out)
    }
    pub fn endpoints(&self) -> Option<[Point; 2]> {
        if let ShapeGeometry::Segment { start, end, .. } = self.geometry {
            Some([self.placement.apply(start), self.placement.apply(end)])
        } else if let ShapeGeometry::Path { ref points, .. } = self.geometry {
            Some([
                self.placement.apply(points[0]),
                self.placement.apply(*points.last()?),
            ])
        } else {
            None
        }
    }
    pub fn with_endpoint(&self, index: usize, point: Point, constrain: bool) -> Option<Self> {
        let mut out = self.clone();
        let mut ends = self.endpoints()?;
        let fixed = ends[1 - index];
        let point = if constrain {
            let d = [point[0] - fixed[0], point[1] - fixed[1]];
            let len = d[0].hypot(d[1]);
            let a = (d[1].atan2(d[0]) / std::f64::consts::FRAC_PI_4).round()
                * std::f64::consts::FRAC_PI_4;
            [fixed[0] + len * a.cos(), fixed[1] + len * a.sin()]
        } else {
            point
        };
        ends[index] = point;
        if let ShapeGeometry::Segment { start, end, .. } = &mut out.geometry {
            *start = self.placement.inverse(ends[0]);
            *end = self.placement.inverse(ends[1]);
        }
        out.valid().then_some(out)
    }
}
// The same shaft/head vertices feed bounds, hits, rendering and future export.
fn segment_points(start: Point, end: Point, head: Option<Arrowhead>) -> [Point; 4] {
    let Some(head) = head else {
        return [start, end, end, end];
    };
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len = dx.hypot(dy);
    let length = head.length.min(len * 0.5);
    let width = head.width.min(len) * 0.5;
    let base = [end[0] - dx / len * length, end[1] - dy / len * length];
    [
        start,
        end,
        [base[0] - dy / len * width, base[1] + dx / len * width],
        [base[0] + dy / len * width, base[1] - dx / len * width],
    ]
}
impl CanvasObject for Shape {
    fn object_id(&self) -> ObjectId {
        self.id
    }
    fn object_type(&self) -> ObjectType {
        ObjectType::Shape
    }
    fn document_id(&self) -> Uuid {
        self.document_id
    }
    fn surface(&self) -> Surface {
        self.surface
    }
    fn placement(&self) -> Placement {
        self.placement
    }
    fn local_bounds(&self) -> Option<ObjectBounds> {
        let mut local = self.clone();
        local.placement = Placement::default();
        Some(
            local
                .geometry_bounds()
                .expand(self.style.envelope_width() * 0.5),
        )
    }
    fn world_bounds(&self) -> Option<ObjectBounds> {
        Some(
            self.geometry_bounds()
                .expand(self.style.envelope_width() * self.placement.scale * 0.5),
        )
    }
    fn geometry_bounds(&self) -> Option<ObjectBounds> {
        Some(Shape::geometry_bounds(self))
    }
    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        if !self.valid()
            || !point.iter().all(|p| p.is_finite())
            || !tolerance.is_finite()
            || tolerance < 0.0
        {
            return false;
        }
        let paths = self.contours(0.025);
        (self.filled() && crate::selection::inside(point, &paths[0]))
            || paths.iter().enumerate().any(|(i, p)| {
                (self.kind().segment() && i > 0 && crate::selection::inside(point, p))
                    || crate::drawing::hit_path(
                        p,
                        point,
                        point,
                        tolerance + self.style.envelope_width() * self.placement.scale * 0.5,
                    )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shape(kind: ShapeKind) -> Shape {
        Shape::create(
            Uuid::new_v4(),
            Surface::Canvas,
            kind,
            [20., 30.],
            [120., 90.],
            false,
        )
        .unwrap()
    }
    #[test]
    fn visible_geometry_fill_outline_curves_and_arrowheads() {
        for kind in ShapeKind::ALL {
            let shape = shape(kind);
            assert!(shape.valid());
            let mut placed = shape.clone();
            placed.placement.rotation = 0.43;
            placed.placement.scale = 1.7;
            placed.placement.reflected = [true, false];
            let bounds = placed.geometry_bounds().expand(1e-8);
            assert!(
                placed
                    .contours(0.025)
                    .iter()
                    .flatten()
                    .all(|p| bounds.contains(*p))
            );
            if kind == ShapeKind::Circle {
                let b = placed.geometry_bounds();
                assert!(((b.max[0] - b.min[0]) - (b.max[1] - b.min[1])).abs() < 1e-8);
            }

            assert_eq!(
                serde_json::from_str::<Shape>(&serde_json::to_string(&shape).unwrap()).unwrap(),
                shape
            );
            for path in shape.contours(0.05) {
                for point in path {
                    assert!(shape.hit_test(point, 0.1));
                }
            }
            let enclosed = ObjectBounds {
                min: [0., 0.],
                max: [150., 150.],
            };
            assert!(shape.rectangle_matches(enclosed, false));
            let lasso = crate::selection::LassoRegion::new(enclosed.corners().to_vec()).unwrap();
            assert!(shape.lasso_matches(&lasso));
        }
        let mut ellipse = shape(ShapeKind::Ellipse);
        assert!(!ellipse.hit_test([21., 31.], 0.));
        assert!(ellipse.hit_test([70., 60.], 0.));
        ellipse.style.fill = None;
        assert!(!ellipse.hit_test([70., 60.], 0.));
        assert!(ellipse.hit_test([120., 60.], 0.));
        let line = shape(ShapeKind::Line);
        assert!(!line.hit_test([25., 85.], 1.));
        assert!(!line.rectangle_matches(
            ObjectBounds {
                min: [20., 75.],
                max: [35., 90.]
            },
            true
        ));
        assert!(line.rectangle_matches(
            ObjectBounds {
                min: [60., 50.],
                max: [75., 65.]
            },
            true
        ));
        assert!(!line.rectangle_matches(
            ObjectBounds {
                min: [60., 50.],
                max: [75., 65.]
            },
            false
        ));
        let arrow = shape(ShapeKind::Arrow);
        let paths = arrow.contours(0.025);
        let head = &paths[1];
        let center: Point = std::array::from_fn(|i| (head[0][i] + head[1][i] + head[2][i]) / 3.);
        assert!(arrow.rectangle_matches(
            ObjectBounds {
                min: center.map(|v| v - 0.01),
                max: center.map(|v| v + 0.01)
            },
            true
        ));
        let triangle = shape(ShapeKind::Triangle);
        assert!(!triangle.hit_test([21., 31.], 0.));
        let rounded = shape(ShapeKind::RoundedRectangle);
        assert!(!rounded.hit_test([20., 30.], 0.));
        let filled = shape(ShapeKind::Rectangle);
        assert!(filled.rectangle_matches(
            ObjectBounds {
                min: [60., 50.],
                max: [70., 60.]
            },
            true
        ));
    }
    #[test]
    fn normalization_constraints_invalid_geometry_and_endpoint_direction() {
        for kind in ShapeKind::ALL {
            let forward = shape(kind);
            let reverse = Shape::create(
                forward.document_id,
                forward.surface,
                kind,
                [120., 90.],
                [20., 30.],
                false,
            )
            .unwrap();
            assert!(reverse.valid());
            if let Some(ends) = reverse.endpoints() {
                assert_eq!(ends, [[120., 90.], [20., 30.]]);
            } else if let ShapeGeometry::Box { size, .. } = reverse.geometry {
                assert!(size.iter().all(|x| *x > 0.));
            }
            assert!(
                Shape::create(
                    forward.document_id,
                    forward.surface,
                    kind,
                    [0., 0.],
                    [0., 0.],
                    false
                )
                .is_none()
            );
        }
        let circle = shape(ShapeKind::Circle);
        if let ShapeGeometry::Box { size, .. } = circle.geometry {
            assert_eq!(size[0], size[1]);
        }
        let mut invalid = shape(ShapeKind::Rectangle);
        invalid.placement.scale = -1.;
        assert!(!invalid.valid());
        invalid.placement = Placement::default();
        invalid.style.border_width = f64::NAN;
        assert!(!invalid.valid());
        let arrow = shape(ShapeKind::Arrow);
        let edited = arrow.with_endpoint(0, [-20., 10.], true).unwrap();
        assert_eq!(edited.id, arrow.id);
        assert_eq!(edited.style, arrow.style);
        assert_eq!(
            edited.endpoints().unwrap()[1],
            arrow.endpoints().unwrap()[1]
        );
    }
}

/// A property patch changes only compatible content; geometry/identity stay intact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StylePatch {
    Fill(Option<[u8; 3]>),
    FillOpacity(u8),
    Border([u8; 3]),
    BorderOpacity(u8),
    Width(f64),
    Pattern(BorderStyle),
    DashLength(f64),
    GapLength(f64),
    DotSpacing(f64),
    DotSize(f64),
    BorderEnabled(bool),
    Heads(bool, bool),
    Radius(f64),
    StickyShadow(bool),
    StickyAutoHeight(bool),
}
impl StylePatch {
    pub fn apply(self, shape: &mut Shape) {
        match self {
            Self::Fill(v) if !shape.kind().segment() => shape.style.fill = v,
            Self::FillOpacity(v) if !shape.kind().segment() => shape.style.fill_opacity = v,
            Self::Border(v) => shape.style.border[..3].copy_from_slice(&v),
            Self::BorderOpacity(v) => shape.style.border[3] = v,
            Self::Width(v) => shape.style.border_width = v,
            Self::Pattern(v) => shape.style.border_style = v,
            Self::DashLength(v) => shape.style.pattern.dash = v,
            Self::GapLength(v) => shape.style.pattern.gap = v,
            Self::DotSpacing(v) => {
                shape.style.pattern.dot_spacing = v;
                shape.style.pattern.dot_size = shape.style.pattern.dot_size.min(v);
            }
            Self::DotSize(v) => {
                shape.style.pattern.dot_size = v;
                shape.style.pattern.dot_spacing = shape.style.pattern.dot_spacing.max(v);
            }
            Self::BorderEnabled(v) => shape.style.border_enabled = v,
            Self::Heads(start, end) => {
                if let ShapeGeometry::Segment { head, .. } = &mut shape.geometry {
                    *head = if end {
                        Some(head.unwrap_or_default())
                    } else {
                        None
                    };
                    shape.style.start_head = start;
                }
            }
            Self::Radius(v) => {
                if let ShapeGeometry::Box {
                    kind: ShapeKind::RoundedRectangle,
                    corner_radius,
                    size,
                } = &mut shape.geometry
                    && v.is_finite()
                {
                    *corner_radius = v.clamp(0., size[0].min(size[1]) * 0.5);
                }
            }
            Self::StickyShadow(value) => {
                if let Some(note) = &mut shape.sticky_note {
                    note.shadow = value;
                }
            }
            Self::StickyAutoHeight(value) => {
                if let Some(note) = &mut shape.sticky_note {
                    note.auto_height = value;
                }
            }
            _ => (),
        }
    }
}
