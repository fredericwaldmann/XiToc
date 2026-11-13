//! Math layout and original Computer Modern outlines; physical-pixel rasterization is cached separately.
use latex_rust::{
    Color, Dim, MathFont, MathStyle, ParseOptions, SvgOptions, layout_with_em_size_pt,
    parse_with_options, render_svg_with_metrics, tokenize,
};
use std::sync::OnceLock;
pub const MAX_PARSE_DEPTH: usize = 32;
const MATH_FONT: &[u8] = include_bytes!("../assets/fonts/NewCMMath-Regular.otf");
static FONT: OnceLock<Result<MathFont, String>> = OnceLock::new();
#[derive(Clone, Debug)]
pub struct RenderedFormula {
    pub svg: String,
    pub width: f32,
    pub height: f32,
    pub baseline: f64,
}
pub fn render(source: &str, color: [u8; 4]) -> Result<RenderedFormula, String> {
    if source.len() > crate::formula::MAX_FORMULA_BYTES {
        return Err("Formula source exceeds 16 KiB.".into());
    }
    if source.trim().is_empty() {
        return Err("Enter a LaTeX math expression.".into());
    }
    let tokens = tokenize(source).map_err(|e| e.to_string())?;
    if tokens.len() > crate::formula::MAX_FORMULA_TOKENS {
        return Err("Formula complexity is limited to 128 math tokens.".into());
    }
    let font = FONT
        .get_or_init(|| MathFont::from_bytes(MATH_FONT).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(Clone::clone)?;
    let (ast, _) = parse_with_options(source, &ParseOptions::new().with_max_depth(MAX_PARSE_DEPTH))
        .map_err(|e| e.to_string())?;
    let em = Dim::from_i64(crate::formula::FORMULA_FONT_SIZE as i64);
    let tree =
        layout_with_em_size_pt(&ast, font, MathStyle::Display, &em).map_err(|e| e.to_string())?;
    let image = render_svg_with_metrics(
        &tree,
        font,
        &SvgOptions {
            font_size_pt: em,
            color: Color::rgb(color[0], color[1], color[2]),
            display: true,
        },
    )
    .map_err(|e| e.to_string())?;
    let out = RenderedFormula {
        svg: image.svg,
        width: dim(&image.width_pt) as f32,
        height: dim(&image.height_pt) as f32,
        baseline: dim(&image.baseline_pt),
    };
    if !(crate::formula::FormulaLayout {
        width: out.width as f64,
        height: out.height as f64,
        baseline: out.baseline,
    })
    .valid()
    {
        return Err("Formula produced invalid geometry.".into());
    }
    Ok(out)
}
fn dim(d: &Dim) -> f64 {
    f32::from_bits(d.to_ieee32_bits()) as f64
}

/// A single bounded texture per object/preview. Layout never depends on resolution.
#[derive(Default)]
pub struct FormulaTexture {
    texture: Option<egui::TextureHandle>,
    density: f32,
}
impl FormulaTexture {
    pub fn paint(
        &mut self,
        painter: &egui::Painter,
        formula: &RenderedFormula,
        density: f32,
        high_quality: bool,
        map: &mut dyn FnMut(egui::Pos2) -> egui::Pos2,
    ) {
        // Power-of-two buckets avoid rerasterization on every resize/zoom sample.
        // Supersampling supplies antialiased outlines even under rotation.
        let requested = (density.max(0.125) * if high_quality { 2.0 } else { 1.0 })
            .log2()
            .ceil()
            .exp2();
        let max_side = painter.ctx().input(|i| i.max_texture_side).min(4096) as f32;
        let limit = (max_side / formula.width.max(formula.height))
            .min((4_194_304.0 / (formula.width * formula.height)).sqrt());
        let target = requested.min(limit).max(0.001);
        if self.texture.is_none() || self.density != target {
            let options = resvg::usvg::Options::default();
            if let Ok(tree) = resvg::usvg::Tree::from_str(&formula.svg, &options) {
                let width = (formula.width * target).ceil().clamp(1.0, max_side) as u32;
                let height = (formula.height * target).ceil().clamp(1.0, max_side) as u32;
                if let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(width, height) {
                    resvg::render(
                        &tree,
                        resvg::tiny_skia::Transform::from_scale(
                            width as f32 / tree.size().width(),
                            height as f32 / tree.size().height(),
                        ),
                        &mut pixmap.as_mut(),
                    );
                    let image = egui::ColorImage {
                        size: [width as usize, height as usize],
                        source_size: egui::vec2(width as f32, height as f32),
                        pixels: pixmap
                            .pixels()
                            .iter()
                            .map(|p| {
                                egui::Color32::from_rgba_premultiplied(
                                    p.red(),
                                    p.green(),
                                    p.blue(),
                                    p.alpha(),
                                )
                            })
                            .collect(),
                    };
                    self.texture = Some(painter.ctx().load_texture(
                        "formula-outline",
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                    self.density = target;
                }
            }
        }
        if let Some(texture) = &self.texture {
            let mut mesh = egui::Mesh::with_texture(texture.id());
            for (p, uv) in [
                (egui::pos2(0., 0.), egui::pos2(0., 0.)),
                (egui::pos2(formula.width, 0.), egui::pos2(1., 0.)),
                (
                    egui::pos2(formula.width, formula.height),
                    egui::pos2(1., 1.),
                ),
                (egui::pos2(0., formula.height), egui::pos2(0., 1.)),
            ] {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: map(p),
                    uv,
                    color: egui::Color32::WHITE,
                });
            }
            mesh.indices = vec![0, 1, 2, 0, 2, 3];
            painter.add(egui::Shape::mesh(mesh));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_computer_modern_contains_math_metrics_and_required_glyphs() {
        let count = u16::from_be_bytes([MATH_FONT[4], MATH_FONT[5]]) as usize;
        assert!((0..count).any(|i| &MATH_FONT[12 + i * 16..16 + i * 16] == b"MATH"));
        let font = MathFont::from_bytes(MATH_FONT).unwrap();
        for ch in ['𝑥', 'α', 'β', '∫', '∑', '√', '(', ')'] {
            assert!(
                font.glyph(ch).is_ok(),
                "Missing Computer Modern glyph: {ch}"
            );
        }
    }

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
            assert!(out.svg.contains("<path"), "{source}");
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

#[cfg(test)]
mod quality_tests {
    use super::*;
    #[test]
    fn physical_pixel_resolution_and_cache_cover_zoom_dpi_and_resize() {
        let ctx = egui::Context::default();
        let formula = render(
            r"\int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}",
            [0, 0, 0, 255],
        )
        .unwrap();
        let mut texture = FormulaTexture::default();
        for dpi in [1.0, 2.0] {
            for zoom in [0.5, 1.0, 2.0, 4.0, 8.0] {
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    texture.paint(ui.painter(), &formula, dpi * zoom, true, &mut |p| p * zoom)
                });
                let requested = (dpi * zoom * 2.0).log2().ceil().exp2();
                let max_side = ctx.input(|i| i.max_texture_side).min(4096) as f32;
                let limit = (max_side / formula.width.max(formula.height))
                    .min((4_194_304.0 / (formula.width * formula.height)).sqrt());
                assert_eq!(texture.density, requested.min(limit));
                for delta in output.textures_delta.set.values() {
                    for delta in delta {
                        assert!(delta.image.size().iter().all(|n| *n <= max_side as usize));
                    }
                }
                output.textures_delta.clear();
                let mut same = ctx.run_ui(Default::default(), |ui| {
                    texture.paint(ui.painter(), &formula, dpi * zoom, true, &mut |p| p * zoom)
                });
                assert!(
                    same.textures_delta.set.is_empty(),
                    "unchanged frame rerasterized"
                );
                same.textures_delta.clear();
            }
        }
        assert!(formula.svg.contains("<path"));
        assert!(!formula.svg.contains("<text"));
        assert!(!formula.svg.contains("<image"));
    }
    #[test]
    #[ignore = "writes a local typography review sheet"]
    fn typography_review_sheet() {
        let sources = [
            r"E=mc^2",
            r"\alpha+\beta=\gamma\quad\sin x+\cos x",
            r"\frac{-b\pm\sqrt{b^2-4ac}}{2a}",
            r"\sum_{i=1}^n i^2=\frac{n(n+1)(2n+1)}{6}",
            r"\int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}",
            r"\left(\frac{\frac{a}{b}}{\sqrt{x_1^2+y_2^2}}\right)",
            r"\begin{bmatrix}a&b\\c&d\end{bmatrix}",
        ];
        let mut sheet = resvg::tiny_skia::Pixmap::new(1600, 1400).unwrap();
        sheet.fill(resvg::tiny_skia::Color::WHITE);
        for (i, source) in sources.iter().enumerate() {
            let f = render(source, [20, 20, 20, 255]).unwrap();
            let tree = resvg::usvg::Tree::from_str(&f.svg, &Default::default()).unwrap();
            let k = 2.0 * f.width / tree.size().width();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(k, k)
                    .post_translate(40., 25. + i as f32 * 190.),
                &mut sheet.as_mut(),
            );
        }
        let mut ppm = format!("P6\n{} {}\n255\n", sheet.width(), sheet.height()).into_bytes();
        for p in sheet.pixels() {
            ppm.extend([p.red(), p.green(), p.blue()]);
        }
        std::fs::write("/tmp/xitoc-math-review.ppm", ppm).unwrap();
    }
}
