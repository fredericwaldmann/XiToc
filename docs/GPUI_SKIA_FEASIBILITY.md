# Desktop GPU composition checkpoint (issue #77)

## Decision needed

GPUI Kit v0.11.0 pins gpui-pre 0.3.14 (Zed snapshot f126950). A custom
canvas element can request layout, receive input and issue GPUI drawing
commands. To display a Skia frame, the compositor must also sample the
producer's texture at the element's scene position, apply the clip and retain
the allocation until the GPU has finished reading it. The public APIs examined
do not provide this operation across XiToc's desktop targets.
[Kit release manifest](https://github.com/longbridge/gpui-kit/blob/v0.11.0/Cargo.toml)

| Platform | Source finding in 0.3.14 | Consequence |
| --- | --- | --- |
| Linux | The WGPU batch dispatcher has an empty arm for surfaces. The public surface constructor is Apple-only. | A Skia Vulkan render target cannot be submitted as a Kit canvas surface under Wayland or X11 using this API. [WGPU dispatcher](https://docs.rs/crate/gpui-pre-wgpu/0.3.14/source/src/wgpu_renderer.rs) |
| Windows | The DirectX renderer dispatches to `draw_surfaces`, which returns success without drawing. | A shared DirectX texture still needs an import and draw operation in GPUI. [DirectX renderer](https://docs.rs/crate/gpui-pre-windows/0.3.14/source/src/directx_renderer.rs) |
| macOS | The source accepts a CoreVideo pixel buffer. Metal checks for full-range bi-planar 4:2:0 YCbCr before creating textures for the two planes. | A normal RGBA Skia Metal surface does not meet this format check. Video chroma subsampling and the absence of an alpha plane matter for colored ink, glyphs and overlays. [Metal renderer](https://docs.rs/crate/gpui-pre-apple/0.3.14/source/src/metal_renderer.rs) |

The public image path reads frame bytes into the sprite atlas on cache miss.
The atlas can reuse a static image. Supplying a new full-frame image as the
camera or stroke changes would require reading back and uploading its pixels.
The optional gpui-fast 0.1.19 surface element and the earlier Kit 0.10.0 default
(gpui-pre 0.3.12) also expose only the Apple CoreVideo surface source.
[Window image and surface APIs](https://docs.rs/crate/gpui-pre/0.3.14/source/src/window.rs)

Ganesh and Graphite change how Skia produces GPU work. Neither adds the missing
consumer operation in Kit. Drawing into a native child window introduces
platform-specific clipping, stacking and input ownership, particularly when
popovers overlap the canvas. A supported upstream scene primitive would need
resource import, synchronization and retirement, premultiplied RGBA, clip and
order semantics, resize and device-loss handling. Verify these with a changing
crossed-stroke image behind a Kit popover before porting the editor.

If Skia is the priority, the existing Glow host offers a narrower next
experiment with a paint callback and a Ganesh GL context sharing the current
context. It needs validation of borrowed framebuffer lifetime, framebuffer
origin, clipping, GL state restoration and DPI. If Kit is the priority, its
own paths, text and images are available for a shared page and board renderer.
Either choice changes the requested stack and requires a product decision.

## Existing behavior and migration inventory

- A single Rust application hosts document tabs and editors. Preserve
  workspace-relative identity, draggable tabs and overflow, native child
  dialogs, focus, settings, file choosers and dirty-document close guards.
- Page Notes use page-local coordinates and paper clipping. Whiteboards have
  an unrestricted camera. Both use `Canvas::paint`. The indexed, sorted
  visible-object query currently applies to the Whiteboard under stable
  geometry. Transform previews have separate conservative handling.
- Raw samples, pressure, smoothing, caps and transforms must remain editable.
  `ink_mesh.rs` uses i_overlay and triangulation for stroke coverage. Preserve
  single-stroke alpha at self-crossings and independent blending between strokes.
- Preserve connectors, rich text and sticky notes, images and attachments,
  file-backed listings, coordinate plots and their TikZ editor, Latin Modern
  Math equations, shared selection, nested groups, locking, clipboard and undo.
- Kurbo is already transitive. Keep f64 geometry and the existing smoothing
  implementation as the comparison point. Benchmark curve bounds and flattening
  before adopting a direct dependency. Compare rstar with the current bounds
  hierarchy under both repeated queries and edits. No head-to-head results yet.
- Keep lopdf and pdf-writer for structural inspection and blank-note round
  trips. External content rendering, full ink persistence and editable PDF
  annotation round trips require additional implementation. PDFium is a
  candidate for worker-side page/tile rendering, subject to binary and
  third-party license review. Evaluate Poppler and MuPDF licensing before a
  distribution decision. Fidelity, packaging, resource costs and annotation
  compatibility remain untested. [Current file format](PDF_FORMAT.md)

## Reproducing the reference workload

Run `cargo test --locked --release highlighter_pan_profile -- --ignored --nocapture --test-threads=1`.
This exercises the existing CPU paint and egui tessellation paths for both
surface types. The finite-page workload clips to an A4 paper footprint, uses
an 1100 × 680 viewport, and measures 100 warm frames after two initial frames.
The tests exclude GPU submission, display latency, file I/O and application
chrome. They do not establish Skia throughput or platform parity.
