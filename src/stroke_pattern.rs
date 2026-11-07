//! UI-independent stroke patterns measured in logical document points.
use crate::object::Point;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PatternSettings {
    pub dash: f64,
    pub gap: f64,
    pub dot_spacing: f64,
    pub dot_size: f64,
}
impl Default for PatternSettings {
    fn default() -> Self {
        Self {
            dash: 8.,
            gap: 4.,
            dot_spacing: 5.,
            dot_size: 2.,
        }
    }
}
impl PatternSettings {
    pub fn valid(self) -> bool {
        [self.dash, self.gap, self.dot_spacing, self.dot_size]
            .iter()
            .all(|v| v.is_finite() && (0.25..=256.).contains(v))
            && self.dot_size <= self.dot_spacing
    }
}
const MAX_MARKS: usize = 8192;
/// One measured path, with duplicate closing/consecutive vertices removed. All
/// distances and slices share this cumulative-length table, including corners.
pub struct MeasuredPath {
    pub points: Vec<Point>,
    pub closed: bool,
    distances: Vec<f64>,
}
impl MeasuredPath {
    pub fn new(points: &[Point], closed: bool) -> Self {
        let mut points: Vec<_> = points
            .iter()
            .copied()
            .filter(|p| p.iter().all(|v| v.is_finite()))
            .collect();
        points.dedup();
        if closed && points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        let mut distances = vec![0.];
        let edges = points.len().saturating_sub(1) + usize::from(closed && points.len() > 1);
        for i in 0..edges {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            distances.push(distances[i] + (b[0] - a[0]).hypot(b[1] - a[1]));
        }
        Self {
            points,
            closed,
            distances,
        }
    }
    pub fn length(&self) -> f64 {
        *self.distances.last().unwrap()
    }
    pub fn at(&self, distance: f64) -> Point {
        if self.points.is_empty() {
            return [0.; 2];
        }
        let d = distance.clamp(0., self.length());
        let edge = self
            .distances
            .partition_point(|v| *v <= d)
            .saturating_sub(1)
            .min(self.distances.len().saturating_sub(2));
        if self.distances.len() < 2 {
            return self.points[0];
        }
        let a = self.points[edge];
        let b = self.points[(edge + 1) % self.points.len()];
        let t = (d - self.distances[edge]) / (self.distances[edge + 1] - self.distances[edge]);
        std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
    }
    fn slice(&self, start: f64, end: f64) -> Vec<Point> {
        let mut out = vec![self.at(start)];
        let first = self.distances.partition_point(|v| *v <= start);
        for i in first..self.distances.len() {
            if self.distances[i] >= end {
                break;
            }
            out.push(self.points[i % self.points.len()]);
        }
        out.push(self.at(end));
        out.dedup();
        out
    }
    /// Closed outlines fit a whole number of periods, uniformly scaling dash
    /// and gap together. The seam lies in the middle of a full gap. Open paths
    /// retain exact requested lengths, with only the final dash clipped.
    pub fn dashes(&self, dash: f64, gap: f64) -> Vec<Vec<Point>> {
        let length = self.length();
        if length <= 0. || !dash.is_finite() || !gap.is_finite() || dash <= 0. || gap <= 0. {
            return Vec::new();
        }
        let (count, dash, gap, start) = if self.closed {
            let count = (length / (dash + gap)).round().clamp(1., MAX_MARKS as f64) as usize;
            let factor = length / (count as f64 * (dash + gap));
            (count, dash * factor, gap * factor, gap * factor * 0.5)
        } else {
            // Bound geometry for enormous paths without allocating per-frame
            // unbounded dash arrays. Semantic requested lengths stay unchanged.
            let factor = (length / ((dash + gap) * MAX_MARKS as f64)).max(1.);
            let dash = dash * factor;
            let gap = gap * factor;
            (
                ((length / (dash + gap)).ceil() as usize).min(MAX_MARKS),
                dash,
                gap,
                0.,
            )
        };
        (0..count)
            .filter_map(|i| {
                let from = start + i as f64 * (dash + gap);
                let to = (from + dash).min(length);
                (to > from).then(|| self.slice(from, to))
            })
            .collect()
    }
    /// Uniform arclength centers. Reduce count globally if corner chords or
    /// opposite sides would overlap round dots; never restart at a vertex.
    pub fn dots(&self, spacing: f64, diameter: f64) -> Vec<Point> {
        let length = self.length();
        if length <= 0.
            || !spacing.is_finite()
            || !diameter.is_finite()
            || spacing <= 0.
            || diameter <= 0.
        {
            return Vec::new();
        }
        let spacing = spacing.max(diameter);
        let mut count = if self.closed {
            (length / spacing).floor().max(1.) as usize
        } else {
            (length / spacing).floor() as usize + 1
        }
        .min(MAX_MARKS);
        let initial_count = count;
        loop {
            let step = if self.closed {
                length / count as f64
            } else if count > 1 {
                if count < initial_count {
                    length / (count - 1) as f64
                } else {
                    spacing.max(length / ((MAX_MARKS - 1) as f64))
                }
            } else {
                0.
            };
            let dots: Vec<_> = (0..count)
                .map(|i| {
                    self.at(if self.closed {
                        (i as f64 + 0.5) * step
                    } else {
                        i as f64 * step
                    })
                })
                .collect();
            if count <= 1 || separated(&dots, diameter) {
                return dots;
            }
            count = ((count as f64 * 0.8).floor() as usize).max(1);
        }
    }
}
fn separated(points: &[Point], diameter: f64) -> bool {
    use std::collections::HashMap;
    let origin = points[0];
    let mut cells: HashMap<(i64, i64), Vec<Point>> = HashMap::new();
    for &p in points {
        let cell = (
            ((p[0] - origin[0]) / diameter).floor() as i64,
            ((p[1] - origin[1]) / diameter).floor() as i64,
        );
        for dx in -1..=1 {
            for dy in -1..=1 {
                if cells
                    .get(&(cell.0.saturating_add(dx), cell.1.saturating_add(dy)))
                    .is_some_and(|v| {
                        v.iter()
                            .any(|q| (p[0] - q[0]).hypot(p[1] - q[1]) < diameter * (1. - 1e-9))
                    })
                {
                    return false;
                }
            }
        }
        cells.entry(cell).or_default().push(p);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn len(p: &[Point]) -> f64 {
        p.windows(2)
            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            .sum()
    }
    #[test]
    fn dash_carries_across_vertices_as_joined_path_and_preserves_open_lengths() {
        let p = MeasuredPath::new(&[[0., 0.], [10., 0.], [10., 20.]], false);
        let d = p.dashes(12., 6.);
        assert_eq!(d[0], vec![[0., 0.], [10., 0.], [10., 2.]]);
        assert_eq!(d[1], vec![[10., 8.], [10., 20.]]);
        assert_eq!(len(&d[0]), 12.);
        assert_eq!(len(&d[1]), 12.);
    }
    #[test]
    fn closed_dash_fit_has_equal_marks_and_full_seam_gap() {
        let p = MeasuredPath::new(
            &[[0., 0.], [37., 0.], [37., 29.], [0., 29.], [0., 0.]],
            true,
        );
        let ds = p.dashes(12., 6.);
        let expected = p.length() / ds.len() as f64 * 2. / 3.;
        assert!(ds.iter().all(|d| (len(d) - expected).abs() < 1e-9));
        let gap = p.length() / ds.len() as f64 / 3.;
        for (actual, expected) in [
            (ds[0][0], p.at(gap * 0.5)),
            (
                *ds.last().unwrap().last().unwrap(),
                p.at(p.length() - gap * 0.5),
            ),
        ] {
            assert!((actual[0] - expected[0]).hypot(actual[1] - expected[1]) < 1e-9);
        }
        assert!(ds.iter().any(|d| d.len() > 2));
    }
    #[test]
    fn dots_are_even_nonoverlapping_on_curves_and_acute_boxes() {
        for points in [
            vec![[0., 0.], [80., 0.], [80., 2.], [0., 2.]],
            (0..128)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::TAU / 128.;
                    [40. * a.cos(), 20. * a.sin()]
                })
                .collect(),
        ] {
            let path = MeasuredPath::new(&points, true);
            let dots = path.dots(5., 3.);
            assert!(separated(&dots, 3.));
            let step = path.length() / dots.len() as f64;
            for (i, dot) in dots.iter().enumerate() {
                assert_eq!(*dot, path.at((i as f64 + 0.5) * step));
            }
        }
    }
    #[test]
    fn duplicate_vertices_degenerate_paths_and_density_are_bounded() {
        let p = MeasuredPath::new(&[[0., 0.], [0., 0.], [10., 0.], [10., 0.], [0., 0.]], true);
        assert_eq!(p.points.len(), 2);
        assert_eq!(p.length(), 20.);
        assert!(MeasuredPath::new(&[], true).dashes(1., 1.).is_empty());
        let huge = MeasuredPath::new(&[[0., 0.], [1e9, 0.]], false);
        assert!(huge.dashes(0.25, 0.25).len() <= MAX_MARKS);
        assert!(huge.dots(0.25, 0.25).len() <= MAX_MARKS);
    }
}
