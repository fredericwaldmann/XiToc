# XiToc dependency audit and build optimization

Baseline `39303e7` on `main`, immediately after the thick-ink performance merge.
This repository has one Cargo package and no workspace members, separate build
dependencies, or separate dev dependencies. Trunk 0.22.0 packages the same
editor for the browser. The native and web CI jobs select their own targets.

## Executive summary

| Count | Before | After |
| --- | ---: | ---: |
| Dependency package versions in Cargo.lock | 382 | 376 |
| Direct package dependencies across all targets | 23 | 23 |
| Transitive package versions in Cargo.lock | 359 | 353 |
| Linux x86-64 normal/build graph | 237 | 231 |
| Windows x86-64 MSVC normal/build graph | 201 | 195 |
| macOS arm64 normal/build graph | 213 | 207 |
| Browser wasm32 normal/build graph | 176 | 171 |
| Duplicate crate names in Cargo.lock | 31 | 26 |
| Extra versions beyond one per name | 33 | 28 |

Counts exclude the XiToc package itself. The inventory includes its row so that
it matches the lockfile exactly. The audited Cargo.lock SHA-256 is
`10033dd3e16278557073c06a9cf6dcf5ad7f9c307c86da870f4d77fd13a3f2ef`.
Direct means declared in XiToc's own manifest,
even when the same version also has a transitive parent. A target count includes
distinct package versions reachable through Cargo's normal and build edges,
including host tools such as derive macros. Windows and macOS are graph
inspections for the stated triples. Linux and WASM artifact counts are in the
build measurements below. Repeated Cargo compilation units and repeated lines
in the dependency tree are counted separately from package versions.

All 23 direct libraries have application uses. The cleanup removes one redundant
feature declaration and six package versions associated with the older SHA-256
implementation. XiToc now uses the SHA-256 version already required by its PDF
parser. The asset ID, Base64 encoding, and serialized bytes have a fixed-vector
regression test.

Of the 376 remaining dependency versions, 327 are reachable in the all-target
normal/build tree and 49 appear only in the lock resolution for this configuration.
Cargo can resolve optional dependencies while preparing the lockfile even when
it leaves them out of the active build. The WGPU renderer and TIFF decoder are
examples. Both are absent from the measured compiler artifacts.

## How to read the full inventory

[DEPENDENCY_INVENTORY.csv](DEPENDENCY_INVENTORY.csv) has every name and version
in the audited Cargo.lock. **Introduced By** names the direct Cargo branches
that can reach the package. **Evidence or Notes** names immediate parents and
the earliest commit in the inspected history that recorded that exact version.
A first lock entry records a resolution event. The history section explains the
product work that accompanied it. Dependencies can be shared by multiple
features and target builds.

**Target Applicability** labels the build that needs the version, including host
code generators. **Enabled Features** records the union of feature names printed
by Cargo for that package in that target tree. Cargo can build different units
of the same version with different host and target feature sets. **Dependency
Kind** records incoming Cargo edge kinds and labels procedural macros and
packages with build scripts. A normal incoming edge can support a host tool.

Descriptions come from local upstream package metadata, with hand-written
explanations for the direct and commonly encountered crates. Rows outside the
four selected targets and duplicate families are flagged for review. Keep means
the current graph provides a reason for inclusion. Optional upstream APIs
remain subject to the selected features.

Regenerate with `python3 scripts/dependency_inventory.py` using Python 3.11+
and the pinned Cargo sources available locally. Populate the cache with
`cargo fetch --locked` first if needed. The generator checks the exact
name/version set against the lockfile before writing. Verify a repeat run with
`python3 scripts/dependency_inventory.py --output /tmp/dependency-inventory.csv`
and `cmp docs/DEPENDENCY_INVENTORY.csv /tmp/dependency-inventory.csv`.

## What XiToc uses directly

