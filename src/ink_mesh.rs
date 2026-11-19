//! Union the coverage of one vector stroke before applying its alpha.
use super::{Stroke, Transform, rounded_shapes};
use egui::{Color32, Mesh, Pos2, Shape, Vec2, epaint::WHITE_UV};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::{overlay::FloatOverlay, single::SingleFloatOverlay},
    mesh::float::{outline::offset::OutlineOffset, style::OutlineStyle},
};

#[path = "ink_tiles.rs"]
mod tiles;
pub(super) use tiles::Tiles;

type Contour = Vec<[f64; 2]>;
type StrokePiece<'a> = (&'a [[f64; 2]], &'a [Option<f32>]);

pub(super) fn mesh(
    pieces: &[StrokePiece<'_>],
    transform: Transform,
    stroke: &Stroke,
    dpi: f32,
) -> Mesh {
    let mut contours = Vec::new();
    for &(path, pressures) in pieces {
        if stroke.pen_type == crate::pen::PenType::Highlighter && path.len() > 1 {
            flat_contours(path, pressures, transform, stroke, &mut contours);
        } else {
            round_contours(path, pressures, transform, stroke, dpi, &mut contours);
        }
    }
    coverage_mesh(&union(&contours), dpi)
}

fn round_contours(
    path: &[[f64; 2]],
    pressures: &[Option<f32>],
    transform: Transform,
    stroke: &Stroke,
    dpi: f32,
    contours: &mut Vec<Contour>,
) {
    for shape in rounded_shapes(
        path,
        pressures,
        transform,
        stroke,
        false,
        Color32::WHITE,
        dpi,
    ) {
        let contour = match shape {
            Shape::Circle(circle) => {
                let n = ((circle.radius * dpi * 3.0).sqrt() * 4.0)
                    .ceil()
                    .clamp(8.0, 64.0) as usize;
                (0..n)
                    .map(|i| {
                        let angle = std::f32::consts::TAU * i as f32 / n as f32;
                        circle.center + Vec2::angled(angle) * circle.radius
                    })
                    .collect()
            }
            Shape::Path(path) => path.points,
            Shape::LineSegment {
                points: [a, b],
                stroke,
            } => {
                let d = (b - a).normalized().rot90() * (stroke.width * 0.5);
                vec![a - d, b - d, b + d, a + d]
            }
            _ => unreachable!(),
        };
        add_contour(contour, contours);
    }
}

fn union(contours: &[Contour]) -> Vec<Vec<Contour>> {
    if contours.is_empty() {
        return Vec::new();
    }
    FloatOverlay::<[f64; 2], i64>::from_subj(contours)
        .overlay(OverlayRule::Subject, FillRule::NonZero)
}

fn coverage_mesh(union: &[Vec<Contour>], dpi: f32) -> Mesh {
    let mut mesh = Mesh::default();
    if union.is_empty() {
        return mesh;
    }
    let pixel = 1.0 / f64::from(dpi);
    let expanded = union.outline_as::<i64>(&OutlineStyle::new(pixel));
    let fringe = expanded.overlay_as::<i64>(&union, OverlayRule::Difference, FillRule::NonZero);
    // The difference retains the original boundary and its exterior offset.
    // Triangulating the whole fringe also unifies coverage in narrow gaps.
    let key = |p: [f64; 2]| p.map(|v| (v as f32 + 0.0).to_bits());
    let boundary: std::collections::HashSet<_> =
        union.iter().flatten().flatten().copied().map(key).collect();
    for polygon in union {
        triangulate(&mut mesh, polygon, |_| Color32::WHITE);
    }
    for polygon in &fringe {
        triangulate(&mut mesh, polygon, |p| {
            if boundary.contains(&key(p)) {
                Color32::WHITE
            } else {
                Color32::TRANSPARENT
            }
        });
    }
    mesh
}

fn triangulate(mesh: &mut Mesh, polygon: &[Contour], coverage: impl Fn([f64; 2]) -> Color32) {
    let mut holes = Vec::new();
    let mut coordinates = Vec::new();
    for (i, ring) in polygon.iter().enumerate() {
        if i > 0 {
            holes.push(coordinates.len() / 2);
        }
        coordinates.extend(ring.iter().flatten().copied());
    }
    let Ok(indices) = earcutr::earcut(&coordinates, &holes, 2) else {
        return;
    };
    let base = mesh.vertices.len() as u32;
    mesh.indices
        .extend(indices.into_iter().map(|i| base + i as u32));
    for &[x, y] in coordinates.as_chunks::<2>().0 {
        vertex(mesh, egui::pos2(x as f32, y as f32), coverage([x, y]));
    }
}

fn add_contour(points: Vec<Pos2>, contours: &mut Vec<Contour>) {
    let area = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| f64::from(a.x) * f64::from(b.y) - f64::from(a.y) * f64::from(b.x))
        .sum::<f64>();
    if area == 0.0 {
        return;
    }
    let to_point = |p: Pos2| [f64::from(p.x), f64::from(p.y)];
    // Every swept primitive contributes positive winding even at a reversal.
    if area < 0.0 {
        contours.push(points.into_iter().rev().map(to_point).collect());
    } else {
        contours.push(points.into_iter().map(to_point).collect());
    }
}

