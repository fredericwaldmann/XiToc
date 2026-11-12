# M1.13 math renderer evaluation

## Decision

XiToc uses `latex-rust` 3.0.3 for math parsing, TeX-style layout and vector shape
generation, with its bundled STIX Two Math font. It is MIT OR Apache-2.0. The bundled
STIX Two Math font is SIL OFL 1.1; the crate's color-name table follows LPPL-1.3c-or-later
and carries upstream attribution in its NOTICE. There is no system TeX, shell process,
service or network rendering dependency. Font initialization is lazy and shared.

The optional upstream egui adapter emits egui 0.36.2 geometry; XiToc uses egui 0.37.0.
XiToc converts mesh/line/rect primitives at the renderer boundary. Both egui versions
therefore compile into the application. Version 3.0.3 requires Rust 1.99; this raised
the declared MSRV from 1.95. Native and WASM compilation are required gates for this
integration. The feature is vector-based, preserving sharp zoom and offering a future
path to PDF paths, although the current M1 PDF codec does not serialize formulas.

## Candidates considered

| Candidate | License / output | Evaluation |
| --- | --- | --- |
| [LaTeX-Rust](https://github.com/jscarr64/latex-rust) | MIT OR Apache-2.0; math AST, TeX layout, vector shapes and optional egui output | Selected: native/WASM-compatible pure Rust math renderer, bundled math font and geometry suitable for existing painter/PDF direction. Limitation: math mode only, no full LaTeX documents/packages; egui 0.36 adapter adds conversion and duplicate dependency. |
| [KaTeX Rust](https://github.com/katex-rs/katex-rs) | MIT; HTML and MathML | Rejected: browser-oriented output requires a separate native layout/rendering stack and would produce inconsistent native/web paths. |
| [tex2math](https://github.com/twn39/tex2math) | LGPL-3.0-only; MathML | Rejected: parser output requires platform-specific MathML renderers and does not directly provide shared vector geometry. |
| [Typst](https://docs.rs/typst/latest/typst/) | Apache-2.0; Typst language and vector/PDF output | Rejected: not original LaTeX syntax, broad compiler footprint, and conversion would not preserve/edit the exact requested source semantics. |

Primary API/license references: [LaTeX-Rust docs](https://docs.rs/latex-rust/latest/latex_rust/),
[LICENSE-MIT](https://github.com/jscarr64/latex-rust/blob/main/LICENSE-MIT),
[LICENSE-APACHE](https://github.com/jscarr64/latex-rust/blob/main/LICENSE-APACHE),
[NOTICE](https://github.com/jscarr64/latex-rust/blob/main/NOTICE).

## Supported subset and limits

The math-mode parser supports common variables, numbers, operators, fractions, roots,
superscripts/subscripts, Greek and relation symbols, accents, large operators,
matrices/arrays, cases and aligned equations. The automated fixtures cover fractions,
roots, scripts, summation, integral, limit, bmatrix, cases and aligned equations.
Commands requiring arbitrary package definitions, document preambles, text-mode prose,
TikZ, chemistry or custom macros are not supported. Unknown/malformed expressions are
reported without replacing the user's source or the last valid preview.

The application bounds input to 16 KiB and 128 math tokens and configures parser
nesting depth to 32. Rendering is synchronous, not dispatched to a worker; bounded
inputs and per-object source/color caches avoid reparsing unchanged formulas, but a
large valid expression still occupies the UI thread during its parse/layout. The
release stress fixture measures the maximum accepted token expression before any
future increase to this bound.

On this development host, a generated 2,047-token expression took 223 ms in an
optimized build. After reducing the guard to 128 tokens, the release stress fixture's
127-token additive chain rendered in 17.19 ms. This is one host/compiler measurement,
not a cross-platform latency guarantee; rendering remains synchronous.

## Verification record

Native and WASM app builds and the Trunk release package pass. The observed raw Trunk
distribution is 17 MB (WASM), before HTTP compression; this is a material cost of the
math/font and duplicate-egui dependency and should be watched. Renderer output is
converted directly to painter meshes/lines/rectangles, not a bitmap, so object zoom
does not resample a texture. Browser smoke inspection verified the toolbar tool, live
`E=mc^2` preview, commit and double-click source reopening at 100%; the full zoom,
theme, Page Note and high-complexity visual matrix remains manual follow-up.
