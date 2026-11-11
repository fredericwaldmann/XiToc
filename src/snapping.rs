//! Document-space alignment and cached gesture snapping, independent of UI/IO.
use crate::object::{ObjectBounds, ObjectId, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sensitivity {
    Low,
    #[default]
    Normal,
    High,
}
impl Sensitivity {
    pub fn threshold(self) -> f64 {
        match self {
            Self::Low => 4.,
            Self::Normal => 8.,
            Self::High => 12.,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low · 4 pt",
            Self::Normal => "Normal · 8 pt",
            Self::High => "High · 12 pt",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SnapSettings {
    pub objects: bool,
    pub guides: bool,
    pub grid: bool,
    pub show_grid: bool,
    pub spacing: f64,
    pub sensitivity: Sensitivity,
}
impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            objects: true,
            guides: true,
            grid: false,
            show_grid: true,
            spacing: 32.,
            sensitivity: Sensitivity::Normal,
        }
    }
}
impl SnapSettings {
    pub fn valid(self) -> bool {
        self.spacing.is_finite() && (1.0..=4096.).contains(&self.spacing)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub id: ObjectId,
    pub bounds: ObjectBounds,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub axis: usize,
    pub coordinate: f64,
    pub from: f64,
    pub to: f64,
    pub gaps: Option<[[f64; 2]; 2]>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Feature {
    value: f64,
    target: usize,
    kind: usize,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Key {
    Edge {
        target: usize,
        source: usize,
        kind: usize,
    },
    Spacing {
        left: usize,
        right: usize,
    },
    Grid {
        source: usize,
        value: f64,
    },
}
#[derive(Clone, Copy, Debug)]
struct Match {
    key: Key,
    correction: f64,
}

/// Targets and sorted features are captured once at gesture start, never from a
/// snapped preview. Latches are gesture-only; document revision/view owns validity.
pub struct SnapSession {
    pub moving_bounds: Option<ObjectBounds>,
    pub settings: SnapSettings,
    targets: Vec<Target>,
    features: [Vec<Feature>; 2],
    ends: [Vec<usize>; 2],
    starts: [Vec<usize>; 2],
    latch: [Option<Match>; 2],
    pub guides: [Option<Guide>; 2],
}
fn value(b: ObjectBounds, axis: usize, kind: usize) -> f64 {
    match kind {
        0 => b.min[axis],
        1 => b.min[axis] + (b.max[axis] - b.min[axis]) * 0.5,
        _ => b.max[axis],
    }
}
impl SnapSession {
    pub fn new(mut targets: Vec<Target>, settings: SnapSettings) -> Self {
        targets.sort_by_key(|t| t.id);
        let features = std::array::from_fn(|axis| {
            let mut features: Vec<_> = targets
                .iter()
                .enumerate()
                .flat_map(|(target, t)| {
                    (0..3).map(move |kind| Feature {
                        value: value(t.bounds, axis, kind),
                        target,
                        kind,
                    })
                })
                .collect();
            features.sort_by(|a, b| {
                a.value
                    .total_cmp(&b.value)
                    .then(a.target.cmp(&b.target))
                    .then(a.kind.cmp(&b.kind))
            });
            features
        });
        let starts = std::array::from_fn(|axis| {
            let mut order: Vec<_> = (0..targets.len()).collect();
            order.sort_by(|a, b| {
                targets[*a].bounds.min[axis]
                    .total_cmp(&targets[*b].bounds.min[axis])
                    .then(a.cmp(b))
            });
            order
        });
        let ends = std::array::from_fn(|axis| {
            let mut order: Vec<_> = (0..targets.len()).collect();
            order.sort_by(|a, b| {
                targets[*a].bounds.max[axis]
                    .total_cmp(&targets[*b].bounds.max[axis])
                    .then(a.cmp(b))
            });
            order
        });
        Self {
            moving_bounds: None,
            settings,
            targets,
            features,
            ends,
            starts,
            latch: [None; 2],
            guides: [None; 2],
        }
    }
    pub fn clear(&mut self) {
        self.latch = [None; 2];
        self.guides = [None; 2];
    }
    /// `active` is None for a move; resizing admits only specified moving edges.
    /// Grid is lower priority than object/spacing snaps and has the same latch.
    pub fn solve(
        &mut self,
        bounds: ObjectBounds,
        scale: f64,
        bypass: bool,
        active: Option<[Option<usize>; 2]>,
    ) -> Point {
        if bypass || !scale.is_finite() || scale <= 0. {
            self.clear();
            return [0.; 2];
        }
        let threshold = self.settings.sensitivity.threshold() / scale;
        let mut result = [0.; 2];
        for axis in 0..2 {
            if active.is_some_and(|a| a[axis].is_none()) {
                self.latch[axis] = None;
                self.guides[axis] = None;
                continue;
            }
            let retained = self.latch[axis]
                .filter(|old| match old.key {
                    Key::Edge { source, .. } | Key::Grid { source, .. } => {
                        active.is_none_or(|a| a[axis] == Some(source))
                    }
                    Key::Spacing { .. } => active.is_none(),
                })
                .and_then(|old| self.evaluate(old.key, bounds, axis))
                .filter(|m| m.correction.abs() <= threshold * 1.5);
            let matched = retained.or_else(|| self.find(bounds, axis, threshold, active));
            self.latch[axis] = matched;
            result[axis] = matched.map_or(0., |m| m.correction);
            self.guides[axis] = matched.and_then(|m| self.guide(m, bounds, axis));
        }
        result
    }
    fn evaluate(&self, key: Key, b: ObjectBounds, axis: usize) -> Option<Match> {
        let correction = match key {
            Key::Edge {
                target,
                source,
                kind,
            } if self.settings.objects => {
                value(self.targets[target].bounds, axis, kind) - value(b, axis, source)
            }
            Key::Grid {
                source,
                value: grid,
            } if self.settings.grid => grid - value(b, axis, source),
            Key::Spacing { left, right } if self.settings.objects => {
                let l = self.targets[left].bounds;
                let r = self.targets[right].bounds;
                if !overlap(l, b, 1 - axis)
                    || !overlap(r, b, 1 - axis)
                    || r.min[axis] - l.max[axis] < b.max[axis] - b.min[axis]
                {
                    return None;
                }
                (r.min[axis] - b.max[axis] - (b.min[axis] - l.max[axis])) * 0.5
            }
            _ => return None,
        };
        Some(Match { key, correction })
    }
    fn find(
        &self,
        b: ObjectBounds,
        axis: usize,
        threshold: f64,
        active: Option<[Option<usize>; 2]>,
    ) -> Option<Match> {
        let mut best: Option<Match> = None;
        let mut consider = |candidate: Match| {
            if candidate.correction.abs() <= threshold
                && best.is_none_or(|old| candidate.correction.abs() < old.correction.abs() - 1e-9)
            {
                best = Some(candidate);
            }
        };
        if self.settings.objects {
            for source in 0..3 {
                if active.is_some_and(|a| a[axis] != Some(source)) {
                    continue;
                }
                let coordinate = value(b, axis, source);
                let list = &self.features[axis];
                let start = list.partition_point(|f| f.value < coordinate - threshold);
                for f in list[start..]
                    .iter()
                    .take_while(|f| f.value <= coordinate + threshold)
                {
                    if (source == 1) != (f.kind == 1) {
                        continue;
                    }
                    consider(Match {
                        key: Key::Edge {
                            target: f.target,
                            source,
                            kind: f.kind,
                        },
                        correction: f.value - coordinate,
                    });
                }
            }
            if active.is_none() {
                let left_end = self.ends[axis].partition_point(|i| {
                    self.targets[*i].bounds.max[axis] <= b.min[axis] + threshold
                });
                let left = self.ends[axis][..left_end]
                    .iter()
                    .rev()
                    .take(32)
                    .find(|i| overlap(self.targets[**i].bounds, b, 1 - axis));
                let right_start = self.starts[axis].partition_point(|i| {
                    self.targets[*i].bounds.min[axis] < b.max[axis] - threshold
                });
                let right = self.starts[axis][right_start..]
                    .iter()
                    .take(32)
                    .find(|i| overlap(self.targets[**i].bounds, b, 1 - axis));
                if let (Some(left), Some(right)) = (left, right)
                    && left != right
                    && let Some(m) = self.evaluate(
                        Key::Spacing {
                            left: *left,
                            right: *right,
                        },
                        b,
                        axis,
                    )
                {
                    consider(m);
                }
            }
        }
        if best.is_some() {
            return best;
        }
        if self.settings.grid && self.settings.valid() {
            // Moving aligns the top/left of the whole selection to the grid.
            let source = active.and_then(|a| a[axis]).unwrap_or(0);
            let coordinate = value(b, axis, source);
            let grid = (coordinate / self.settings.spacing).round() * self.settings.spacing;
            if (grid - coordinate).abs() <= threshold {
                return Some(Match {
                    key: Key::Grid {
                        source,
                        value: grid,
                    },
                    correction: grid - coordinate,
                });
            }
        }
        None
    }
    fn guide(&self, m: Match, b: ObjectBounds, axis: usize) -> Option<Guide> {
        if !self.settings.guides {
            return None;
        }
        let other = 1 - axis;
        match m.key {
            Key::Edge {
                target,
                source: _,
                kind,
            } => {
                let t = self.targets[target].bounds;
                Some(Guide {
                    axis,
                    coordinate: value(t, axis, kind),
                    from: b.min[other].min(t.min[other]),
                    to: b.max[other].max(t.max[other]),
                    gaps: None,
                })
            }
            Key::Spacing { left, right } => {
                let l = self.targets[left].bounds;
                let r = self.targets[right].bounds;
                Some(Guide {
                    axis,
                    coordinate: b.center()[other],
                    from: l.max[axis],
                    to: r.min[axis],
                    gaps: Some([
                        [l.max[axis], b.min[axis] + m.correction],
                        [b.max[axis] + m.correction, r.min[axis]],
                    ]),
                })
            }
            Key::Grid { .. } => None,
        }
    }
    /// Discard feedback for a correction rejected by geometry/page/minimum-size rules.
    pub fn reconcile(&mut self, bounds: ObjectBounds) {
        for axis in 0..2 {
            if let Some(m) = self.latch[axis] {
                let matched = self
                    .evaluate(m.key, bounds, axis)
                    .filter(|m| m.correction.abs() < 1e-6);
                self.latch[axis] = matched;
                self.guides[axis] = matched.and_then(|m| self.guide(m, bounds, axis));
            }
        }
    }
    /// Axis-aligned boxes snap their changing AABB edges. Rotated hosts snap
    /// their active world-space grip; a one-axis edge grip stays on its local rail.
    pub fn handle(
        &mut self,
        bounds: ObjectBounds,
        grip: Point,
        rail: Option<Point>,
        rotated: bool,
        scale: f64,
        bypass: bool,
    ) -> Point {
        let active = std::array::from_fn(|axis| {
            if !rotated && (grip[axis] - bounds.center()[axis]).abs() < 1e-7 {
                None
            } else {
                Some(if rotated || grip[axis] < bounds.center()[axis] {
                    0
                } else {
                    2
                })
            }
        });
        let b = if rotated {
            ObjectBounds {
                min: grip,
                max: grip,
            }
        } else {
            bounds
        };
        let correction = self.solve(b, scale, bypass, Some(active));
        if let Some(direction) = rail {
            let length = direction[0].hypot(direction[1]);
            let cost = |axis: usize| correction[axis].abs() * length / direction[axis].abs();
            let axis = (0..2)
                .filter(|i| direction[*i].abs() > 1e-8 && self.latch[*i].is_some())
                .filter(|i| cost(*i) * scale <= self.settings.sensitivity.threshold() * 1.5)
                .min_by(|a, b| cost(*a).total_cmp(&cost(*b)));
            if let Some(axis) = axis {
                self.latch[1 - axis] = None;
                self.guides[1 - axis] = None;
                std::array::from_fn(|i| grip[i] + direction[i] * correction[axis] / direction[axis])
            } else {
                self.clear();
                grip
            }
        } else {
            std::array::from_fn(|i| grip[i] + correction[i])
        }
    }
    /// Proportional resizing has one degree of freedom. Choose one realizable
    /// axis, then derive the scalar about the original opposite corner.
    pub fn resize(
        &mut self,
        source: ObjectBounds,
        delta: crate::object::Placement,
        corner: usize,
        scale: f64,
        bypass: bool,
    ) -> crate::object::Placement {
        let active = match corner {
            0 => [Some(0), Some(0)],
            1 => [Some(2), Some(0)],
            2 => [Some(2), Some(2)],
            _ => [Some(0), Some(2)],
        };
        let b = source.placed(delta);
        let adjustment = self.solve(b, scale, bypass, Some(active));
        let direction: Point =
            std::array::from_fn(|i| value(source, i, active[i].unwrap()) - delta.origin[i]);
        let length = direction[0].hypot(direction[1]);
        let cost = |axis: usize| adjustment[axis].abs() * length / direction[axis].abs();
        let axis = (0..2)
            .filter(|i| self.latch[*i].is_some())
            .filter(|i| (value(source, *i, active[*i].unwrap()) - delta.origin[*i]).abs() > 1e-9)
            .filter(|i| cost(*i) * scale <= self.settings.sensitivity.threshold() * 1.5)
            .min_by(|a, b| cost(*a).total_cmp(&cost(*b)));
        let Some(axis) = axis else {
            self.clear();
            return delta;
        };
        self.guides[1 - axis] = None;
        self.latch[1 - axis] = None;
        let factor = delta.scale
            + adjustment[axis] / (value(source, axis, active[axis].unwrap()) - delta.origin[axis]);
        if !(1e-4..=1e4).contains(&factor) {
            self.clear();
            return delta;
        }
        crate::object::Placement {
            scale: factor,
            ..delta
        }
    }
}
fn overlap(a: ObjectBounds, b: ObjectBounds, axis: usize) -> bool {
    a.max[axis] > b.min[axis] && b.max[axis] > a.min[axis]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
    DistributeHorizontal,
    DistributeVertical,
}
impl Align {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Align Left",
            Self::Center => "Align Center Horizontally",
            Self::Right => "Align Right",
            Self::Top => "Align Top",
            Self::Middle => "Align Middle Vertically",
            Self::Bottom => "Align Bottom",
            Self::DistributeHorizontal => "Distribute Horizontally",
            Self::DistributeVertical => "Distribute Vertically",
        }
    }
    pub fn minimum(self) -> usize {
        if matches!(self, Self::DistributeHorizontal | Self::DistributeVertical) {
            3
        } else {
            2
        }
    }
}
pub fn alignment(
    targets: &[Target],
    command: Align,
) -> Result<Vec<(ObjectId, Point)>, &'static str> {
    if targets.len() < command.minimum() {
        return Err("Select more unlocked canvas objects in this editing scope.");
    }
    let mut result = Vec::with_capacity(targets.len());
    if matches!(
        command,
        Align::DistributeHorizontal | Align::DistributeVertical
    ) {
        let axis = usize::from(command == Align::DistributeVertical);
        let mut ordered = targets.to_vec();
        ordered.sort_by(|a, b| {
            a.bounds.min[axis]
                .total_cmp(&b.bounds.min[axis])
                .then(a.id.cmp(&b.id))
        });
        let first = ordered[0].bounds;
        let last = ordered.last().unwrap().bounds;
        let occupied: f64 = ordered[1..ordered.len() - 1]
            .iter()
            .map(|t| t.bounds.max[axis] - t.bounds.min[axis])
            .sum();
        let gap = (last.min[axis] - first.max[axis] - occupied) / (ordered.len() - 1) as f64;
        if gap < -1e-9 {
            return Err(
                "Not enough space for equal non-overlapping gaps. Move the outer objects farther apart.",
            );
        }
        let mut position = first.max[axis] + gap.max(0.);
        for t in &ordered[1..ordered.len() - 1] {
            let mut movement = [0.; 2];
            movement[axis] = position - t.bounds.min[axis];
            result.push((t.id, movement));
            position += t.bounds.max[axis] - t.bounds.min[axis] + gap.max(0.);
        }
    } else {
        let bounds = targets
            .iter()
            .map(|t| t.bounds)
            .reduce(ObjectBounds::union)
            .unwrap();
        let (axis, kind) = match command {
            Align::Left => (0, 0),
            Align::Center => (0, 1),
            Align::Right => (0, 2),
            Align::Top => (1, 0),
            Align::Middle => (1, 1),
            _ => (1, 2),
        };
        for t in targets {
            let mut movement = [0.; 2];
            movement[axis] = value(bounds, axis, kind) - value(t.bounds, axis, kind);
            result.push((t.id, movement));
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "snapping_tests.rs"]
mod tests;
