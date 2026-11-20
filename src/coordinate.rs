//! Authored scientific diagrams and platform-independent plot geometry.
use crate::{
    drawing::Surface,
    object::{CanvasObject, ObjectBounds, ObjectType, Placement, Point},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preset {
    Positive,
    Signed,
    FirstQuadrant,
    FourQuadrants,
    Octant,
}
impl Preset {
    pub const ALL: [Self; 5] = [
        Self::Positive,
        Self::Signed,
        Self::FirstQuadrant,
        Self::FourQuadrants,
        Self::Octant,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Positive => "1D Positive",
            Self::Signed => "1D Signed",
            Self::FirstQuadrant => "2D First Quadrant",
            Self::FourQuadrants => "2D Four Quadrants",
            Self::Octant => "3D Positive Octant",
        }
    }
    pub fn dimension(self) -> usize {
        match self {
            Self::Positive | Self::Signed => 1,
            Self::Octant => 3,
            _ => 2,
        }
    }
    fn signed(self) -> bool {
        matches!(self, Self::Signed | Self::FourQuadrants)
    }
    pub fn size(self) -> Point {
        if self.dimension() == 1 {
            [320., 96.]
        } else {
            [330., 300.]
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    pub range: Point,
    pub visible: bool,
    pub show_label: bool,
    pub label: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arrow {
    Scientific,
    Open,
    None,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pattern {
    Solid,
    Dashed,
    Dotted,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plot {
    pub expression: String,
    pub visible: bool,
    pub color: [u8; 3],
    pub width: f64,
    pub pattern: Pattern,
    pub domain: Option<Point>,
    pub y_domain: Option<Point>,
}
impl Plot {
    pub fn new(index: usize, surface: bool) -> Self {
        const COLORS: [[u8; 3]; 6] = [
            [46, 112, 210],
            [209, 70, 80],
            [24, 142, 109],
            [138, 86, 201],
            [202, 110, 27],
            [0, 136, 162],
        ];
        Self {
            expression: if surface { "x^2 + y^2" } else { "x^2" }.into(),
            visible: true,
            color: COLORS[index % COLORS.len()],
            width: 1.5,
            pattern: Pattern::Solid,
            domain: None,
            y_domain: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub start: f64,
    pub end: Option<f64>,
    pub closed: [bool; 2],
    pub color: [u8; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub preset: Preset,
    pub axes: [Axis; 3],
    pub axis_color: Option<[u8; 3]>,
    pub axis_width: f64,
    pub arrow: Arrow,
    pub origin: bool,
    pub origin_label: String,
    pub numbers: bool,
    pub equal_units: bool,
    pub background: Option<[u8; 3]>,
    pub grid: bool,
    pub grid_step: f64,
    pub grid_color: [u8; 3],
    pub grid_width: f64,
    pub grid_opacity: u8,
    pub grid_subdivisions: u8,
    pub ticks: bool,
    pub tick_step: f64,
    pub tick_length: f64,
    pub tick_width: f64,
    pub tick_color: Option<[u8; 3]>,
    pub curves: Vec<Plot>,
    pub surfaces: Vec<Plot>,
    pub markers: Vec<Marker>,
    pub azimuth: f64,
    pub elevation: f64,
}
pub const MAX_PLOTS: usize = 8;
pub const MAX_MARKERS: usize = 32;
pub fn valid_range([low, high]: Point) -> bool {
    low.is_finite() && high.is_finite() && low >= -1e6 && high <= 1e6 && high - low >= 1e-6
}
fn label_valid(value: &str) -> bool {
    value.len() <= 128 && !value.chars().any(char::is_control)
}

impl Config {
    pub fn new(preset: Preset) -> Self {
        let low = if preset.signed() { -5. } else { 0. };
        Self {
            preset,
            axes: ["x", "y", "z"].map(|label| Axis {
                range: [low, 5.],
                visible: true,
                show_label: true,
                label: label.into(),
            }),
            axis_color: None,
            axis_width: 1.,
            arrow: Arrow::Scientific,
            origin: true,
            origin_label: "0".into(),
            numbers: false,
            equal_units: true,
            background: None,
            grid: false,
            grid_step: 1.,
            grid_color: [115, 130, 148],
            grid_width: 0.65,
            grid_opacity: 105,
            grid_subdivisions: 1,
            ticks: false,
            tick_step: 1.,
            tick_length: 5.,
            tick_width: 0.8,
            tick_color: None,
            curves: vec![],
            surfaces: vec![],
            markers: vec![],
            azimuth: 40.,
            elevation: 25.,
        }
    }
    pub fn set_preset(&mut self, preset: Preset) {
        let defaults = Self::new(preset);
        for (index, axis) in self.axes.iter_mut().enumerate().take(preset.dimension()) {
            if preset.signed() && axis.range[0] >= 0. {
                axis.range[0] = defaults.axes[index].range[0];
            } else if !preset.signed() {
                axis.range[0] = 0.;
            }
        }
        self.preset = preset;
    }
    pub fn validate(&self) -> Result<(), String> {
        for (index, axis) in self.axes.iter().enumerate() {
            if !valid_range(axis.range)
                || !label_valid(&axis.label)
                || index < self.preset.dimension()
                    && (axis.range[1] <= 0.
                        || if self.preset.signed() {
                            axis.range[0] >= 0.
                        } else {
                            axis.range[0] != 0.
                        })
            {
                return Err(format!(
                    "Axis {} needs a finite range containing zero and a valid label.",
                    ["X", "Y", "Z"][index]
                ));
            }
        }
        if !label_valid(&self.origin_label)
            || ![self.axis_width, self.grid_width, self.tick_width]
                .into_iter()
                .all(|n| (0.15..=8.).contains(&n))
            || ![self.grid_step, self.tick_step]
                .into_iter()
                .all(|n| (1e-6..=1e6).contains(&n))
            || !(1. ..=24.).contains(&self.tick_length)
            || !(1..=8).contains(&self.grid_subdivisions)
            || !(5. ..=85.).contains(&self.elevation)
            || !(-180. ..=180.).contains(&self.azimuth)
        {
            return Err("Appearance or viewpoint is outside its supported range.".into());
        }
        if self.markers.len() > MAX_MARKERS
            || self.markers.iter().any(|m| {
                !(-1e6..=1e6).contains(&m.start)
                    || m.end.is_some_and(|e| !valid_range([m.start, e]))
            })
        {
            return Err("Enter finite points and intervals with start below end.".into());
        }
        for (surface, plots) in [(false, &self.curves), (true, &self.surfaces)] {
            if plots.len() > MAX_PLOTS {
                return Err("Up to eight plots per dimension are supported.".into());
            }
            for (index, plot) in plots.iter().enumerate() {
                if !(0.15..=8.).contains(&plot.width)
                    || [plot.domain, plot.y_domain]
                        .into_iter()
                        .flatten()
                        .any(|r| !valid_range(r))
                {
                    return Err(format!(
                        "Plot {} has an invalid domain or line width.",
                        index + 1
                    ));
                }
                crate::coordinate_expression::parse(&plot.expression, surface)
                    .map_err(|error| format!("Plot {}: {error}", index + 1))?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coordinate {
    pub id: Uuid,
    pub document_id: Uuid,
    pub surface: Surface,
    pub placement: Placement,
    pub size: Point,
    pub locked: bool,
    pub config: std::sync::Arc<Config>,
    /// Last valid, editable TikZ-subset source applied to this diagram.
    #[serde(default)]
    pub tikz_source: String,
}

impl Coordinate {
    pub fn new(owner: Uuid, surface: Surface, center: Point, preset: Preset) -> Self {
        let size = preset.size();
        Self {
            id: Uuid::new_v4(),
            document_id: owner,
            surface,
            placement: Placement {
                position: [center[0] - size[0] / 2., center[1] - size[1] / 2.],
                ..Default::default()
            },
            size,
            locked: false,
            config: std::sync::Arc::new(Config::new(preset)),
            tikz_source: String::new(),
        }
    }
    pub fn valid(&self) -> bool {
        self.geometry_valid() && self.config.validate().is_ok()
    }
    pub fn geometry_valid(&self) -> bool {
        !self.id.is_nil()
            && !self.document_id.is_nil()
            && self.placement.valid()
            && self.size.iter().all(|n| (48. ..=1e6).contains(n))
            && self
                .corners()
                .iter()
                .flatten()
                .all(|n| n.is_finite() && n.abs() <= 1e15)
    }
    pub fn corners(&self) -> [Point; 4] {
        self.local_bounds()
            .unwrap()
            .corners()
            .map(|p| self.placement.apply(p))
    }
}
impl CanvasObject for Coordinate {
    fn object_id(&self) -> Uuid {
        self.id
    }
    fn object_type(&self) -> ObjectType {
        ObjectType::Coordinate
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
        Some(ObjectBounds {
            min: [0.; 2],
            max: self.size,
        })
    }
    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        self.local_bounds()
            .unwrap()
            .expand(tolerance / self.placement.scale)
            .contains(self.placement.inverse(point))
    }
}
