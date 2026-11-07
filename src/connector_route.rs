//! Bounded local routing. Only endpoint obstacles are considered; no global scan.
use crate::{
    connector::{Connector, Endpoint, Port, Rail, Route},
    object::{ObjectId, Point},
    shape::Shape,
};
fn add(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dist(a: Point, b: Point) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn unit(a: Point) -> Point {
    let l = a[0].hypot(a[1]);
    if l < 1e-8 { [1., 0.] } else { a.map(|v| v / l) }
}
impl Endpoint {
    pub fn automatic(&self) -> bool {
        matches!(
            self,
            Self::Attached {
                automatic: true,
                ..
            }
        )
    }
    pub fn set_automatic(&mut self, enabled: bool) {
        if let Self::Attached { automatic, .. } = self {
            *automatic = enabled;
        }
    }
    pub fn resolve(&mut self, shape: &Shape, other: Point) {
        if let Self::Attached {
            anchor,
            last_position,
            automatic,
            normal,
            bounds,
            ..
        } = self
        {
            if *automatic {
                let score = |port: Port| {
                    let p = port.point(shape).unwrap();
                    let v = unit(sub(other, p));
                    let n = port.normal(shape);
                    dist(p, other) + 32. * (1. - v[0] * n[0] - v[1] * n[1])
                };
                let best = Port::ALL
                    .into_iter()
                    .min_by(|a, b| score(*a).total_cmp(&score(*b)))
                    .unwrap();
                // Absolute and proportional dead bands preserve the last chosen local port.
                if score(best) + 20. + (score(best) * 0.12).min(20.) < score(*anchor) {
                    *anchor = best;
                }
            }
            *last_position = anchor.point(shape).unwrap();
            *normal = anchor.normal(shape);
            let b = shape.geometry_bounds();
            *bounds = [b.min, b.max];
        }
    }
    fn direction(&self, other: Point) -> Point {
        match self {
            Self::Attached { normal, .. } if normal[0].hypot(normal[1]) > 0.1 => *normal,
            _ => unit(sub(other, self.position())),
        }
    }
    fn exit(&self, other: Point) -> Point {
        let p = self.position();
        let n = self.direction(other);
        let axis = usize::from(n[1].abs() > n[0].abs());
        let sign = if n[axis] < 0. { -1. } else { 1. };
        let mut q = p;
        q[axis] = match self {
            Self::Attached { bounds, .. } => {
                if sign > 0. {
                    bounds[1][axis] + 16.
                } else {
                    bounds[0][axis] - 16.
                }
            }
            _ => p[axis],
        };
        q
    }
}
impl Connector {
    pub fn resolve_routes<'a>(&mut self, lookup: impl Fn(ObjectId) -> Option<&'a Shape>) {
        // Choose against target centers, avoiding sequential anchor feedback.
        let centers = self.ends().map(|e| {
            e.target()
                .and_then(&lookup)
                .map(|s| s.geometry_bounds().center())
                .unwrap_or(e.position())
        });
        for (i, e) in self.ends_mut().into_iter().enumerate() {
            if let Some(s) = e.target().and_then(&lookup) {
                e.resolve(s, centers[1 - i]);
            }
        }
    }
    pub fn route_valid(&self) -> bool {
        let valid = |p: &Point| p.iter().all(|v| v.is_finite() && v.abs() <= 1e15);
        self.ends().iter().all(|e| match e {
            Endpoint::Attached { normal, bounds, .. } => valid(normal) && bounds.iter().all(valid),
            _ => true,
        }) && match self.route {
            Route::Straight => true,
            Route::Orthogonal { rail } => {
                rail.is_none_or(|r| r.axis < 2 && r.offset.is_finite() && r.offset.abs() <= 1e9)
            }
            Route::Curved { controls } => controls.is_none_or(|p| p.iter().all(valid)),
        }
    }
    /// Control offsets are relative to each endpoint, so resizing a target never erases edits.
    pub fn controls(&self) -> [Point; 2] {
        let a = self.source.position();
        let b = self.target.position();
        if let Route::Curved { controls: Some(c) } = self.route {
            return [add(a, c[0]), add(b, c[1])];
        }
        let length = (dist(a, b) * 0.4).clamp(24., 240.);
        [
            add(a, self.source.direction(b).map(|v| v * length)),
            add(b, self.target.direction(a).map(|v| v * length)),
        ]
    }
    pub fn tangents(&self, points: &[Point]) -> [Point; 2] {
        let a = self.source.position();
        let b = self.target.position();
        if matches!(self.route, Route::Curved { .. }) {
            let c = self.controls();
            let start = if dist(c[0], a) > 1e-8 {
                sub(c[0], a)
            } else if dist(c[1], a) > 1e-8 {
                sub(c[1], a)
            } else {
                sub(b, a)
            };
            let end = if dist(b, c[1]) > 1e-8 {
                sub(b, c[1])
            } else if dist(b, c[0]) > 1e-8 {
                sub(b, c[0])
            } else {
                sub(b, a)
            };
            return [unit(start), unit(end)];
        }
        [
            unit(sub(points[1], points[0])),
            unit(sub(*points.last().unwrap(), points[points.len() - 2])),
        ]
    }
    pub fn path(&self, tolerance: f64) -> Vec<Point> {
        let a = self.source.position();
        let b = self.target.position();
        match self.route {
            Route::Straight => vec![a, b],
            Route::Curved { .. } => {
                let c = self.controls();
                let mut out = vec![a];
                flatten([a, c[0], c[1], b], tolerance.clamp(0.001, 1.), 0, &mut out);
                out.dedup();
                if out.len() == 1 {
                    out.push(b);
                }
                out
            }
            Route::Orthogonal { rail } => self.orthogonal(rail),
        }
    }
    fn orthogonal(&self, rail: Option<Rail>) -> Vec<Point> {
        let a = self.source.position();
        let b = self.target.position();
        let direction = unit(sub(b, a));
        let outward = self.source.direction(b);
        let inward = self.target.direction(a);
        if rail.is_none()
            && ((a[0] - b[0]).abs() < 1e-8 || (a[1] - b[1]).abs() < 1e-8)
            && outward[0] * direction[0] + outward[1] * direction[1] > 0.5
            && inward[0] * direction[0] + inward[1] * direction[1] < -0.5
        {
            return vec![a, b];
        }
        let s = self.source.exit(b);
        let t = self.target.exit(a);
        if let Some(r) = rail {
            let m = (a[r.axis] + b[r.axis]) * 0.5 + r.offset;
            let mut u = s;
            let mut v = t;
            u[r.axis] = m;
            v[r.axis] = m;
            return clean(vec![a, s, u, v, t, b]);
        }
        let mut xs = vec![s[0], t[0], (s[0] + t[0]) * 0.5];
        let mut ys = vec![s[1], t[1], (s[1] + t[1]) * 0.5];
        let obstacles: Vec<_> = self
            .ends()
            .iter()
            .filter_map(|e| match e {
                Endpoint::Attached { bounds, .. } => Some(*bounds),
                _ => None,
            })
            .collect();
        for bounds in &obstacles {
            xs.extend([bounds[0][0] - 16., bounds[1][0] + 16.]);
            ys.extend([bounds[0][1] - 16., bounds[1][1] + 16.]);
        }
        let clear = |p: Point, q: Point| {
            !obstacles.iter().any(|r| {
                if (p[0] - q[0]).abs() < 1e-7 {
                    p[0] > r[0][0] + 1e-7
                        && p[0] < r[1][0] - 1e-7
                        && p[1].max(q[1]) > r[0][1] + 1e-7
                        && p[1].min(q[1]) < r[1][1] - 1e-7
                } else {
                    p[1] > r[0][1] + 1e-7
                        && p[1] < r[1][1] - 1e-7
                        && p[0].max(q[0]) > r[0][0] + 1e-7
                        && p[0].min(q[0]) < r[1][0] - 1e-7
                }
            })
        };
        let mut candidates = vec![
            vec![
                a,
                s,
                [(s[0] + t[0]) * 0.5, s[1]],
                [(s[0] + t[0]) * 0.5, t[1]],
                t,
                b,
            ],
            vec![
                a,
                s,
                [s[0], (s[1] + t[1]) * 0.5],
                [t[0], (s[1] + t[1]) * 0.5],
                t,
                b,
            ],
            vec![a, s, [t[0], s[1]], t, b],
            vec![a, s, [s[0], t[1]], t, b],
        ];
        for x in xs {
            candidates.push(vec![a, s, [x, s[1]], [x, t[1]], t, b]);
        }
        for y in ys {
            candidates.push(vec![a, s, [s[0], y], [t[0], y], t, b]);
        }
        // Endpoint exit stubs are allowed within their own conservative AABB.
        // All corridor edges must avoid both target interiors. Overlap can make
        // clearance impossible: retain deterministic least-penalty geometry.
        candidates
            .into_iter()
            .map(clean)
            .min_by(|p, q| {
                let cost = |p: &Vec<Point>| {
                    let mut cost =
                        p.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>() + p.len() as f64 * 18.;
                    for w in p.windows(2) {
                        if w[0] != a && w[1] != b && !clear(w[0], w[1]) {
                            cost += 1e8;
                        }
                    }
                    for w in p.windows(3) {
                        let u = sub(w[1], w[0]);
                        let v = sub(w[2], w[1]);
                        if u[0] * v[0] + u[1] * v[1] < -1e-8 {
                            cost += 1e9;
                        }
                    }
                    cost
                };
                cost(p).total_cmp(&cost(q))
            })
            .unwrap_or(vec![a, b])
    }
    pub fn route_handles(&self) -> Vec<Point> {
        match self.route {
            Route::Straight => Vec::new(),
            Route::Curved { .. } => self.controls().to_vec(),
            Route::Orthogonal { .. } => {
                let p = self.path(0.01);
                p.windows(2)
                    .enumerate()
                    .filter(|(i, w)| {
                        (p.len() <= 3 || (*i > 0 && *i + 2 < p.len())) && dist(w[0], w[1]) > 16.
                    })
                    .map(|(_, w)| [(w[0][0] + w[1][0]) * 0.5, (w[0][1] + w[1][1]) * 0.5])
                    .collect()
            }
        }
    }
    pub fn edit_route(&mut self, index: usize, point: Point) {
        match self.route {
            Route::Straight => (),
            Route::Curved { .. } => {
                let mut c = self.controls();
                if index < 2 {
                    c[index] = point;
                    self.route = Route::Curved {
                        controls: Some([
                            sub(c[0], self.source.position()),
                            sub(c[1], self.target.position()),
                        ]),
                    };
                }
            }
            Route::Orthogonal { .. } => {
                let handles = self.route_handles();
                if let Some(p) = handles.get(index) {
                    let path = self.path(0.01);
                    let segment = path
                        .windows(2)
                        .find(|w| {
                            dist([(w[0][0] + w[1][0]) * 0.5, (w[0][1] + w[1][1]) * 0.5], *p) < 1e-6
                        })
                        .unwrap();
                    let axis = usize::from((segment[0][0] - segment[1][0]).abs() > 1e-6);
                    self.route = Route::Orthogonal {
                        rail: Some(Rail {
                            axis,
                            offset: point[axis]
                                - (self.source.position()[axis] + self.target.position()[axis])
                                    * 0.5,
                        }),
                    };
                }
            }
        }
    }
}
fn clean(points: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::new();
    for p in points {
        if out.last() == Some(&p) {
            continue;
        }
        while out.len() > 1 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            if (((a[0] - b[0]).abs() < 1e-7 && (b[0] - p[0]).abs() < 1e-7)
                || ((a[1] - b[1]).abs() < 1e-7 && (b[1] - p[1]).abs() < 1e-7))
                && (b[0] - a[0]) * (p[0] - b[0]) + (b[1] - a[1]) * (p[1] - b[1]) >= 0.
            {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    if out.len() == 1 {
        out.push(out[0]);
    }
    out
}
/// Adaptive de Casteljau subdivision, <= 0.01 document point chord error for
/// normal rendering (0.04 screen points at 400%). Bounded at 16k chords.
fn flatten(p: [Point; 4], tol: f64, depth: u8, out: &mut Vec<Point>) {
    let chord = sub(p[3], p[0]);
    let len = chord[0].hypot(chord[1]);
    let error = if len < 1e-9 {
        dist(p[0], p[1]).max(dist(p[0], p[2]))
    } else {
        p[1..3]
            .iter()
            .map(|q| {
                let v = sub(*q, p[0]);
                (v[0] * chord[1] - v[1] * chord[0]).abs() / len
            })
            .fold(0., f64::max)
    };
    // Polygon excess also detects collinear reversals/cusps.
    let excess = dist(p[0], p[1]) + dist(p[1], p[2]) + dist(p[2], p[3]) - len;
    if depth == 14 || (error <= tol && excess <= tol) {
        out.push(p[3]);
        return;
    }
    let mid = |a: Point, b: Point| std::array::from_fn(|i| (a[i] + b[i]) * 0.5);
    let a = mid(p[0], p[1]);
    let b = mid(p[1], p[2]);
    let c = mid(p[2], p[3]);
    let d = mid(a, b);
    let e = mid(b, c);
    let m = mid(d, e);
    flatten([p[0], a, d, m], tol, depth + 1, out);
    flatten([m, e, c, p[3]], tol, depth + 1, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{DocumentData, DocumentKind},
        drawing::{Drawing, Surface},
        object::{CanvasObject, ObjectSnapshot, Placement},
        object_clipboard::ObjectBundle,
        shape::{BorderStyle, ShapeKind, ShapeStyle},
        stroke_pattern::MeasuredPath,
    };
    use std::collections::HashSet;
    fn fixture(kind: DocumentKind, position: Point) -> (Drawing, Shape, Shape, Connector) {
        let data = DocumentData::blank(kind);
        let mut d = Drawing::new(&data);
        let surface = if kind == DocumentKind::Page {
            Surface::Page(0)
        } else {
            Surface::Canvas
        };
        let a = Shape::create(
            d.owner(),
            surface,
            ShapeKind::Rectangle,
            [40., 80.],
            [120., 140.],
            false,
        )
        .unwrap();
        let b = Shape::create(
            d.owner(),
            surface,
            ShapeKind::Rectangle,
            position,
            add(position, [80., 60.]),
            false,
        )
        .unwrap();
        let c = Connector::new(
            d.owner(),
            surface,
            Endpoint::attached(&a, Port::Right).unwrap(),
            Endpoint::attached(&b, Port::Left).unwrap(),
            ShapeStyle::default(),
        );
        assert!(d.add_shape(a.clone()));
        assert!(d.add_shape(b.clone()));
        assert!(d.add_connector(c.clone()));
        (d, a, b, c)
    }
    fn edge(d: &Drawing, id: ObjectId) -> &Connector {
        let Some(ObjectSnapshot::Connector(c)) = d.snapshot(id) else {
            panic!()
        };
        c
    }
    fn orthogonal(points: &[Point]) {
        for w in points.windows(2) {
            assert!(
                (w[0][0] - w[1][0]).abs() < 1e-7 || (w[0][1] - w[1][1]).abs() < 1e-7,
                "{points:?}"
            );
            assert!(dist(w[0], w[1]) > 1e-7);
        }
    }
    #[test]
    fn horizontal_vertical_diagonal_routes_are_short_orthogonal_and_avoid_endpoint_interiors() {
        for position in [
            [280., 80.],
            [40., 300.],
            [280., 300.],
            [280., -100.],
            [-200., 80.],
        ] {
            let (_, a, b, mut c) = fixture(DocumentKind::Whiteboard, position);
            if position[0] == 40. {
                c.source = Endpoint::attached(&a, Port::Bottom).unwrap();
                c.target = Endpoint::attached(&b, Port::Top).unwrap();
            }
            c.route = Route::Orthogonal { rail: None };
            let path = c.path(0.01);
            orthogonal(&path);
            assert!(path.len() <= 6, "{path:?}");
            if position == [280., 80.] || position == [40., 300.] {
                assert_eq!(path.len(), 2);
            }
            for w in path.windows(2) {
                for t in 1..100 {
                    let p = add(w[0], sub(w[1], w[0]).map(|v| v * t as f64 / 100.));
                    for shape in [&a, &b] {
                        let p = shape.placement.inverse(p);
                        let crate::shape::ShapeGeometry::Box { size, .. } = shape.geometry else {
                            panic!()
                        };
                        assert!(
                            !(p[0] > 0.001
                                && p[0] < size[0] - 0.001
                                && p[1] > 0.001
                                && p[1] < size[1] - 0.001),
                            "{position:?}: {path:?} intersects {p:?}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn automatic_anchors_switch_with_dead_band_and_fixed_ports_never_change() {
        let (_, a, mut b, mut c) = fixture(DocumentKind::Whiteboard, [280., 80.]);
        c.source.set_automatic(true);
        c.target.set_automatic(true);
        c.resolve_routes(|id| if id == a.id { Some(&a) } else { Some(&b) });
        assert!(matches!(
            c.source,
            Endpoint::Attached {
                anchor: Port::Right,
                ..
            }
        ));
        for y in [-0.01, 0.01, -0.02, 0.02, 0.] {
            b.placement.position = [280., 80. + y];
            c.resolve_routes(|id| if id == a.id { Some(&a) } else { Some(&b) });
            assert!(matches!(
                c.source,
                Endpoint::Attached {
                    anchor: Port::Right,
                    ..
                }
            ));
        }
        b.placement.position = [-400., 80.];
        c.resolve_routes(|id| if id == a.id { Some(&a) } else { Some(&b) });
        assert!(matches!(
            c.source,
            Endpoint::Attached {
                anchor: Port::Left,
                ..
            }
        ));
        c.source.set_automatic(false);
        b.placement.position = [8000., 80.];
        c.resolve_routes(|id| if id == a.id { Some(&a) } else { Some(&b) });
        assert!(matches!(
            c.source,
            Endpoint::Attached {
                anchor: Port::Left,
                ..
            }
        ));
    }
    #[test]
    fn curves_are_adaptive_smooth_pickable_and_heads_follow_exact_derivatives() {
        let (_, _, _, mut c) = fixture(DocumentKind::Whiteboard, [280., 300.]);
        c.route = Route::Curved {
            controls: Some([[100., -120.], [-100., 120.]]),
        };
        c.style.start_head = true;
        let points = c.path(0.01);
        assert!(points.len() > 30);
        assert!(points.len() < 16385);
        let controls = c.controls();
        let tangents = c.tangents(&points);
        assert_eq!(tangents[0], unit(sub(controls[0], c.source.position())));
        assert_eq!(tangents[1], unit(sub(c.target.position(), controls[1])));
        let shape = c.as_shape();
        let contours = shape.contours(0.01);
        assert_eq!(contours.len(), 3);
        for path in &contours[1..] {
            assert!(path.iter().flatten().all(|v| v.is_finite()));
            assert!(dist(path[1], path[2]) > 1.);
        }
        for p in points.iter().step_by(5) {
            assert!(c.hit_test(*p, 2.));
        }
        assert!(!c.hit_test([180., 310.], 1.));
        let b = c.geometry_bounds().unwrap();
        assert!(c.as_shape().rectangle_matches(b.expand(4.), false));
        let json = serde_json::to_string(&c).unwrap();
        let copy: Connector = serde_json::from_str(&json).unwrap();
        assert_eq!(c, copy);
        assert_eq!(points, copy.path(0.01));
    }
    #[test]
    fn manual_rails_and_control_offsets_survive_node_transforms_clipboard_and_exact_history() {
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            for route in [
                Route::Orthogonal { rail: None },
                Route::Curved { controls: None },
            ] {
                let (mut d, a, b, c) = fixture(kind, [280., 300.]);
                let id = c.id;
                assert!(d.edit_connectors(&HashSet::from([id]), |c| c.route = route));
                let mut c = edge(&d, id).clone();
                let handle = c.route_handles()[0];
                c.edit_route(0, add(handle, [35., 30.]));
                assert!(d.replace_connector(c));
                let manual = edge(&d, id).route;
                let before = d.items().to_vec();
                let delta = Placement {
                    position: [10., 15.],
                    ..Default::default()
                };
                assert!(d.transform_objects(&HashSet::from([a.id, b.id]), delta));
                assert_eq!(edge(&d, id).route, manual);
                let after = d.items().to_vec();
                d.undo();
                assert_eq!(d.items(), before);
                d.redo();
                assert_eq!(d.items(), after);
                let capture = ObjectBundle::capture_objects(
                    d.items()
                        .iter()
                        .filter(|o| [a.id, b.id, id].contains(&o.id()))
                        .cloned(),
                )
                .unwrap();
                let ids = capture.duplicate(&mut d, a.surface);
                assert_eq!(ids.len(), 3);
                let copy = ids
                    .iter()
                    .find_map(|id| match d.snapshot(*id) {
                        Some(ObjectSnapshot::Connector(c)) => Some(c),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(copy.route, manual);
                assert_ne!(copy.source.target(), Some(a.id));
                assert!(d.edit_connectors(&HashSet::from([id]), |c| c.route = route));
                assert_eq!(edge(&d, id).route, route);
                d.undo();
                assert_eq!(edge(&d, id).route, manual);
            }
        }
    }
    #[test]
    fn route_switching_styles_and_batch_anchor_changes_are_atomic_and_preserve_links() {
        let (mut d, a, b, c) = fixture(DocumentKind::Whiteboard, [280., 300.]);
        let mut c2 = c.clone();
        c2.id = uuid::Uuid::new_v4();
        assert!(d.add_connector(c2.clone()));
        let ids = HashSet::from([c.id, c2.id]);
        let before = d.items().to_vec();
        assert!(d.edit_connectors(&ids, |c| {
            c.route = Route::Curved { controls: None };
            c.source.set_automatic(true);
        }));
        let after = d.items().to_vec();
        d.undo();
        assert_eq!(d.items(), before);
        d.redo();
        assert_eq!(d.items(), after);
        for id in ids {
            let edge = edge(&d, id);
            assert_eq!(edge.source.target(), Some(a.id));
            assert_eq!(edge.target.target(), Some(b.id));
        }
    }
    #[test]
    fn dash_and_dot_measurement_is_global_and_zoom_independent_for_all_modes() {
        let (_, _, _, mut c) = fixture(DocumentKind::Whiteboard, [280., 300.]);
        for route in [
            Route::Straight,
            Route::Orthogonal { rail: None },
            Route::Curved { controls: None },
        ] {
            c.route = route;
            let points = c.path(0.01);
            let path = MeasuredPath::new(&points, false);
            let dashes = path.dashes(12., 6.);
            for dash in dashes.iter().take(dashes.len() - 1) {
                assert!((MeasuredPath::new(dash, false).length() - 12.).abs() < 1e-7);
            }
            let mut counts = None;
            for zoom in [0.5, 1., 2., 4.] {
                c.style.border_style = BorderStyle::Dashed;
                let rendered = crate::shape_border::shapes(
                    &points,
                    false,
                    &c.style,
                    1.,
                    crate::drawing::Transform {
                        scale: zoom,
                        origin: [200., 100.],
                        center: [-100., 70.],
                    },
                    1.,
                    false,
                );
                if let Some(n) = counts {
                    assert_eq!(rendered.len(), n);
                } else {
                    counts = Some(rendered.len());
                }
            }
        }
    }
    #[test]
    fn attached_routes_follow_resize_rotation_flip_and_large_offscreen_positions() {
        for route in [
            Route::Straight,
            Route::Orthogonal { rail: None },
            Route::Curved { controls: None },
        ] {
            let (mut d, a, b, c) = fixture(DocumentKind::Whiteboard, [280., 300.]);
            let ids = HashSet::from([c.id]);
            d.edit_connectors(&ids, |c| c.route = route);
            for delta in [
                Placement {
                    position: [1e7, -1e7],
                    ..Default::default()
                },
                Placement {
                    rotation: 0.7,
                    reflected: [true, false],
                    ..Default::default()
                },
                Placement {
                    scale: 1.4,
                    ..Default::default()
                },
            ] {
                assert!(d.transform_objects(&HashSet::from([a.id, b.id]), delta));
                let c = edge(&d, c.id);
                assert!(c.path(0.01).iter().flatten().all(|v| v.is_finite()));
                for end in c.ends() {
                    let Endpoint::Attached {
                        object_id, anchor, ..
                    } = end
                    else {
                        panic!()
                    };
                    let Some(ObjectSnapshot::Shape(s)) = d.snapshot(*object_id) else {
                        panic!()
                    };
                    assert_eq!(end.position(), anchor.point(s).unwrap());
                }
            }
        }
    }
    #[test]
    fn alignment_thresholds_and_semantic_reconstruction_are_deterministic() {
        let (_, a, mut b, mut c) = fixture(DocumentKind::Whiteboard, [280., 80.]);
        c.route = Route::Orthogonal { rail: None };
        for offset in [-0.01, 0.01, -0.01, 0.01, 0.] {
            b.placement.position[1] = 80. + offset;
            c.resolve_routes(|id| if id == a.id { Some(&a) } else { Some(&b) });
            let path = c.path(0.01);
            orthogonal(&path);
            let again = c.path(0.01);
            assert_eq!(path, again);
            // Degenerate collinear elbows disappear at exact alignment. Every
            // point remains within the actual subpixel displacement envelope.
            assert!(path.iter().all(|p| (p[1] - 110.).abs() <= 0.011));
        }
    }
    #[test]
    fn live_anchor_hint_retains_hysteresis_until_commit_and_clears_on_history() {
        let (mut d, a, mut b, mut c) = fixture(DocumentKind::Whiteboard, [280., 80.]);
        c.source.set_automatic(true);
        let mut threshold = 0.;
        for y in 80..1000 {
            b.placement.position[1] = y as f64;
            let mut probe = c.clone();
            probe.resolve_routes(|id| if id == a.id { Some(&a) } else { Some(&b) });
            if matches!(
                probe.source,
                Endpoint::Attached {
                    anchor: Port::Bottom,
                    ..
                }
            ) {
                threshold = y as f64;
                c = probe;
                break;
            }
        }
        assert!(threshold > 80.);
        d.hint_routing(&c);
        let mut frozen = edge(&d, c.id).clone();
        frozen.source.set_automatic(true);
        b.placement.position[1] = threshold - 5.;
        d.apply_routing_hint(&mut frozen);
        frozen.resolve_routes(|id| if id == a.id { Some(&a) } else { Some(&b) });
        assert!(matches!(
            frozen.source,
            Endpoint::Attached {
                anchor: Port::Bottom,
                ..
            }
        ));
        d.undo();
        let mut original = c.clone();
        original.source = Endpoint::attached(&a, Port::Right).unwrap();
        original.source.set_automatic(true);
        d.apply_routing_hint(&mut original);
        assert!(matches!(
            original.source,
            Endpoint::Attached {
                anchor: Port::Right,
                ..
            }
        ));
    }
    #[test]
    fn route_parameters_roundtrip_validate_and_transform_without_screen_units() {
        let (_, a, _, mut c) = fixture(DocumentKind::Whiteboard, [280., 300.]);
        for route in [
            Route::Straight,
            Route::Orthogonal {
                rail: Some(Rail {
                    axis: 0,
                    offset: 35.,
                }),
            },
            Route::Curved {
                controls: Some([[80., -35.], [-70., 40.]]),
            },
        ] {
            c.route = route;
            c.source.set_automatic(true);
            let restored: Connector =
                serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
            assert_eq!(c, restored);
            assert_eq!(c.path(0.01), restored.path(0.01));
            let transformed = c.transformed(
                Placement {
                    reflected: [true, false],
                    ..Default::default()
                },
                &HashSet::new(),
            );
            assert!(transformed.valid());
            if let Route::Orthogonal { rail: Some(r) } = transformed.route {
                assert_eq!(r.axis, 0);
                assert_eq!(r.offset, -35.);
            }
        }
        c.route = Route::Orthogonal {
            rail: Some(Rail {
                axis: 2,
                offset: 0.,
            }),
        };
        assert!(!c.valid());
        c.route = Route::Curved {
            controls: Some([[f64::NAN, 0.], [0.; 2]]),
        };
        assert!(!c.valid());
        let mut far = a.clone();
        far.placement.position = [1e15, -1e15];
        far.placement.scale = 0.0001;
        far.placement.rotation = 0.7;
        for port in Port::ALL {
            let n = port.normal(&far);
            assert!((n[0].hypot(n[1]) - 1.).abs() < 1e-10);
        }
    }
}
