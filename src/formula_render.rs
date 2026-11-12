//! Bounded LaTeX-Rust adapter. It caches no document state; callers cache its
//! local-coordinate vector shapes by source and appearance.
use latex_rust::{
    Color, Dim, EguiOptions, MathBox, MathFont, MathStyle, ParseOptions, layout_with_em_size_pt,
    parse_with_options, shapes, tokenize,
};
use std::sync::OnceLock;

pub const MAX_PARSE_DEPTH: usize = 32;

static FONT: OnceLock<Result<MathFont, String>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct RenderedFormula {
    pub shapes: Vec<egui::Shape>,
    pub width: f32,
    pub height: f32,
    pub baseline: f64,
}

pub fn render(source: &str, color: [u8; 4]) -> Result<RenderedFormula, String> {
    if source.len() > crate::formula::MAX_FORMULA_BYTES {
        return Err(format!(
            "Formula source is limited to {} bytes.",
            crate::formula::MAX_FORMULA_BYTES
        ));
    }
    if source.trim().is_empty() {
        return Err("Enter a LaTeX math expression.".into());
    }
    let tokens = tokenize(source).map_err(|e| e.to_string())?;
    if tokens.len() > crate::formula::MAX_FORMULA_TOKENS {
        return Err(format!(
            "Formula complexity is limited to {} math tokens.",
            crate::formula::MAX_FORMULA_TOKENS
        ));
    }
    let font = FONT
        .get_or_init(|| MathFont::stix_two_math().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(Clone::clone)?;
    let (ast, _) = parse_with_options(source, &ParseOptions::new().with_max_depth(MAX_PARSE_DEPTH))
        .map_err(|e| e.to_string())?;
    let em = Dim::from_i64(crate::formula::FORMULA_FONT_SIZE as i64);
    let tree =
        layout_with_em_size_pt(&ast, font, MathStyle::Display, &em).map_err(|e| e.to_string())?;
    let options = EguiOptions {
        font_size_pt: em,
        color: Color::rgb(color[0], color[1], color[2]),
        display: true,
    };
    let (native_shapes, rect) =
        shapes(&tree, font, &options, egui_036::Pos2::ZERO, 1.0).map_err(|e| e.to_string())?;
    let shapes = native_shapes
        .into_iter()
        .map(convert_shape)
        .collect::<Result<Vec<_>, _>>()?;
    let baseline = dim_to_f64(&tree.height) * crate::formula::FORMULA_FONT_SIZE;
    if !rect.width().is_finite()
        || !rect.height().is_finite()
        || rect.width() <= 0.0
        || rect.height() <= 0.0
    {
        return Err("The formula produced invalid geometry.".into());
    }
    Ok(RenderedFormula {
        shapes,
        width: rect.width(),
        height: rect.height(),
        baseline,
    })
}

fn dim_to_f64(dim: &Dim) -> f64 {
    f32::from_bits(dim.to_ieee32_bits()) as f64
}

fn color32(color: egui_036::Color32) -> egui::Color32 {
    let [r, g, b, a] = color.to_array();
    egui::Color32::from_rgba_premultiplied(r, g, b, a)
}

fn convert_shape(shape: egui_036::Shape) -> Result<egui::Shape, String> {
    use egui_036::Shape as Source;
    Ok(match shape {
        Source::Noop => egui::Shape::Noop,
        Source::Vec(shapes) => egui::Shape::Vec(
            shapes
                .into_iter()
                .map(convert_shape)
                .collect::<Result<_, _>>()?,
        ),
        Source::Mesh(mesh) => {
            let converted = egui::Mesh {
                indices: mesh.indices.clone(),
                vertices: mesh
                    .vertices
                    .iter()
                    .map(|vertex| egui::epaint::Vertex {
                        pos: egui::pos2(vertex.pos.x, vertex.pos.y),
                        uv: egui::pos2(vertex.uv.x, vertex.uv.y),
                        color: color32(vertex.color),
                    })
                    .collect(),
                ..Default::default()
            };
            egui::Shape::Mesh(converted.into())
        }
        Source::LineSegment { points, stroke } => egui::Shape::line_segment(
            [
                egui::pos2(points[0].x, points[0].y),
                egui::pos2(points[1].x, points[1].y),
            ],
            egui::Stroke::new(stroke.width, color32(stroke.color)),
        ),
        Source::Rect(rect) => {
            let min = egui::pos2(rect.rect.min.x, rect.rect.min.y);
            let max = egui::pos2(rect.rect.max.x, rect.rect.max.y);
            let mut shapes = Vec::with_capacity(2);
            if rect.fill.a() > 0 {
                shapes.push(egui::Shape::rect_filled(
                    egui::Rect::from_min_max(min, max),
                    0.0,
                    color32(rect.fill),
                ));
            }
            if rect.stroke.width > 0.0 && rect.stroke.color.a() > 0 {
                shapes.push(egui::Shape::rect_stroke(
                    egui::Rect::from_min_max(min, max),
                    0.0,
                    egui::Stroke::new(rect.stroke.width, color32(rect.stroke.color)),
                    egui::StrokeKind::Middle,
                ));
            }
            egui::Shape::Vec(shapes)
        }
        _ => return Err("The formula renderer returned an unsupported vector primitive.".into()),
    })
}

pub fn map_to_screen(
    shape: &egui::Shape,
    map: &mut dyn FnMut(egui::Pos2) -> egui::Pos2,
) -> egui::Shape {
    match shape {
        egui::Shape::Noop => egui::Shape::Noop,
        egui::Shape::Vec(shapes) => {
            egui::Shape::Vec(shapes.iter().map(|s| map_to_screen(s, map)).collect())
        }
        egui::Shape::Mesh(mesh) => {
            let mut mesh = (**mesh).clone();
            for vertex in &mut mesh.vertices {
                vertex.pos = map(vertex.pos);
            }
            egui::Shape::Mesh(mesh.into())
        }
        egui::Shape::LineSegment { points, stroke } => {
            egui::Shape::line_segment([map(points[0]), map(points[1])], *stroke)
        }
        egui::Shape::Rect(rect) => {
            let min = map(rect.rect.min);
            let max = map(rect.rect.max);
            let r = egui::Rect::from_min_max(min, max);
            let mut result = Vec::with_capacity(2);
            if rect.fill.a() > 0 {
                result.push(egui::Shape::rect_filled(r, rect.corner_radius, rect.fill));
            }
            if rect.stroke.width > 0.0 && rect.stroke.color.a() > 0 {
                result.push(egui::Shape::rect_stroke(
                    r,
                    rect.corner_radius,
                    rect.stroke,
                    rect.stroke_kind,
                ));
            }
            egui::Shape::Vec(result)
        }
        _ => shape.clone(),
    }
}

impl From<&MathBox> for crate::formula::FormulaLayout {
    fn from(value: &MathBox) -> Self {
        let scale = crate::formula::FORMULA_FONT_SIZE;
        Self {
            width: dim_to_f64(&value.width) * scale,
            height: (dim_to_f64(&value.height) + dim_to_f64(&value.depth)) * scale,
            baseline: dim_to_f64(&value.height) * scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_math_fixtures_produce_cached_vector_geometry() {
        for source in [
            r"\frac{a}{b}",
            r"\sqrt{x^2+y_1}",
            r"\sum_{i=1}^{n} i^2",
            r"\int_0^\infty e^{-x}\,dx",
            r"\lim_{x \to 0} \frac{\sin x}{x}",
            r"\begin{bmatrix}a&b\\c&d\end{bmatrix}",
            r"\begin{cases}x^2&x\ge0\\-x&x<0\end{cases}",
            r"\begin{aligned}x&=1\\y&=2\end{aligned}",
        ] {
            let out = render(source, [28, 38, 49, 255])
                .unwrap_or_else(|error| panic!("{source}: {error}"));
            assert!(!out.shapes.is_empty(), "{source}");
            assert!(out.width > 0.0 && out.height > 0.0 && out.baseline >= 0.0);
            assert!(
                crate::formula::FormulaLayout {
                    width: out.width as f64,
                    height: out.height as f64,
                    baseline: out.baseline,
                }
                .valid(),
                "invalid derived bounds: {source}"
            );
        }
    }
    #[test]
    fn malformed_unsupported_deep_and_oversized_sources_return_errors() {
        assert!(render(r"\notARealCommand{x}", [0, 0, 0, 255]).is_err());
        assert!(render(r"\frac{a}{", [0, 0, 0, 255]).is_err());
        let over_limit = std::iter::repeat_n("x", crate::formula::MAX_FORMULA_TOKENS / 2 + 1)
            .collect::<Vec<_>>()
            .join("+");
        assert!(render(&over_limit, [0, 0, 0, 255]).is_err());
        let deep = format!("{}x{}", "{".repeat(48), "}".repeat(48));
        assert!(render(&deep, [0, 0, 0, 255]).is_err());
        assert!(
            render(
                &"x".repeat(crate::formula::MAX_FORMULA_BYTES + 1),
                [0, 0, 0, 255]
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "release-only worst-case parser/layout latency measurement"]
    fn worst_case_supported_source_latency() {
        let source = std::iter::repeat_n("x", crate::formula::MAX_FORMULA_TOKENS / 2)
            .collect::<Vec<_>>()
            .join("+");
        let start = std::time::Instant::now();
        let output = render(&source, [0, 0, 0, 255]);
        eprintln!("worst-case formula render: {:?}", start.elapsed());
        assert!(output.is_ok());
    }
}