| Direct package | Category and platform | Use and audit decision |
| --- | --- | --- |
| egui | GUI, all | Editor widgets, events, layout, and drawing. Keep the direct API import and remove the redundant explicit `web_file` feature. eframe's WASM-specific edge enables it where needed. |
| eframe | GUI, all | Native window or browser canvas, repaint, rendering, and local preferences. Keep Glow, default fonts, persistence, X11, and Wayland. WGPU is inactive in ordinary builds. |
| i_overlay | Rendering, all | Union an ink stroke's outlines so self-overlap shares one opacity. Keep. |
| earcutr | Rendering, all | Turn vector outline regions and holes into triangles for the renderer. Keep. |
| image | Image handling, all | Decode PNG, JPEG, and WebP and normalize clipboard images. Keep the existing explicit codec set. |
| fontdb | Fonts, native | Find installed fonts and obtain their bytes for rich text. Keep filesystem and platform font discovery. |
| unicode-segmentation | Text, all | Move cursors and edit complete user-visible characters. Keep. |
| latex-rust | LaTeX, all | Parse and lay out math with the bundled font. Keep `std`; its optional old egui adapter is inactive. |
| resvg | LaTeX, all | Parse formula SVG outlines and produce antialiased textures. Keep default features off; ordinary rich text has its own font path. |
| rfd | File management, all | Native file/folder dialogs and browser import/export. Keep Linux `xdg-portal` and the browser file-handle API. |
| cap-std | File management, native | Restrict filesystem operations to the selected workspace capability. Keep. |
| tempfile | File management, native | Attachment staging, export, and test fixtures. Keep as a production dependency; attachments promoted it from test-only use. |
| lopdf | Document, all | Read PDF structure and embedded metadata. Keep defaults off and retain browser entropy support. |
| pdf-writer | Document, all | Create valid blank PDFs and their metadata attachments. Keep the small writer and the existing read/write separation. |
| serde | Serialization, all | Derive model serialization and share immutable samples and assets through Arc. Keep `derive` and `rc`. |
| serde_json | Serialization, all | Read and write model JSON. Keep `float_roundtrip` for precise coordinates. |
| uuid | Documents, all | Create stable random document and object identifiers. Keep v4, Serde, and browser randomness. |
| sha2 | Documents, all | Content-address original attachment and image bytes. Keep and align to lopdf's existing 0.11 version. |
| base64 | Documents, all | Encode binary asset bytes in JSON clipboard data. Keep the interoperable wire format. |
| js-sys | WASM | Read JavaScript buffers and arrays and set browser state. Keep. |
| wasm-bindgen | WASM | Connect Rust entry points and pen and clipboard listeners to JavaScript. Keep. |
| wasm-bindgen-futures | WASM | Await browser file promises and start async UI work. Keep. |
| web-sys | WASM | Browser document, canvas, clipboard, and file APIs. Keep explicit interface features; Cargo unifies upstream requests. |

The inspection traced source references, including conditional imports, derive
macros, qualified calls and uses inside Serde field adapters. Cargo supports the
target graph inspection here. Cargo-machete was unavailable locally, so its
heuristic output is not part of the evidence. There was no confirmed unused
direct package.

## The largest chains

Counts are distinct reachable dependency versions **including the listed direct
package**, across the all-target normal/build tree after cleanup. Chains
overlap; adding the numbers would count shared packages more than once.

| Direct root | Reach | Why this chain exists |
| --- | ---: | --- |
| eframe | 245 | Window creation, input, OpenGL/WebGL, text drawing, and preferences across operating systems. |
| egui | 85 | UI state, text shaping, fonts, vector rendering, and browser file integration. Shared substantially with eframe. |
| lopdf | 73 | PDF parsing and streams, decompression, and PDF cryptography support. |
| resvg | 40 | Turn formula SVGs into pixels, including XML, geometry, and color processing. |
| rfd | 39 | Native dialog backends and the browser file chooser. |
| cap-std | 34 | Scoped directory operations and operating-system bindings. |
| tempfile | 32 | Secure temporary files and compatible native filesystem primitives. |
| image | 28 | PNG, JPEG, WebP, pixel conversion, and color profile handling. |
| uuid | 25 | Platform random bytes, identifier generation, and serialization. |
| wasm-bindgen-futures | 20 | Rust-to-JavaScript promise bridging and generated browser bindings. |