fn flat_contours(
    path: &[[f64; 2]],
    pressures: &[Option<f32>],
    transform: Transform,
    stroke: &Stroke,
    contours: &mut Vec<Contour>,
) {
    let start = contours.len();
    flat_range(
        path,
        pressures,
        transform,
        stroke,
        0.0,
        0..path.len() - 1,
        contours,
    );
    if contours.len() == start {
        let radius = pressures
            .iter()
            .map(|force| nib_radius(stroke, transform, *force))
            .fold(0.0_f32, f32::max);
        let center = screen_point(transform, path[0]);
        add_contour(
            (0..24)
                .map(|i| center + radius * Vec2::angled(std::f32::consts::TAU * i as f32 / 24.0))
                .collect(),
            contours,
        );
    }
}

fn screen_point(transform: Transform, point: [f64; 2]) -> Pos2 {
    let [x, y] = transform.to_screen(point);
    egui::pos2(x as f32, y as f32)
}

fn nib_radius(stroke: &Stroke, transform: Transform, pressure: Option<f32>) -> f32 {
    (0.5 * stroke.visible_width() * transform.scale * stroke.pressure_style.factor(pressure)) as f32
}

fn flat_range(
    path: &[[f64; 2]],
    pressures: &[Option<f32>],
    transform: Transform,
    stroke: &Stroke,
    tolerance: f32,
    segments: std::ops::Range<usize>,
    contours: &mut Vec<Contour>,
) {
    let point = |i: usize| screen_point(transform, path[i]);
    let radius = |i: usize| nib_radius(stroke, transform, pressures[i]);
    let mut previous = (0..segments.start).rev().find_map(|i| {
        let d = (point(i + 1) - point(i)).normalized();
        (d != Vec2::ZERO).then(|| d.rot90() * radius(i + 1))
    });
    let samples: Vec<_> = (segments.start..=segments.end)
        .map(|i| (point(i), radius(i)))
        .collect();
    // Keep the boundary tangents, including the live nib's final segment.
    // Interior samples use a subpixel bound on center/radius displacement.
    let kept = if samples.len() < 5 {
        (0..samples.len()).collect::<Vec<_>>()
    } else {
        let mut kept = vec![0];
        kept.extend(
            super::simplify_swept_disks(&samples[1..samples.len() - 1], tolerance)
                .into_iter()
                .map(|index| index + 1),
        );
        kept.push(samples.len() - 1);
        kept
    };
    for segment in kept.windows(2) {
        let (a, ra) = samples[segment[0]];
        let (b, rb) = samples[segment[1]];
        let direction = (b - a).normalized();
        if direction == Vec2::ZERO {
            continue;
        }
        let offset = Vec2::new(-direction.y, direction.x);
        let (start, end) = (offset * ra, offset * rb);
        // A subpixel shared area keeps independent polygon unions from
        // rounding merely coincident join edges to opposite sides.
        let overlap = direction * (tolerance / 3.0);
        let strip_start = if segments.start + segment[0] == 0 {
            a
        } else {
            a - overlap
        };
        let strip_end = if segments.start + segment[1] == path.len() - 1 {
            b
        } else {
            b + overlap
        };
        add_contour(
            vec![
                strip_start - start,
                strip_end - end,
                strip_end + end,
                strip_start + start,
            ],
            contours,
        );
        if let Some(previous) = previous {
            // A bevel closes the outside of the join without folding the nib
            // back across itself when the centerline makes a sharp turn.
            for side in [-1.0, 1.0] {
                add_contour(vec![a, a + previous * side, a + start * side], contours);
            }
        }
        previous = Some(end);
    }
}

