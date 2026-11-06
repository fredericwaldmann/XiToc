//! Shared cached border adapter: semantic arclength patterns and joined meshes.
use crate::{
    drawing::Transform,
    object::Point,
    shape::{BorderStyle, ShapeStyle},
    stroke_pattern::MeasuredPath,
};
use egui::{Color32, Pos2, Vec2};

pub fn shapes(
    points: &[Point],
    closed: bool,
    style: &ShapeStyle,
    object_scale: f64,
    view: Transform,
    ppp: f32,
    solid: bool,
) -> Vec<egui::Shape> {
    if !style.border_enabled {
        return Vec::new();
    }
    let path = MeasuredPath::new(points, closed);
    let color = Color32::from_rgba_unmultiplied(
        style.border[0],
        style.border[1],
        style.border[2],
        style.border[3],
    );
    let screen = |p: Point| {
        let p = view.to_screen(p);
        Pos2::new(p[0] as f32, p[1] as f32)
    };
    let width = (style.border_width * object_scale * view.scale) as f32;
    match if solid {
        BorderStyle::Solid
    } else {
        style.border_style
    } {
        BorderStyle::Solid => vec![joined_stroke(
            &path.points.iter().copied().map(screen).collect::<Vec<_>>(),
            closed,
            width,
            color,
            ppp,
        )],
        BorderStyle::Dashed => path
            .dashes(
                style.pattern.dash * object_scale,
                style.pattern.gap * object_scale,
            )
            .into_iter()
            .map(|p| {
                joined_stroke(
                    &p.into_iter().map(screen).collect::<Vec<_>>(),
                    false,
                    width,
                    color,
                    ppp,
                )
            })
            .collect(),
        BorderStyle::Dotted => path
            .dots(
                style.pattern.dot_spacing * object_scale,
                style.pattern.dot_size * object_scale,
            )
            .into_iter()
            .map(|p| {
                egui::Shape::circle_filled(
                    screen(p),
                    (style.pattern.dot_size * object_scale * view.scale * 0.5) as f32,
                    color,
                )
            })
            .collect(),
    }
}
/// Cyclic joins for closed contours, joined dash paths through every corner.
/// Miter joins up to a two-half-width limit; sharper turns use a bounded bevel
/// construction based on egui's open-path tessellator.
fn joined_stroke(
    points: &[Pos2],
    closed: bool,
    width: f32,
    color: Color32,
    ppp: f32,
) -> egui::Shape {
    let mut points = points.to_vec();
    points.dedup();
    if closed && points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    if points.len() < 2 {
        return egui::Shape::Noop;
    }
    let mut path = egui::epaint::tessellator::Path::default();
    let n = points.len();
    for i in 0..n {
        let previous = (points[i] - points[(i + n - 1) % n]).normalized().rot90();
        let next = (points[(i + 1) % n] - points[i]).normalized().rot90();
        if !closed && i == 0 {
            path.add_point(points[i], next);
            continue;
        }
        if !closed && i == n - 1 {
            path.add_point(points[i], previous);
            continue;
        }
        let normal = (previous + next) * 0.5;
        if normal.length_sq() < 0.25 {
            let center = normal.normalized();
            for side in [previous, next] {
                let bevel = (side + center) * 0.5;
                path.add_point(
                    points[i],
                    if bevel == Vec2::ZERO {
                        side
                    } else {
                        bevel / bevel.length_sq()
                    },
                );
            }
        } else {
            path.add_point(points[i], normal / normal.length_sq());
        }
    }
    let mut mesh = egui::Mesh::default();
    let stroke = egui::epaint::PathStroke::new(width, color);
    if closed {
        path.stroke_closed(1. / ppp, &stroke, &mut mesh);
    } else {
        path.stroke_open(1. / ppp, &stroke, &mut mesh);
    }
    egui::Shape::mesh(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn covered(mesh: &egui::Mesh, p: Pos2) -> bool {
        mesh.indices.as_chunks::<3>().0.iter().any(|t| {
            let [a, b, c] = [
                mesh.vertices[t[0] as usize].pos,
                mesh.vertices[t[1] as usize].pos,
                mesh.vertices[t[2] as usize].pos,
            ];
            let cross = |u: Vec2, v: Vec2| u.x * v.y - u.y * v.x;
            let signs = [
                cross(b - a, p - a),
                cross(c - b, p - b),
                cross(a - c, p - c),
            ];
            (signs.iter().all(|v| *v >= -1e-5) || signs.iter().all(|v| *v <= 1e-5))
                && cross(b - a, c - a).abs() > 1e-6
        })
    }
    #[test]
    fn closed_rectangle_mesh_covers_every_edge_and_corner_after_rotation_zoom() {
        for scale in [0.5, 1., 2., 4.] {
            for rotation in [0., 0.37, 1.2] {
                let points = [[0., 0.], [100., 0.], [100., 60.], [0., 60.]].map(|p| {
                    let (s, c) = f64::sin_cos(rotation);
                    Pos2::new(
                        ((p[0] * c - p[1] * s) * scale + 200.) as f32,
                        ((p[0] * s + p[1] * c) * scale + 200.) as f32,
                    )
                });
                let egui::Shape::Mesh(mesh) =
                    joined_stroke(&points, true, (4. * scale) as f32, Color32::BLACK, 2.)
                else {
                    panic!()
                };
                let center = points[0].lerp(points[2], 0.5);
                for corner in points {
                    assert!(
                        covered(
                            &mesh,
                            corner + (corner - center).normalized() * (scale as f32 * 1.4)
                        ),
                        "Corner join missing"
                    );
                }
                for edge in 0..4 {
                    for i in 0..=100 {
                        let p = points[edge].lerp(points[(edge + 1) % 4], i as f32 / 100.);
                        assert!(
                            covered(&mesh, p),
                            "Missing edge/corner at {p:?}, scale {scale}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn patterns_keep_mark_counts_and_dot_sizes_independent_of_width_and_zoom() {
        let points = [[0., 0.], [100., 0.], [100., 60.], [0., 60.], [0., 0.]];
        for pattern in [BorderStyle::Dashed, BorderStyle::Dotted] {
            let mut style = ShapeStyle {
                border_style: pattern,
                ..Default::default()
            };
            let mut count = None;
            for width in [1., 2., 12.] {
                style.border_width = width;
                for zoom in [0.5, 1., 2., 4.] {
                    let view = Transform {
                        scale: zoom,
                        origin: [200., 200.],
                        center: [0.; 2],
                    };
                    let rendered = shapes(&points, true, &style, 1., view, 2., false);
                    if let Some(n) = count {
                        assert_eq!(rendered.len(), n);
                    } else {
                        count = Some(rendered.len());
                    }
                    if pattern == BorderStyle::Dotted {
                        for shape in rendered {
                            let egui::Shape::Circle(circle) = shape else {
                                panic!()
                            };
                            assert_eq!(circle.radius, (style.pattern.dot_size * zoom * 0.5) as f32);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn rounded_rectangle_circle_and_ellipse_have_continuous_closed_meshes() {
        use crate::shape::{Shape, ShapeKind};
        for kind in [
            ShapeKind::RoundedRectangle,
            ShapeKind::Circle,
            ShapeKind::Ellipse,
        ] {
            let mut shape = Shape::create(
                uuid::Uuid::new_v4(),
                crate::drawing::Surface::Canvas,
                kind,
                [0., 0.],
                [120., 80.],
                false,
            )
            .unwrap();
            shape.placement.rotation = 0.37;
            let contours = shape.contours(0.01);
            let points = &contours[0];
            for zoom in [0.5, 1., 2., 4.] {
                let view = Transform {
                    scale: zoom,
                    origin: [200., 200.],
                    center: [0.; 2],
                };
                let output = shapes(points, true, &shape.style, 1., view, 2., false);
                let egui::Shape::Mesh(mesh) = &output[0] else {
                    panic!()
                };
                for point in points.iter().step_by((points.len() / 32).max(1)) {
                    let p = view.to_screen(*point);
                    assert!(
                        covered(mesh, egui::pos2(p[0] as f32, p[1] as f32)),
                        "Curve border missing: {kind:?} at {zoom}"
                    );
                }
            }
        }
    }
    #[test]
    fn acute_join_has_bounded_vertices_and_single_mesh() {
        let points = [egui::pos2(0., 0.), egui::pos2(200., 1.), egui::pos2(0., 2.)];
        let egui::Shape::Mesh(mesh) = joined_stroke(&points, true, 4., Color32::BLACK, 1.) else {
            panic!()
        };
        assert!(
            mesh.vertices
                .iter()
                .all(|v| v.pos.x >= -6. && v.pos.x <= 206. && v.pos.y >= -6. && v.pos.y <= 8.)
        );
    }
}