A large chain can consist of many small packages and mutually exclusive OS
bindings. Build timing is a separate question. The development/build category
includes code generators such as `syn` and `quote`, build scripts such as
`cc` and `pkg-config`, and the host half of browser binding generation. The
shared infrastructure category covers utilities such as small collections,
logging, flags, synchronization, and platform bindings. These are often
shared between a GUI, file, and document chain.

## Target and feature decisions

- Linux uses winit/glutin with X11 and Wayland and a native portal file chooser.
  Windows uses Win32 bindings, and macOS uses AppKit/Objective-C bindings.
  Browser builds exclude winit, glutin, installed font discovery, and native
  attachment staging. Glow runs through OpenGL on native and WebGL on the web.
- The explicit eframe defaults are already narrow. Adding upstream defaults
  would activate additional systems, including a second rendering backend.
  Removing X11 or Wayland to speed builds would reduce supported desktops.
  An opt-in single-display-backend developer build would need maintenance and
  measurement before consideration.
- The resolved image features include the three user-supported codecs and no
  TIFF, AVIF, or all-formats feature in the inspected builds. The image library
  also supports the eframe icon path. Keep interoperability. Resvg's optional
  text, raster-image, and system-font features are already disabled in XiToc.
- The eframe web manifest already requests `egui/web_file`, whose browser
  API is also guarded by the WASM target. Dropping the root request removes
  it from the three desktop graphs and leaves the WASM feature set intact.
  It removes no package versions on its own.
- Some shared manifest flags could be expressed as target-specific declarations,
  such as browser randomness and rfd's inner handle API. Their present target
  code already gates the browser integrations. Without an observed compile-unit
  reduction, duplicating declarations adds maintenance cost for little benefit.
- `serde_json/float_roundtrip`, font discovery features, and the native portal
  backend have concrete product behavior behind them. The editor runs locally. Some OS binding crates expose networking APIs to
  their parents as part of their larger platform interface.

## Verified dependency growth

Counts below are dependency versions in the lockfile, excluding XiToc.
The commit references identify the graph changes in Git history.

| Commit | Versions | Change |
| --- | ---: | --- |
| `ee8f3c6` | 291 | eframe desktop/browser scaffold. Most of the total was already platform and GUI infrastructure. |
| `39f571b` | 342 | Workspaces, scoped native filesystem, file picker, PDF reader/writer, UUIDs, and Serde. |
| `8978451` | 345 | Local settings persistence, including the eframe/RON path. |
| `16398b2` | 348 | Installed font discovery and grapheme-aware rich-text editing. |
| `8473efd` | 361 | Editable formulas added latex-rust and initially used a second egui version through its adapter. |
| `1456ad3` | 367 | Formula typography switched to SVG and resvg, removing the duplicate egui stack while adding SVG/vector processing. |
| `4e64b97` | 375 | Editable images activated the shared image codecs and added asset hashing and clipboard Base64. |
| `af2942f` | 375 | File attachments reused the asset and dialog layers. Tempfile moved from dev use to native runtime use without a lockfile change. |
| `33da870` | 382 | Editable ink attributes added polygon coverage union and triangulation for translucent strokes. |
| `39303e7` | 382 | Thick opaque ink optimization added no dependencies. |
| This audit | 376 | Reused the SHA-256 version already used by the PDF library. |

The web bindings were present in the scaffold. Stylus pressure reused them in
`1c77ec5`, with no lockfile change. Packaged Trunk tooling and CI's Rust build
cache are external to this Cargo graph.

## Duplicate versions and removal decisions

The older SHA-256 chain was the directly controllable overlap with compatible
call sites found here. XiToc's old branch used only SHA-256 and
the shared digest API. Fixed-vector and asset tests exercise its semantics.
The six removed package versions are `sha2 0.10.9`, `digest 0.10.7`,
`block-buffer 0.10.4`, `crypto-common 0.1.7`, `generic-array 0.14.7`,
and `cpufeatures 0.2.17`. No entire direct API was removed.
The lockfile change stays within that branch. The eliminated units totaled
6.09 seconds of baseline Cargo unit durations on Linux and 4.14 on WASM.
These overlapping durations estimate removed work independently of wall time.