fn vertex(mesh: &mut Mesh, pos: Pos2, color: Color32) {
    mesh.vertices.push(egui::epaint::Vertex {
        pos,
        uv: WHITE_UV,
        color,
    });
}

/// Tessellate the tangent strip of the pressure disks, plus just the exposed
/// arcs at joins and caps. The fringe is confined to the exterior boundaries.
pub(super) fn opaque_round_mesh(
    (path, pressures): StrokePiece<'_>,
    transform: Transform,
    stroke: &Stroke,
    dpi: f32,
) -> Mesh {
    use std::f32::consts::{PI, TAU};
    let aa = 1.0 / dpi;
    let samples: Vec<_> = path
        .iter()
        .zip(pressures)
        .map(|(&p, &force)| {
            (
                screen_point(transform, p),
                nib_radius(stroke, transform, force),
            )
        })
        .collect();
    let kept = super::simplify_swept_disks(&samples, 0.2 * aa);
    let mut points = kept.into_iter().map(|i| samples[i]);
    let mut mesh = Mesh::default();
    let Some((mut a, mut ra)) = points.next() else {
        return mesh;
    };
    let mut last: Option<(Vec2, f32, Vec2)> = None;
    for (b, rb) in points {
        let delta = b - a;
        let length = delta.length();
        if length == 0.0 && ra == rb {
            continue;
        }
        let slope = (ra - rb) / length;
        if slope.abs() >= 1.0 {
            // The steep endpoint contains the entire segment. Both endpoints
            // may still border exposed parts of their neighboring strips.
            if let Some((_, tilt, right)) = last.take() {
                arc(&mut mesh, a, right, PI - 2.0 * tilt, ra, aa);
            }
            let (center, radius) = if ra >= rb { (a, ra) } else { (b, rb) };
            arc(&mut mesh, center, Vec2::X, TAU, radius, aa);
            (a, ra) = (b, rb);
            continue;
        }
        let dir = delta / length;
        let normal = dir.rot90() * (1.0 - slope * slope).sqrt();
        let left = dir * slope + normal;
        let right = dir * slope - normal;
        let tilt = slope.asin();
        if let Some((previous, prior_tilt, prior_right)) = last {
            let cross = previous.x * dir.y - previous.y * dir.x;
            let turn = cross.atan2(previous.dot(dir));
            let right_sweep = turn + tilt - prior_tilt;
            let left_sweep = turn - tilt + prior_tilt;
            if right_sweep > 0.0001 {
                arc(&mut mesh, a, prior_right, right_sweep, ra, aa);
            }
            if left_sweep < -0.0001 {
                arc(&mut mesh, a, left, -left_sweep, ra, aa);
            }
        } else {
            arc(&mut mesh, a, left, PI + 2.0 * tilt, ra, aa);
        }
        let base = mesh.vertices.len() as u32;
        // Share a subpixel area with the radial edges of caps and joins.
        let overlap = dir * (0.05 * aa);
        for (point, radius, offset) in [
            (a - overlap, ra, right),
            (a - overlap, ra, left),
            (b + overlap, rb, right),
            (b + overlap, rb, left),
        ] {
            vertex(&mut mesh, point + offset * radius, Color32::WHITE);
            vertex(
                &mut mesh,
                point + offset * (radius + aa),
                Color32::TRANSPARENT,
            );
        }
        for index in [
            0, 2, 4, 2, 6, 4, // core
            0, 4, 1, 1, 4, 5, // right fringe
            2, 3, 6, 3, 7, 6, // left fringe
        ] {
            mesh.indices.push(base + index);
        }
        last = Some((dir, tilt, right));
        (a, ra) = (b, rb);
    }
    if let Some((_, tilt, right)) = last {
        arc(&mut mesh, a, right, PI - 2.0 * tilt, ra, aa);
    } else if mesh.is_empty() {
        arc(&mut mesh, a, Vec2::X, TAU, ra, aa);
    }
    mesh
}

