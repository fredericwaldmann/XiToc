# M1.13 math renderer evaluation

## Investigation and decision

The original display path already used the genuine STIX Two Math font and
`latex-rust` 3.0.3 OpenType MATH layout. It did not use UI fonts, fallback glyphs,
or a low-resolution image cache. Inspection of the installed upstream source
identified two display defects: the egui adapter emits raw filled triangle meshes
without antialiased contour fringes, and its outline tessellator flattens every
curve to exactly eight segments regardless of magnification. XiToc then scales
those meshes with object/camera transforms. These defects make small math strokes
look uneven and enlarged curves faceted, despite valid math layout/font metrics.
The adapter also required a second egui version (0.36 alongside XiToc's 0.37).

Retain the bounded parser/layout engine, replace its egui adapter with
`render_svg_with_metrics` and `resvg` 0.50.0. Original Bézier glyph outlines remain
vector data; display uses antialiased textures regenerated from those paths for
physical pixel density, zoom and object resizing. Ink-containing SVG metrics fix
overhang/clipping and supply the matching baseline. The duplicate egui dependency
is removed. Shared native/WASM rendering has no system TeX, network, JavaScript
math engine or platform font requirement.

The raster display is a deliberate tradeoff: egui's built-in painter does not fill
arbitrary compound Bézier glyph paths with holes and robust antialiasing. Using a
mature pure-Rust SVG rasterizer avoids maintaining a custom glyph tessellator.
Bucketed textures cost bounded per-object memory and occasional resize/zoom
rasterization; original SVG is retained. Rendering remains synchronous with existing
source/token/depth bounds. Extreme sizes reach documented texture limits.

## Alternatives

| Candidate | Assessment |
| --- | --- |
| Current LaTeX-Rust egui meshes | Genuine math layout, but fixed outline approximation and un-antialiased edges; replaced display adapter. |
| LaTeX-Rust SVG + resvg | Selected: original outlines, self-contained output, shared Rust native/WASM path, mature antialiasing. |
| Latin Modern Math | Genuine math font, but no need to replace STIX's compatible bundled MATH metrics; defer another preset until all glyph/assembly fixtures are verified with it. |
| STIX Two Math | Selected, bundled reliable coverage and tested MATH constants/italic corrections/variants; no UI-font substitution. |
| KaTeX/MathJax HTML or MathML | Would need separate browser/native layout integration; no shared egui drawing path. |
| Typst | Larger compiler footprint and a different source language; conversion creates additional syntax/semantic maintenance. |
| Full TeX installation | Violates no-system-installation requirement and complicates WASM/offline distribution. |

## Licenses and sources

LaTeX-Rust is MIT OR Apache-2.0; resvg is Apache-2.0 OR MIT. STIX Two Math is
SIL OFL 1.1. Font/license and upstream color-table notice are distributed under
`assets/licenses/`. Font bytes are embedded unmodified by LaTeX-Rust. Its color
name table carries LPPL-1.3c-or-later attribution in the upstream NOTICE.

Primary references: [LaTeX-Rust API](https://docs.rs/latex-rust/3.0.3/latex_rust/),
[SVG metrics](https://docs.rs/latex-rust/3.0.3/latex_rust/fn.render_svg_with_metrics.html),
[resvg](https://github.com/linebender/resvg),
[STIX project](https://github.com/stipub/stixfonts).
Local upstream source files inspected: `font/math.rs`, `layout/engine.rs`,
`render/egui/emit.rs`, `render/egui/tessellate.rs`, `render/svg/render.rs`,
`render/svg/outline.rs`, `fonts/stix-two-math/OFL.txt`, and `NOTICE`.

See [LATEX_CONFIGURATION.md](LATEX_CONFIGURATION.md) for configuration semantics,
unsupported commands, limits, caching, fallback and persistence boundaries.
See [TEST_PLAN.md](TEST_PLAN.md) for executed checks and manual limits.
