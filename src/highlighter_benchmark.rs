//! Repeatable CPU-only highlighter pan profile using the production paint path.
use super::*;
use crate::{coordinate, document::DocumentKind, formula, shape, text};
use std::time::Instant;

#[test]
#[ignore = "release highlighter pan profile"]
fn highlighter_pan_profile() {
    for (scene, count) in [
        ("opaque", 1_000),
        ("highlighters", 1_000),
        ("overlap", 1_000),
        ("distributed", 5_000),
        ("empty", 5_000),
        ("mixed", 1_000),
    ] {
        for scale in [1., 2., 4., 8.] {
            let ctx = egui::Context::default();
            crate::text_layout::install(&ctx);
            let data = DocumentData::blank(DocumentKind::Whiteboard);
            let mut canvas = Canvas::new(&data);
            let mut objects = Vec::with_capacity(count);
            for i in 0..count {
                let (x, y) = if matches!(scene, "overlap" | "empty") {
                    ((i % 16) as f64 * 3., (i / 16 % 20) as f64 * 3. + 35.)
                } else {
                    ((i % 50) as f64 * 150., (i / 50) as f64 * 72. + 35.)
                };
                if scene == "mixed" {
                    let other = match i % 12 {
                        0 => Some(
                            shape::Shape::create(
                                data.id(),
                                Surface::Canvas,
                                shape::ShapeKind::Ellipse,
                                [x, y],
                                [x + 66., y + 35.],
                                false,
                            )
                            .unwrap()
                            .into(),
                        ),
                        1 => {
                            let mut label =
                                text::TextObject::new(data.id(), Surface::Canvas, [x, y], 90.);
                            label.content_mut().text = "Field notes".into();
                            Some(label.into())
                        }
                        2 => {
                            let source = r"\sqrt{x^2+1}";
                            let mut equation = formula::FormulaObject::new(
                                data.id(),
                                Surface::Canvas,
                                [x, y],
                                source.into(),
                                [23, 34, 45, 255],
                            );
                            let measured =
                                crate::formula_render::render(source, equation.color).unwrap();
                            equation.layout = formula::FormulaLayout {
                                width: f64::from(measured.width),
                                height: f64::from(measured.height),
                                baseline: measured.baseline,
                            };
                            Some(equation.into())
                        }
                        3 => Some(
                            coordinate::Coordinate::new(
                                data.id(),
                                Surface::Canvas,
                                [x + 120., y + 140.],
                                coordinate::Preset::FourQuadrants,
                            )
                            .into(),
                        ),
                        _ => None,
                    };
                    if let Some(other) = other {
                        objects.push(other);
                        continue;
                    }
                }
                let opaque = scene == "opaque" || scene == "mixed" && i % 12 == 4;
                let mut stroke = Stroke::new(
                    data.id(),
                    Surface::Canvas,
                    if opaque {
                        [34, 58, 90, 255]
                    } else {
                        [232, 191, 32, 89]
                    },
                    24.,
                );
                if !opaque {
                    stroke.pen_type = crate::pen::PenType::Highlighter;
                }
                stroke.smoothing = drawing::Smoothing::AdaptiveV1 { strength: 65 };
                for j in 0..48 {
                    let t = f64::from(j);
                    stroke.push([x + t * 2.7, y + (t * 0.11).sin() * 9.]);
                }
                objects.push(stroke.into());
            }
            assert!(canvas.drawing.insert_snapshots(objects));
            let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(1100., 680.));
            let mut times = Vec::new();
            let (mut prepare, mut assemble, mut rebases, mut peak) = (0., 0., 0, [0; 4]);
            let mut bases = HashMap::new();
            let mut cold = 0.;
            for frame in 0..26 {
                let start = Instant::now();
                let center = if scene == "empty" { 50_000. } else { -20. };
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(rect),
                        ..Default::default()
                    },
                    |ui| {
                        canvas.paint(
                            ui,
                            rect,
                            Surface::Canvas,
                            Transform {
                                origin: [0.; 2],
                                center: [center + f64::from(frame) * 5., 0.],
                                scale,
                            },
                        );
                    },
                );
                let prep = start.elapsed().as_secs_f64() * 1000.;
                let shapes = out.shapes.len();
                let primitives =
                    ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
                let total = start.elapsed().as_secs_f64() * 1000.;
                out.textures_delta.clear();
                let mut changed = 0;
                for (&id, cache) in &canvas.cache {
                    if let Some((key, _)) = &cache.tiled
                        && bases.insert(id, key.transform) != Some(key.transform)
                    {
                        changed += 1;
                    }
                }
                if frame == 0 {
                    cold = total;
                }
                if frame >= 2 {
                    times.push(total);
                    prepare += prep;
                    assemble += total - prep;
                    rebases += changed;
                    let (vertices, triangles) = primitives.iter().fold((0, 0), |(v, t), p| {
                        if let egui::epaint::Primitive::Mesh(mesh) = &p.primitive {
                            (v + mesh.vertices.len(), t + mesh.indices.len() / 3)
                        } else {
                            (v, t)
                        }
                    });
                    for (max, value) in
                        peak.iter_mut()
                            .zip([shapes, primitives.len(), vertices, triangles])
                    {
                        *max = (*max).max(value);
                    }
                }
            }
            times.sort_by(f64::total_cmp);
            let n = times.len() as f64;
            eprintln!(
                "{scene} {scale}x mean/p95/max {:.3}/{:.3}/{:.3} ms paint {:.3} assembly {:.3} cold {cold:.3} cache {} tile changes {rebases} peak submissions/batches/verts/tris {peak:?}",
                times.iter().sum::<f64>() / n,
                times[22],
                times[23],
                prepare / n,
                assemble / n,
                canvas.cache.len()
            );
        }
    }
}