The table includes all duplicate families from the baseline. Parent names
come from Cargo metadata's lock resolution, so a lock-only parent can appear.
The inventory supplies exact parent versions and target coverage.
| Crate | Locked versions and immediate requesting parents | Targets compiling multiple versions in graph | Alignment decision |
| --- | --- | --- | --- |
| `bitflags` | 1.3.2 (core-graphics, core-graphics-types, redox_syscall); 2.13.2 (android-activity, calloop, dispatch2, egui, glutin, harfrust, libredox, lopdf, naga, ndk, objc2-app-kit, objc2-cloud-kit, objc2-core-data, objc2-core-foundation, objc2-core-graphics, objc2-foundation, objc2-io-surface, objc2-metal, objc2-quartz-core, objc2-ui-kit, objc2-user-notifications, pdf-writer, png, redox_syscall, ron, rustix, smithay-client-toolkit, wayland-client, wayland-csd-frame, wayland-protocols, wayland-protocols-plasma, wayland-protocols-wlr, wgpu, wgpu-core, wgpu-hal, wgpu-types, winit, winx, xkbcommon-dl) | macOS | Upstream platform dependency ranges; allow parent releases to converge. |
| `block-buffer` | 0.10.4 (digest); 0.12.1 (digest) | Linux, Windows, macOS, WASM | Aligned through sha2; older version removed. |
| `block2` | 0.5.1 (objc2-app-kit, objc2-cloud-kit, objc2-contacts, objc2-core-data, objc2-core-image, objc2-core-location, objc2-foundation, objc2-link-presentation, objc2-metal, objc2-quartz-core, objc2-ui-kit, objc2-uniform-type-identifiers, objc2-user-notifications, winit); 0.6.2 (dispatch2, egui-winit, objc2-app-kit, objc2-foundation, rfd) | macOS | Requires winit and Apple-binding parent updates; benefit limited to Apple builds. |
| `cpufeatures` | 0.2.17 (sha2); 0.3.1 (aes, chacha20, sha2) | Linux, Windows, macOS | Aligned through sha2; older version removed. |
| `crypto-common` | 0.1.7 (digest); 0.2.2 (cipher, digest) | Linux, Windows, macOS, WASM | Aligned through sha2; older version removed. |
| `digest` | 0.10.7 (sha2); 0.11.3 (md-5, sha2) | Linux, Windows, macOS, WASM | Aligned through sha2; older version removed. |
| `foldhash` | 0.1.5 (hashbrown); 0.2.0 (glifo, hashbrown) | None of the four | At least one version is lock-only here; no two-version compile saving to pursue. |
| `font-types` | 0.10.1 (read-fonts); 0.12.7 (epaint, read-fonts) | Linux, Windows, macOS, WASM | Requires latex-rust and egui font-stack alignment; benchmark glyphs and formulas on an upstream update. |
| `getrandom` | 0.3.4 (ahash); 0.4.3 (jobserver, lopdf, rand, tempfile, uuid) | Linux | Upstream platform dependency ranges; allow parent releases to converge. |
| `hashbrown` | 0.15.5 (petgraph); 0.17.1 (glifo, indexmap, naga, naga-types, vello_cpu, wgpu, wgpu-core, wgpu-hal) | None of the four | At least one version is lock-only here; no two-version compile saving to pursue. |
| `itertools` | 0.14.0 (earcutr); 0.15.0 (egui) | Linux, Windows, macOS, WASM | earcutr and egui use adjacent incompatible ranges; wait for an upstream bump. |
| `jni-sys` | 0.3.1 (ndk, ndk-sys); 0.4.1 (jni, jni-sys) | None of the four | Upstream platform dependency ranges; allow parent releases to converge. |
| `linux-raw-sys` | 0.4.15 (rustix); 0.12.1 (rustix) | Linux | Upstream platform dependency ranges; allow parent releases to converge. |
| `miniz_oxide` | 0.8.9 (png); 0.9.1 (flate2) | Linux, Windows, macOS, WASM | Compression parents select different ranges; keep supported codecs and revisit upstream. |
| `objc2` | 0.5.3 (block2, objc2-app-kit, objc2-cloud-kit, objc2-contacts, objc2-core-data, objc2-core-image, objc2-core-location, objc2-foundation, objc2-link-presentation, objc2-metal, objc2-quartz-core, objc2-symbols, objc2-ui-kit, objc2-uniform-type-identifiers, objc2-user-notifications, winit); 0.6.5 (arboard, block2, dispatch2, eframe, egui-winit, glutin, objc2-app-kit, objc2-core-foundation, objc2-core-graphics, objc2-foundation, objc2-io-surface, objc2-ui-kit, rfd) | macOS | Requires winit and Apple-binding parent updates; benefit limited to Apple builds. |
| `objc2-app-kit` | 0.2.2 (objc2-link-presentation, winit); 0.3.2 (arboard, eframe, egui-winit, glutin, rfd) | macOS | Requires winit and Apple-binding parent updates; benefit limited to Apple builds. |
| `objc2-foundation` | 0.2.2 (objc2-app-kit, objc2-cloud-kit, objc2-contacts, objc2-core-data, objc2-core-image, objc2-core-location, objc2-link-presentation, objc2-metal, objc2-quartz-core, objc2-symbols, objc2-ui-kit, objc2-uniform-type-identifiers, objc2-user-notifications, winit); 0.3.2 (arboard, eframe, egui-winit, glutin, objc2-app-kit, objc2-ui-kit, rfd) | macOS | Requires winit and Apple-binding parent updates; benefit limited to Apple builds. |
| `objc2-ui-kit` | 0.2.2 (winit); 0.3.2 (egui-winit) | None of the four | Requires winit and Apple-binding parent updates; benefit limited to Apple builds. |
| `r-efi` | 5.3.0 (getrandom); 6.0.0 (getrandom) | None of the four | Upstream platform dependency ranges; allow parent releases to converge. |
| `read-fonts` | 0.36.0 (skrifa); 0.41.0 (harfrust, skrifa) | Linux, Windows, macOS, WASM | Requires latex-rust and egui font-stack alignment; benchmark glyphs and formulas on an upstream update. |
| `redox_syscall` | 0.4.1 (winit); 0.5.18 (parking_lot_core); 0.9.5 (libredox) | None of the four | Upstream platform dependency ranges; allow parent releases to converge. |
| `roxmltree` | 0.20.0 (fontconfig-parser); 0.21.2 (usvg) | Linux | Fontconfig parsing and SVG parsing use different ranges; update when their parents align. |
| `rustc-hash` | 1.1.0 (naga, naga-types, wgpu-core); 2.1.3 (type-map) | None of the four | At least one version is lock-only here; no two-version compile saving to pursue. |
| `rustix` | 0.38.44 (calloop, calloop-wayland-source, smithay-client-toolkit, winit); 1.1.5 (cap-primitives, cap-std, fs-set-times, gethostname, polling, rustix-linux-procfs, tempfile, wayland-backend, wayland-client, wayland-cursor, wl-clipboard-rs, x11rb) | Linux | Upstream platform dependency ranges; allow parent releases to converge. |
| `sha2` | 0.10.9 (xitoc); 0.11.1 (lopdf) | Linux, Windows, macOS, WASM | Aligned through sha2; older version removed. |
| `skrifa` | 0.39.0 (latex-rust); 0.44.0 (epaint, glifo) | Linux, Windows, macOS, WASM | Requires latex-rust and egui font-stack alignment; benchmark glyphs and formulas on an upstream update. |
| `syn` | 2.0.119 (enumn, jni-macros, jni-sys-macros, num_enum_derive, pin-project-internal, thiserror-impl, zerocopy-derive); 3.0.9 (bytemuck_derive, foreign-types-macros, serde_derive, thiserror-impl, wasm-bindgen-macro-support) | Linux, Windows, macOS, WASM | Parent macros target different APIs; avoid forcing version changes. |
| `thiserror` | 1.0.69 (calloop, ndk, smithay-client-toolkit); 2.0.21 (android-activity, egui-wgpu, jni, lopdf, naga, naga-types, vello_common, wgpu-core, wgpu-hal, wl-clipboard-rs) | Linux | Parent macros target different APIs; avoid forcing version changes. |
| `thiserror-impl` | 1.0.69 (thiserror); 2.0.21 (thiserror) | Linux | Parent macros target different APIs; avoid forcing version changes. |
| `weezl` | 0.1.12 (tiff); 0.2.1 (lopdf) | None of the four | At least one version is lock-only here; no two-version compile saving to pursue. |
| `windows-sys` | 0.52.0 (glutin, glutin_egl_sys, winit); 0.59.0 (cap-primitives, fs-set-times, io-extras, rustix, winx); 0.61.2 (arboard, eframe, egui-winit, errno, os_pipe, polling, rfd, rustix, tempfile, winapi-util) | Windows | Upstream platform dependency ranges; allow parent releases to converge. |

