//! Analytic capsule clipping of the rendered vector centerline. No UI or IO.
use crate::{
    drawing::{self, InkPoint, Smoothing, Stroke, Surface},
    eraser_index::EraserIndex,
};

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn mix(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
fn slab(origin: f64, delta: f64, min: f64, max: f64, range: &mut (f64, f64)) -> bool {
    if delta == 0.0 {
        return (min..=max).contains(&origin);
    }
    let a = (min - origin) / delta;
    let b = (max - origin) / delta;
    range.0 = range.0.max(a.min(b));
    range.1 = range.1.min(a.max(b));
    range.0 <= range.1
}
fn circle(
    a: [f64; 2],
    unit: [f64; 2],
    length: f64,
    center: [f64; 2],
    radius: f64,
) -> Option<(f64, f64)> {
    let offset = sub(center, a);
    let along = dot(offset, unit);
    let perpendicular = (offset[0] * unit[1] - offset[1] * unit[0]).abs();
    if perpendicular > radius {
        return None;
    }
    let half = ((radius - perpendicular) * (radius + perpendicular)).sqrt();
    let range = (
        ((along - half) / length).max(0.0),
        ((along + half) / length).min(1.0),
    );
    (range.0 <= range.1).then_some(range)
}
/// Parameter interval along a→b inside the swept disk (capsule). The caller
/// includes half the ink width in radius so new round caps meet the cursor edge.
pub fn clip_segment(
    a: [f64; 2],
    b: [f64; 2],
    from: [f64; 2],
    to: [f64; 2],
    radius: f64,
) -> Option<(f64, f64)> {
    let delta = sub(b, a);
    let length = delta[0].hypot(delta[1]);
    if length == 0.0 {
        return drawing::hit_path(&[a], from, to, radius).then_some((0.0, 1.0));
    }
    let unit = delta.map(|v| v / length);
    let sweep = sub(to, from);
    let sweep_length = sweep[0].hypot(sweep[1]);
    let mut rectangle = None;
    if sweep_length > 0.0 {
        let along = sweep.map(|v| v / sweep_length);
        let normal = [-along[1], along[0]];
        let offset = sub(a, from);
        let mut range = (0.0, 1.0);
        if slab(
            dot(offset, along),
            dot(delta, along),
            0.0,
            sweep_length,
            &mut range,
        ) && slab(
            dot(offset, normal),
            dot(delta, normal),
            -radius,
            radius,
            &mut range,
        ) {
            rectangle = Some(range);
        }
    }
    // Capsule = two disks + the oriented rectangle between their centers. Its
    // intersection with a line is convex, so the union is one interval.
    let mut result: Option<(f64, f64)> = None;
    for part in [
        circle(a, unit, length, from, radius),
        circle(a, unit, length, to, radius),
        rectangle,
    ]
    .into_iter()
    .flatten()
    {
        result = Some(result.map_or(part, |old| (old.0.min(part.0), old.1.max(part.1))));
    }
    result
}

#[derive(Default)]
pub struct Cuts(pub Vec<(f64, f64)>);
impl Cuts {
    fn insert(&mut self, range: (f64, f64)) -> bool {
        // A tangent point has no length: do not manufacture invisible fragments.
        if range.1 <= range.0 {
            return false;
        }
        let first = self.0.partition_point(|r| r.1.next_up() < range.0);
        let mut end = first;
        let mut merged = range;
        while end < self.0.len() && self.0[end].0 <= merged.1.next_up() {
            merged = (merged.0.min(self.0[end].0), merged.1.max(self.0[end].1));
            end += 1;
        }
        if end == first + 1 && self.0[first] == merged {
            return false;
        }
        if first == end {
            self.0.insert(first, merged);
        } else {
            self.0[first] = merged;
            self.0.drain(first + 1..end);
        }
        true
    }
    pub fn covers(&self, end: f64) -> bool {
        self.0.first().is_some_and(|r| r.0 <= 0.0 && r.1 >= end)
    }
    pub fn touches(&self, start: f64, end: f64) -> bool {
        let i = self.0.partition_point(|r| r.1 < start);
        self.0.get(i).is_some_and(|r| r.0 <= end)
    }
    pub fn connected_before(&self, point: f64) -> bool {
        let i = self.0.partition_point(|r| r.1 < point);
        !self.0.get(i).is_some_and(|r| r.0 < point && r.1 >= point)
    }
    pub fn visit_visible(&self, start: f64, end: f64, mut visit: impl FnMut(f64, f64)) {
        let mut cursor = start;
        let first = self.0.partition_point(|r| r.1 < start);
        for &(lo, hi) in &self.0[first..] {
            if lo > end {
                break;
            }
            if lo > cursor {
                visit(cursor, lo.min(end));
            }
            cursor = cursor.max(hi);
            if cursor >= end {
                return;
            }
        }
        if cursor < end {
            visit(cursor, end);
        }
    }
}

pub struct PartialCut {
    pub cuts: Cuts,
    index: EraserIndex,
    width: f64,
}
impl PartialCut {
    pub fn new(path: &[[f64; 2]], width: f64, surface: Surface) -> Self {
        Self {
            cuts: Cuts::default(),
            width,
            index: EraserIndex::for_path(path, width, surface),
        }
    }
    pub fn sweep(
        &mut self,
        path: &[[f64; 2]],
        surface: Surface,
        from: [f64; 2],
        to: [f64; 2],
        radius: f64,
        mut changed: impl FnMut(f64, f64),
    ) -> bool {
        if self.cuts.covers(end(path)) {
            return false;
        }
        let mut any = false;
        self.index.query(surface, from, to, radius, |index| {
            for index in index
                ..(index + crate::eraser_index::SEGMENTS_PER_ENTRY)
                    .min(path.len().saturating_sub(1).max(1))
            {
                let a = path[index];
                let b = path[(index + 1).min(path.len() - 1)];
                if let Some((lo, hi)) = clip_segment(a, b, from, to, radius + self.width * 0.5) {
                    let range = (index as f64 + lo, index as f64 + hi);
                    if self.cuts.insert(range) {
                        any = true;
                        changed(range.0, range.1);
                    }
                }
            }
        });
        any
    }
}

pub fn end(path: &[[f64; 2]]) -> f64 {
    path.len().saturating_sub(1).max(1) as f64
}
pub fn position(path: &[[f64; 2]], parameter: f64) -> [f64; 2] {
    let i = (parameter.floor() as usize).min(path.len() - 1);
    mix(
        path[i],
        path[(i + 1).min(path.len() - 1)],
        parameter - i as f64,
    )
}
/// Emit a continuous retained polyline, preserving every unaffected path vertex.
pub fn visit_piece(path: &[[f64; 2]], start: f64, end: f64, mut point: impl FnMut([f64; 2], f64)) {
    point(position(path, start), start);
    let first = start.floor() as usize + 1;
    let last = (end.ceil() as usize).min(path.len());
    for (i, p) in path.iter().enumerate().take(last).skip(first) {
        point(*p, i as f64);
    }
    let last_position = position(path, end);
    if last_position != position(path, start) || end > start {
        point(last_position, end);
    }
}

/// Interpolate existing pressure along normalized raw/rendered arc length. M1
/// still renders constant width; no hardware pressure support is introduced.
fn path_pressures(stroke: &Stroke, path: &[[f64; 2]]) -> Option<Vec<f32>> {
    // Mouse ink has uniform pressure; do not allocate arc-length/pressure maps.
    if stroke
        .points
        .iter()
        .all(|p| p.pressure == stroke.points[0].pressure)
    {
        return None;
    }
    let cumulative = |points: &[[f64; 2]]| {
        let mut distances = Vec::with_capacity(points.len());
        let mut sum = 0.0;
        distances.push(sum);
        for pair in points.windows(2) {
            let d = sub(pair[1], pair[0]);
            sum += d[0].hypot(d[1]);
            distances.push(sum);
        }
        distances
    };
    let raw = cumulative(&stroke.points.iter().map(|p| p.position).collect::<Vec<_>>());
    let rendered = cumulative(path);
    let mut i = 0;
    Some(
        rendered
            .iter()
            .map(|distance| {
                let target = if *rendered.last().unwrap() > 0.0 {
                    distance / rendered.last().unwrap() * raw.last().unwrap()
                } else {
                    0.0
                };
                while i + 1 < raw.len() && raw[i + 1] < target {
                    i += 1;
                }
                let next = (i + 1).min(raw.len() - 1);
                let t = if raw[next] > raw[i] {
                    ((target - raw[i]) / (raw[next] - raw[i])) as f32
                } else {
                    0.0
                };
                stroke.points[i].pressure
                    + t * (stroke.points[next].pressure - stroke.points[i].pressure)
            })
            .collect(),
    )
}

pub fn fragments(stroke: &Stroke, path: &[[f64; 2]], cuts: &Cuts) -> Vec<Stroke> {
    if cuts.covers(end(path)) {
        return Vec::new();
    }
    let pressures = path_pressures(stroke, path);
    let mut pieces = Vec::new();
    cuts.visit_visible(0.0, end(path), |start, end| {
        let mut piece = Stroke::new(
            stroke.document_id,
            stroke.surface,
            stroke.color,
            stroke.width,
        );
        piece.smoothing = Smoothing::PolylineV1;
        visit_piece(path, start, end, |position, t| {
            let pressure = if let Some(pressures) = &pressures {
                let i = (t.floor() as usize).min(pressures.len() - 1);
                let next = (i + 1).min(pressures.len() - 1);
                pressures[i] + (t - i as f64) as f32 * (pressures[next] - pressures[i])
            } else {
                stroke.points[0].pressure
            };
            if piece.points.last().is_none_or(|p| p.position != position) {
                piece.points.push(InkPoint { position, pressure });
            }
        });
        if !piece.points.is_empty() {
            pieces.push(piece);
        }
    });
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{DocumentData, DocumentKind};
    fn line(width: f64) -> Stroke {
        let data = DocumentData::blank(DocumentKind::Whiteboard);
        let mut stroke = Stroke::new(data.id(), Surface::Canvas, [20, 60, 180, 255], width);
        stroke.smoothing = Smoothing::PolylineV1;
        stroke.points = vec![
            InkPoint {
                position: [0.0, 0.0],
                pressure: 0.2,
            },
            InkPoint {
                position: [100.0, 0.0],
                pressure: 0.8,
            },
        ];
        stroke
    }
    fn cut(stroke: &Stroke, from: [f64; 2], to: [f64; 2], radius: f64) -> Vec<Stroke> {
        let path = stroke.path();
        let mut partial = PartialCut::new(&path, stroke.width, stroke.surface);
        assert!(partial.sweep(&path, stroke.surface, from, to, radius, |_, _| {}));
        fragments(stroke, &path, &partial.cuts)
    }
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-8, "{a} != {b}");
    }
    #[test]
    fn capsule_intersections_include_disks_strip_and_stationary_sweeps() {
        for (from, to, radius, expected) in [
            ([50.0, -20.0], [50.0, 20.0], 10.0, (0.4, 0.6)),
            ([40.0, 3.0], [60.0, 3.0], 5.0, (0.36, 0.64)),
            ([50.0, 0.0], [50.0, 0.0], 10.0, (0.4, 0.6)),
            ([50.0, -20.0], [50.0, -20.0], 20.0, (0.5, 0.5)),
        ] {
            let range = clip_segment([0.0, 0.0], [100.0, 0.0], from, to, radius).unwrap();
            close(range.0, expected.0);
            close(range.1, expected.1);
        }
        assert!(clip_segment([0.0, 0.0], [100.0, 0.0], [0.0, 30.0], [100.0, 30.0], 10.0).is_none());
    }
    #[test]
    fn middle_endpoints_thickness_dots_and_pressure() {
        for width in [0.25, 2.0, 24.0] {
            let source = line(width);
            let pieces = cut(&source, [50.0, -20.0], [50.0, 20.0], 10.0);
            assert_eq!(pieces.len(), 2);
            close(
                pieces[0].points.last().unwrap().position[0],
                40.0 - width * 0.5,
            );
            close(pieces[1].points[0].position[0], 60.0 + width * 0.5);
            for piece in &pieces {
                assert_ne!(piece.id, source.id);
                assert_eq!(piece.document_id, source.document_id);
                assert_eq!(piece.color, source.color);
                assert_eq!(piece.width, source.width);
                assert_eq!(piece.smoothing, Smoothing::PolylineV1);
                assert_eq!(
                    serde_json::from_slice::<Stroke>(&serde_json::to_vec(piece).unwrap()).unwrap(),
                    *piece
                );
                assert!(
                    piece
                        .points
                        .iter()
                        .all(|p| (0.2..=0.8).contains(&p.pressure))
                );
            }
            assert_ne!(pieces[0].id, pieces[1].id);
            let at_start = cut(&source, [0.0, 0.0], [0.0, 0.0], 5.0);
            assert_eq!(at_start.len(), 1);
            close(at_start[0].points[0].position[0], 5.0 + width * 0.5);
            let at_end = cut(&source, [100.0, 0.0], [100.0, 0.0], 5.0);
            assert_eq!(at_end.len(), 1);
            close(
                at_end[0].points.last().unwrap().position[0],
                95.0 - width * 0.5,
            );
            assert!(cut(&source, [-10.0, 0.0], [110.0, 0.0], 10.0).is_empty());
        }
        let mut dot = line(2.0);
        dot.points.truncate(1);
        assert!(cut(&dot, [0.0, 0.0], [0.0, 0.0], 4.0).is_empty());
    }
    #[test]
    fn repeated_sweeps_merge_cuts_without_fragment_explosion_or_tangent_splits() {
        let source = line(2.0);
        let path = source.path();
        let mut partial = PartialCut::new(&path, source.width, source.surface);
        for i in 0..1000 {
            let x = 30.0 + i as f64 * 0.04;
            partial.sweep(&path, source.surface, [x, 0.0], [x, 0.0], 3.0, |_, _| {});
        }
        assert_eq!(partial.cuts.0.len(), 1);
        assert_eq!(fragments(&source, &path, &partial.cuts).len(), 2);
        assert!(!partial.sweep(
            &path,
            source.surface,
            [50.0, 0.0],
            [50.0, 0.0],
            3.0,
            |_, _| panic!()
        ));
        let mut tangent = PartialCut::new(&path, 2.0, source.surface);
        assert!(!tangent.sweep(
            &path,
            source.surface,
            [50.0, 5.0],
            [50.0, 5.0],
            4.0,
            |_, _| panic!()
        ));
        assert!(tangent.cuts.0.is_empty());
        // Tiny intervals on a very long segment must not be treated as tangency.
        let long = [[0.0, 0.0], [1e12, 0.0]];
        let mut partial = PartialCut::new(&long, 2.0, source.surface);
        assert!(partial.sweep(
            &long,
            source.surface,
            [5e11, 0.0],
            [5e11, 0.0],
            4.0,
            |_, _| {}
        ));
    }
    #[test]
    fn smoothed_survivors_retain_exact_unaffected_centerline_vertices() {
        for smoothing in [
            Smoothing::Legacy,
            Smoothing::AdaptiveV1 { strength: 0 },
            Smoothing::AdaptiveV1 { strength: 50 },
            Smoothing::AdaptiveV1 { strength: 100 },
        ] {
            let mut source = line(4.0);
            source.points = [
                [0.0, 20.0],
                [25.0, 25.0],
                [50.0, 15.0],
                [75.0, 25.0],
                [100.0, 20.0],
            ]
            .map(InkPoint::mouse)
            .to_vec();
            source.smoothing = smoothing;
            let original = source.path();
            let pieces = cut(&source, [50.0, 0.0], [50.0, 40.0], 5.0);
            assert_eq!(pieces.len(), 2);
            let surviving: Vec<_> = pieces.iter().flat_map(Stroke::path).collect();
            for point in original.iter().filter(|p| (p[0] - 50.0).abs() > 7.0) {
                assert!(
                    surviving.contains(point),
                    "{smoothing:?}: missing {point:?}"
                );
            }
            assert_eq!(source.path(), original);
            // Explicit paths remain stable on the next partial cut too.
            let more = cut(&pieces[0], [10.0, 0.0], [10.0, 40.0], 2.0);
            assert_eq!(more.len(), 2);
        }
    }
    #[test]
    fn analytic_clipping_matches_independent_distance_samples_and_large_origins() {
        let mut seed = 17u64;
        let mut random = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 10000) as f64 / 100.0 - 50.0
        };
        for _ in 0..500 {
            let a = [random(), random()];
            let b = [random(), random()];
            let from = [random(), random()];
            let to = [random(), random()];
            let radius = random().abs() + 0.25;
            let interval = clip_segment(a, b, from, to, radius);
            for i in 0..41 {
                let t = i as f64 / 40.0;
                let actual = drawing::hit_path(&[mix(a, b, t)], from, to, radius);
                assert_eq!(actual, interval.is_some_and(|(lo, hi)| t >= lo && t <= hi));
            }
        }
        let range = clip_segment(
            [1e12, -1e12],
            [1e12 + 100.0, -1e12],
            [1e12 + 50.0, -1e12 - 20.0],
            [1e12 + 50.0, -1e12 + 20.0],
            10.0,
        )
        .unwrap();
        close(range.0, 0.4);
        close(range.1, 0.6);
    }
    #[test]
    fn long_stroke_index_prunes_segments() {
        let path: Vec<_> = (0..100_001)
            .map(|i| [i as f64, (i as f64 * 0.01).sin()])
            .collect();
        let partial = PartialCut::new(&path, 2.0, Surface::Canvas);
        let point = path[50_000];
        let mut candidates = 0;
        partial
            .index
            .query(Surface::Canvas, point, point, 1.0, |_| candidates += 1);
        assert!(candidates <= 2, "{candidates} path blocks");
        assert!(candidates * crate::eraser_index::SEGMENTS_PER_ENTRY <= 32);
    }
    #[test]
    #[ignore = "manual release-mode partial-erasing benchmark"]
    fn partial_geometry_performance() {
        for count in [100, 100_000] {
            let path: Vec<_> = (0..=count)
                .map(|i| [i as f64, (i as f64 * 0.01).sin()])
                .collect();
            let start = std::time::Instant::now();
            let mut partial = PartialCut::new(&path, 2.0, Surface::Canvas);
            eprintln!("{count} segments, index build: {:?}", start.elapsed());
            let start = std::time::Instant::now();
            for i in 0..2000 {
                let x = i as f64 * (count as f64 / 4000.0);
                partial.sweep(&path, Surface::Canvas, [x, 0.0], [x, 0.0], 4.0, |_, _| {});
            }
            eprintln!(
                "{count} segments, 2000 samples: {:?}, {} merged cuts",
                start.elapsed(),
                partial.cuts.0.len()
            );
        }
    }
}
