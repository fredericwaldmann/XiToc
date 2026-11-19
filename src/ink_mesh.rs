//! Union the coverage of one vector stroke before applying its alpha.
use super::{Stroke, Transform, rounded_shapes};
use egui::{Color32, Mesh, Pos2, Shape, Vec2, epaint::WHITE_UV};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::{overlay::FloatOverlay, single::SingleFloatOverlay},
    mesh::float::{outline::offset::OutlineOffset, style::OutlineStyle},
};

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
                    _ => unreachable!(),
                };
                add_contour(contour, &mut contours);
            }
        }
    }
    let mut mesh = Mesh::default();
    if contours.is_empty() {
        return mesh;
    }
    let union = FloatOverlay::<[f64; 2], i64>::from_subj(&contours)
        .overlay(OverlayRule::Subject, FillRule::NonZero);
    let pixel = 1.0 / f64::from(dpi);
    let expanded = union.outline_as::<i64>(&OutlineStyle::new(pixel));
    let fringe = expanded.overlay_as::<i64>(&union, OverlayRule::Difference, FillRule::NonZero);
    // The difference retains the original boundary and its exterior offset.
    // Triangulating the whole fringe also unifies coverage in narrow gaps.
    let key = |p: [f64; 2]| p.map(|v| (v as f32 + 0.0).to_bits());
    let boundary: std::collections::HashSet<_> =
        union.iter().flatten().flatten().copied().map(key).collect();
    for polygon in &union {
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
    let points: Vec<_> = path
        .iter()
        .map(|point| {
            let [x, y] = transform.to_screen(*point);
            egui::pos2(x as f32, y as f32)
        })
        .collect();
    let directions: Vec<_> = points
        .windows(2)
        .map(|p| (p[1] - p[0]).normalized())
        .collect();
    let radius = |i: usize| {
        (0.5 * stroke.visible_width()
            * transform.scale
            * stroke.pressure_style.factor(pressures[i])) as f32
    };
    if directions.iter().all(|d| *d == Vec2::ZERO) {
        let radius = (0..pressures.len()).map(radius).fold(0.0_f32, f32::max);
        add_contour(
            (0..24)
                .map(|i| points[0] + radius * Vec2::angled(std::f32::consts::TAU * i as f32 / 24.0))
                .collect(),
            contours,
        );
        return;
    }
    let mut previous = None::<Vec2>;
    for (i, direction) in directions.into_iter().enumerate() {
        if direction == Vec2::ZERO {
            continue;
        }
        let offset = Vec2::new(-direction.y, direction.x);
        let (a, b) = (points[i], points[i + 1]);
        let (start, end) = (offset * radius(i), offset * radius(i + 1));
        add_contour(vec![a - start, b - end, b + end, a + start], contours);
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