A duplicate costs another compilation only on a target that builds both
versions. On this machine, the font stack is the most promising future
alignment to investigate because both branches can be substantial build units.
Apple API bindings, Linux syscalls, and derive infrastructure should converge
through compatible upstream releases. Audit the pinned graph again when updating
eframe or latex-rust. The active graph and actual compile timings should drive
that choice.

## Build performance

Measurements used an Intel Core i7-6600U laptop with four logical CPUs, 14 GiB
RAM, Linux x86-64, Rust and Cargo 1.99.1, and Cargo's four-job default. The
source cache was warm and Cargo ran offline. Each revision had its own
initially empty target directory on the same local disk. The WASM target
was built after Linux in that directory, so its target artifacts were fresh
and compatible host artifacts could be reused. The paired sequences used the
same build order. The host source cache and machine are shared across runs,
and timings are single samples.

For clean timings, run `cargo build --locked --offline --release --timings
--message-format=json --target-dir DIR`, first for the native host and then
with `--target wasm32-unknown-unknown`. For the warm rebuild, touch
`src/main.rs` and repeat both invocations in the same target directory. This
measures a source-triggered Cargo rebuild with cached dependencies. Cargo's
default release profile uses whole-crate recompilation for the changed app.

| Release measurement | Before | After |
| --- | ---: | ---: |
| Linux clean wall time | 379.48 s | 328.42 s |
| WASM fresh target, warm host wall time | 277.30 s | 264.93 s |
| Linux warm source rebuild | 49.64 s | 54.46 s |
| WASM warm source rebuild | 41.62 s | 44.83 s |
| Linux dependency versions with compiler artifacts | 237 | 231 |
| WASM dependency versions with compiler artifacts | 176 | 171 |
| Linux artifact units, including XiToc and build scripts | 282 | 275 |
| WASM artifact units, including XiToc and build scripts | 202 | 196 |
| Linux dirty units in Cargo timing report, including script execution | 323 | 315 |
| WASM dirty units in Cargo timing report | 193 | 187 |

