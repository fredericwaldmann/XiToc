//! Math layout and original Latin Modern Math outlines; physical-pixel rasterization is cached separately.
use latex_rust::{
    Color, Dim, MathFont, MathStyle, ParseOptions, SvgOptions, layout_with_em_size_pt,
    parse_with_options, render_svg_with_metrics, tokenize,
};
use std::sync::OnceLock;
pub const MAX_PARSE_DEPTH: usize = 32;
const MATH_FONT: &[u8] = include_bytes!("../assets/fonts/latinmodern-math.otf");
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
    tree: Option<resvg::usvg::Tree>,
    high_quality: bool,
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
        let max_side = painter.ctx().input(|i| i.max_texture_side).clamp(1, 4096) as u32;
        let target = texture_density(formula, density, max_side);
        if self.texture.is_none() || self.density != target || self.high_quality != high_quality {
            if self.tree.is_none() {
                self.tree =
                    resvg::usvg::Tree::from_str(&formula.svg, &resvg::usvg::Options::default())
                        .ok();
            }
            if let Some(tree) = &self.tree
                && let Some(image) = rasterize(tree, formula, target, max_side, high_quality)
            {
                self.texture = Some(painter.ctx().load_texture(
                    "formula-outline",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
                self.density = target;
                self.high_quality = high_quality;
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
const MAX_SAMPLES: u32 = 4_194_304;

fn texture_density(formula: &RenderedFormula, density: f32, max_side: u32) -> f32 {
    let bucket = ((density.max(0.125).log2() * 2.).ceil() * 0.5).exp2();
    bucket
        .min(max_side as f32 / formula.width.max(formula.height))
        .min((MAX_SAMPLES as f32 / (formula.width * formula.height)).sqrt())
}

fn rasterize(
    tree: &resvg::usvg::Tree,
    formula: &RenderedFormula,
    density: f32,
    max_side: u32,
    high_quality: bool,
) -> Option<egui::ColorImage> {
    let width = (formula.width * density).ceil().clamp(1., max_side as f32) as u32;
    let height = (formula.height * density).ceil().clamp(1., max_side as f32) as u32;
    let height = height.min(MAX_SAMPLES / width);
    // Resolve coverage with an area filter before upload. Bilinear texture
    // filtering alone only samples four texels, even under strong minification.
    let factor =
        if high_quality && width * height <= MAX_SAMPLES / 4 && width.max(height) <= max_side / 2 {
            2
        } else {
            1
        };
    let stride = width * factor;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(stride, height * factor)?;
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::from_scale(
            stride as f32 / tree.size().width(),
            (height * factor) as f32 / tree.size().height(),
        ),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap.data();
    let pixels = if factor == 1 {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .map(|&[r, g, b, a]| egui::Color32::from_rgba_premultiplied(r, g, b, a))
            .collect()
    } else {
        let mut pixels = Vec::with_capacity((width * height) as usize);
        let rows = rgba.chunks_exact((4 * stride) as usize);
        let mut rows = rows;
        while let (Some(top), Some(bottom)) = (rows.next(), rows.next()) {
            for (a, b) in top.as_chunks::<8>().0.iter().zip(bottom.as_chunks::<8>().0) {
                let [r, g, b, a] = std::array::from_fn(|c| {
                    ((u16::from(a[c])
                        + u16::from(a[c + 4])
                        + u16::from(b[c])
                        + u16::from(b[c + 4])
                        + 2)
                        / 4) as u8
                });
                pixels.push(egui::Color32::from_rgba_premultiplied(r, g, b, a));
            }
        }
        pixels
    };
    Some(egui::ColorImage::new(
        [width as usize, height as usize],
        pixels,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formula_coverage_is_resolved_at_bounded_physical_density() {
        let formula = render(r"\frac{\alpha_1}{\sqrt{x^2+1}}", [32, 40, 48, 255]).unwrap();
        let tree = resvg::usvg::Tree::from_str(&formula.svg, &Default::default()).unwrap();
        for zoom in [0.5, 1., 2., 4., 8.] {
            for dpi in [1., 1.25, 2.] {
                let density = texture_density(&formula, zoom * dpi, 4096);
                assert!(density >= zoom * dpi * 0.999);
                let image = rasterize(&tree, &formula, density, 4096, true).unwrap();
                assert!(image.pixels.len() <= MAX_SAMPLES as usize);
                assert!(image.pixels.iter().any(|p| (1..255).contains(&p.a())));
                assert!(
                    image
                        .pixels
                        .iter()
                        .all(|p| p.r() <= p.a() && p.g() <= p.a() && p.b() <= p.a())
                );
            }
        }
    }

    #[test]
    fn bundled_latin_modern_math_contains_math_metrics_and_required_glyphs() {
        let count = u16::from_be_bytes([MATH_FONT[4], MATH_FONT[5]]) as usize;
        assert!((0..count).any(|i| &MATH_FONT[12 + i * 16..16 + i * 16] == b"MATH"));
        let font = MathFont::from_bytes(MATH_FONT).unwrap();
        for ch in ['𝑥', 'α', 'β', '∫', '∑', '√', '(', ')'] {
            assert!(
                font.glyph(ch).is_ok(),
                "Missing Latin Modern Math glyph: {ch}"
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
    #[ignore = "writes a local CPU formula coverage comparison"]
    fn formula_zoom_comparison() {
        let formula = render(r"\frac{\alpha_1+\sqrt{x^2+1}}{2}", [24, 32, 40, 255]).unwrap();
        let tree = resvg::usvg::Tree::from_str(&formula.svg, &Default::default()).unwrap();
        let zooms = [0.5_f32, 1., 2., 4., 8.];
        let pad = 12;
        let width = (formula.width * 8.).ceil() as u32 + 2 * pad;
        let height = zooms
            .iter()
            .map(|z| (formula.height * z).ceil() as u32 + 2 * pad)
            .sum();
        let mut sheet = image::RgbImage::from_pixel(width * 2, height, image::Rgb([255; 3]));
        let mut top = 0;
        for zoom in zooms {
            let old = rasterize(
                &tree,
                &formula,
                (zoom * 2.).log2().ceil().exp2(),
                4096,
                false,
            )
            .unwrap();
            let new = rasterize(
                &tree,
                &formula,
                texture_density(&formula, zoom, 4096),
                4096,
                true,
            )
            .unwrap();
            let bounds = [formula.width * zoom, formula.height * zoom];
            for (column, image) in [old, new].iter().enumerate() {
                let pixel = |x: f32, y: f32| {
                    let x = (x as isize).clamp(0, image.size[0] as isize - 1) as usize;
                    let y = (y as isize).clamp(0, image.size[1] as isize - 1) as usize;
                    image.pixels[y * image.size[0] + x].to_array()
                };
                for row in 0..bounds[1].ceil() as u32 {
                    for col in 0..bounds[0].ceil() as u32 {
                        let x = (col as f32 + 0.5) * image.size[0] as f32 / bounds[0] - 0.5;
                        let y = (row as f32 + 0.5) * image.size[1] as f32 / bounds[1] - 0.5;
                        let ix = x.floor();
                        let iy = y.floor();
                        let mix = |a: f32, b: f32, t: f32| a + (b - a) * t;
                        let rgba: [f32; 4] = std::array::from_fn(|i| {
                            let a = mix(
                                pixel(ix, iy)[i] as f32,
                                pixel(ix + 1., iy)[i] as f32,
                                x - ix,
                            );
                            let b = mix(
                                pixel(ix, iy + 1.)[i] as f32,
                                pixel(ix + 1., iy + 1.)[i] as f32,
                                x - ix,
                            );
                            mix(a, b, y - iy)
                        });
                        let rgb = std::array::from_fn(|i| (rgba[i] + 255. - rgba[3]).round() as u8);
                        sheet.put_pixel(
                            column as u32 * width + pad + col,
                            top + pad + row,
                            image::Rgb(rgb),
                        );
                    }
                }
            }
            top += bounds[1].ceil() as u32 + 2 * pad;
        }
        sheet
            .save("/tmp/xitoc-formula-zoom-comparison.png")
            .unwrap();
    }
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
                let max_side = ctx.input(|i| i.max_texture_side).clamp(1, 4096) as u32;
                let dimensions: Vec<_> = output
                    .textures_delta
                    .set
                    .values()
                    .flatten()
                    .map(|delta| delta.image.size())
                    .collect();
                output.textures_delta.clear();
                assert_eq!(
                    texture.density,
                    texture_density(&formula, dpi * zoom, max_side)
                );
                assert!(dimensions.iter().flatten().all(|n| *n <= max_side as usize));
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