fn arc(mesh: &mut Mesh, center: Pos2, start: Vec2, sweep: f32, radius: f32, aa: f32) {
    // Bound the polygon sagitta to 0.15 physical pixels.
    let step = (1.0 - 0.15 * aa / (radius + aa)).acos() * 2.0;
    let steps = (sweep.abs() / step).ceil().clamp(1.0, 128.0) as u32;
    let base = mesh.vertices.len() as u32;
    vertex(mesh, center, Color32::WHITE);
    for i in 0..=steps {
        let n = if i == 0 {
            start
        } else {
            let rotation = Vec2::angled(sweep * i as f32 / steps as f32);
            egui::vec2(
                start.x * rotation.x - start.y * rotation.y,
                start.y * rotation.x + start.x * rotation.y,
            )
        };
        vertex(mesh, center + n * radius, Color32::WHITE);
        vertex(mesh, center + n * (radius + aa), Color32::TRANSPARENT);
        if i > 0 {
            let a = base + 2 * i - 1;
            let b = a + 2;
            mesh.indices
                .extend([base, a, b, a, a + 1, b, b, a + 1, b + 1]);
        }
    }
}

pub(super) fn tint(mesh: &mut Mesh, color: Color32, coverage: &[u8]) {
    for (vertex, &alpha) in mesh.vertices.iter_mut().zip(coverage) {
        vertex.color = color.gamma_multiply_u8(alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::{Smoothing, Surface};

    fn interior_hits(mesh: &Mesh, point: Pos2) -> usize {
        mesh.indices
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|tri| {
                let [a, b, c] = tri.map(|i| mesh.vertices[i as usize].pos);
                let cross = |u: Vec2, v: Vec2| u.x * v.y - u.y * v.x;
                let sides = [
                    cross(b - a, point - a),
                    cross(c - b, point - b),
                    cross(a - c, point - c),
                ];
                sides.iter().all(|v| *v > 0.0) || sides.iter().all(|v| *v < 0.0)
            })
            .count()
    }

    #[test]
    fn crossings_have_one_coverage_hit_per_stroke_at_each_zoom() {
        let mut stroke = Stroke::new(
            uuid::Uuid::new_v4(),
            Surface::Canvas,
            [70, 40, 90, 102],
            8.0,
        );
        stroke.smoothing = Smoothing::PolylineV1;
        stroke.pressure_style.enabled = true;
        let path = [[10., 10.], [100., 100.], [100., 10.], [10., 100.]];
        let force = [Some(0.25), Some(0.5), Some(1.0), Some(0.75)];
        let other = [[5., 55.], [105., 55.]];
        for scale in [0.5, 1., 2., 4.] {
            let transform = Transform {
                origin: [10., 10.],
                center: [0., 0.],
                scale,
            };
            for pen in crate::pen::PenType::ALL {
                stroke.pen_type = pen;
                let first = mesh(&[(&path, &force)], transform, &stroke, 1.0);
                let second = mesh(&[(&other, &[None, None])], transform, &stroke, 1.0);
                let [x, y] = transform.to_screen([55.18, 55.07]);
                let point = egui::pos2(x as f32, y as f32);
                assert_eq!(interior_hits(&first, point), 1, "{pen:?} at {scale}");
                assert_eq!(interior_hits(&second, point), 1, "{pen:?} at {scale}");
                assert!(
                    first
                        .vertices
                        .iter()
                        .any(|v| v.color == Color32::TRANSPARENT)
                );
            }
        }
    }
}