Compiler artifact counts include reused host artifacts. WASM rebuilt 166
dependency versions before and 161 after. The warm source rebuilds recompiled
only XiToc and took 4.82 seconds longer on Linux and 3.21 seconds longer on WASM
in these samples. Clean elapsed time was 51.06 seconds lower on Linux and 12.37
seconds lower on WASM in this pair. Unchanged dependency units also varied
between runs. The verified optimization is the eliminated compile work, and
the wall-time deltas are measurements from this machine rather than a
portable speedup estimate.

A separate warm run used `cargo rustc --release -- -C linker=WRAPPER` with
the same release target directory and the same default linker inside a
`time` wrapper. The linker invocations took 0.35 seconds with native `cc`
and 0.50 seconds with WASM `rust-lld`. These are spot measurements of the
final link commands. The cold Cargo timing unit for XiToc includes code
generation and linking and took 49.54 → 45.87 seconds on Linux and
40.61 → 43.14 seconds on WASM.

The ten longest dependency units in the baseline Cargo timings follow. Several
ran at the same time, so adding these durations would overstate wall time.

| Rank | Linux package and elapsed seconds | WASM package and elapsed seconds |
| --- | --- | --- |
| 1 | egui 0.37.0 · 93.51 | egui 0.37.0 · 75.14 |
| 2 | x11rb-protocol 0.13.2 · 66.74 | lopdf 0.45.0 · 71.84 |
| 3 | lopdf 0.45.0 · 60.03 | web-sys 0.3.106 · 47.61 |
| 4 | vello_cpu 0.2.0 · 56.67 | read-fonts 0.41.0 · 39.62 |
| 5 | image 0.25.10 · 52.34 | vello_cpu 0.2.0 · 39.59 |
| 6 | eframe 0.37.0 · 49.26 | image 0.25.10 · 38.53 |
| 7 | read-fonts 0.41.0 · 46.94 | read-fonts 0.36.0 · 34.98 |
| 8 | moxcms 0.8.1 · 40.88 | pxfm 0.1.30 · 29.33 |
| 9 | read-fonts 0.36.0 · 39.55 | skrifa 0.44.0 · 27.67 |
| 10 | wayland-protocols 0.32.13 · 39.53 | harfrust 0.12.0 · 24.59 |

Most of the cost sits in GUI, protocol bindings, fonts, PDF and image
processing. Build-script execution was small. The longest baseline execution
was the Unicode category data generator at 0.59 seconds on Linux and 0.56 on
WASM. Building the largest script took 1.15 seconds (Linux libc) and
0.22 seconds (WASM wasm-bindgen-shared).

## Recommendations

- **Keep** the selected platform backends, PDF reader/writer split, glyph and
  formula libraries, scoped file access, and PNG/JPEG/WebP support. Each serves
  an exercised part of the product.
- **Remove** the older SHA-256 chain and the root-level browser-file feature
  request already supplied by eframe on WASM. These are the changes in this PR.
- **Optimize** repeated builds by keeping `target` warm and using the existing
  CI Rust cache. Use `cargo check` for ordinary type checking and release builds
  to profile runtime or package a release. Measure job count and compiler-cache
  alternatives with separate target directories before making them defaults.
- **Investigate later** upstream font-stack convergence, Linux winit/syscall
  convergence, and Apple binding updates. Making image codecs or platform
  support optional would require an explicit product decision and separate
  compatibility testing. Optional lock-only packages offer no ordinary build
  time benefit when removed from the lockfile alone.
- **Review after major features** with the inventory generator and a locked,
  target-specific tree. Re-run clean timing samples before adopting profile,
  linker, or feature changes. This audit changes neither optimization levels
  nor LTO.

## Functional verification

- Formatting, denied-warning Clippy, and `cargo test --locked --all-targets`
  passed locally. The suite had 345 passes and 14 ignored benchmarks. It covers
  pressure ink and highlighter coverage, shapes, connectors, rich text and
  installed font discovery, formula rendering and LaTeX configuration,
  image and attachment flows, selection, transforms, groups and history.
  The native opener test exercises the dispatch boundary with a controlled
  opener and checks missing-handler errors.
- Linux release, WASM release, WASM type check and Trunk release passed. The
  Linux release opened a new X11 window with disposable preferences.
- Firefox ran the packaged WASM release and displayed a new Whiteboard with
  monoline ink, a highlighter stroke, a committed `x^2+1` formula and a PNG
  dropped through the browser canvas file event. Image selection handles
  appeared on the canvas.
- Hosted [CI on the cleanup commit](https://github.com/fredericwaldmann/XiToc/actions/runs/44550326880)
  passed the Linux, Windows, macOS and web jobs. Dialog and default-application
  behavior on a real Windows or macOS desktop remains outside this smoke test.
- A repeat generation produced an identical CSV, with 377 data rows including
  the workspace root and exact agreement with Cargo.lock.

## Native Ubuntu window-decoration follow-up

The follow-up adds Linux-targeted direct winit 0.30.13 with defaults disabled and
only `wayland-csd-adwaita` enabled. This restores GNOME-style client-side
decoration support omitted when eframe defaults are disabled; it does not add
winit to the WASM target or enable unrelated features. Inventory regeneration
also corrected its direct-dependency catalog, which had omitted the existing
`two-face` listing dependency. The counts above describe the original audit
baseline.
