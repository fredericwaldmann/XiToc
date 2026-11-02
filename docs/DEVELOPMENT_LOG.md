# XiToc development log

This log reconstructs repository history through main `8fc0f94` on 2026-11-02
from first-parent Git history, merged GitHub PR metadata and milestone records in
[TEST_PLAN.md](TEST_PLAN.md), [M0_REVIEW.md](M0_REVIEW.md) and
[screenshots/README.md](screenshots/README.md). Dates below use GitHub merge dates
in UTC where available; author/commit dates can differ. Test results are attributed
historical records, not new executions by this documentation audit. A configured
workflow does not prove that its remote run or deployment succeeded.

For current behavior, use [CODEBASE_GUIDE.md](CODEBASE_GUIDE.md). Architectural
constraints and their evidence are recorded in
[ARCHITECTURE_DECISIONS.md](ARCHITECTURE_DECISIONS.md).

## M0 — Shared shell and web packaging (2026-10-27)

**Evidence:** scaffold commit `ee8f3c6`; `0a6fb2a` (`index.yaml`), `0959126`
(merge from main) and `7d483e2` (CI milestone tests) also appear in early history.
The latter commit subjects alone do not provide a complete feature rationale.

**Implemented:** Rust/egui/eframe 0.37 shell, native/WASM entry points, resizable
placeholder Explorer, fitted page/grid preview, disabled drawing controls.
Trunk configuration and the Pages workflow followed. At this stage document kind
was still preview UI state and there were only two Rust source files.

**Recorded checks:** M0_REVIEW reports Linux fmt, Clippy, zero Rust tests, native
build, WASM check and Trunk builds. The Pages follow-up records local HTTP 200
checks for HTML/JS/WASM and `/XiToc/` paths, not actual browser rendering or a live
GitHub deployment. **Limitations then:** no files, semantic model, ink or pan/zoom.
The original visual mode switch is no longer the current document model.

## Desktop release pipeline (2026-10-27)

**Evidence:** [PR #1](https://github.com/fredericwaldmann/XiToc/pull/1),
implementation `f10a1f9`, merge `4f1710c`.

**Implemented:** tag/manifest/Cargo metadata validation, native build matrix,
archive packaging and CPU/content verification, draft creation followed by asset
verification before publication. Release artifacts are unsigned standalone
executables for Windows x64, Linux x64 and macOS arm64/x64.

**Recorded verification:** TEST_PLAN describes archive-failure fixtures, expected
native checks and manual release failure/launch scenarios. The current Python
suite concretely tests packaging headers/contents and error paths; fixtures are
not executable applications. **Limitations:** signing/installers, matching-machine
launch checks and remote publication failure scenarios need separate evidence.

## M0.5 — Document types and blank native PDFs (2026-10-27)

**Evidence:** [PR #2](https://github.com/fredericwaldmann/XiToc/pull/2),
implementation `39f571b`, browser export wording fix `81474c6`, merge `dd93153`.

**Implemented:** immutable creation-time Page/Whiteboard layouts, UUID identity
separate from location, selected native workspace/real-folder tree, virtual
categories, named blank-PDF creation/load and browser import/export session
fallback. `pdf-writer` creates standard blank PDF graphics and the named schema-1
attachment; `lopdf` validates reads. Native capability paths and no-clobber atomic
publication protect creation. External PDFs remain read-only; malformed/future
native metadata errors instead of silent conversion.

**Recorded verification:** TEST_PLAN records native restart/relocation, PDF
writer/parser interoperability, collision/write-failure/path/symlink cases and
manual independent-viewer procedures. Its performed-observation sections must be
read separately from the reproduction checklist. **Limitations:** one blank A4
page, no ink serialization, existing-file updates or browser directory API.
Export wording was explicitly corrected to avoid claiming a completed browser save.

## M0.6 — UI design foundation (2026-10-27)

**Evidence:** [PR #3](https://github.com/fredericwaldmann/XiToc/pull/3),
implementation `ab2cf33`, merge `b9d3c7a`.

**Implemented:** centralized light/dark tokens and vector icons, Explorer,
horizontal typed tabs, compact controls/status, page edge/shadow and Whiteboard
grid, responsive menus and original shared presentation components.

**Recorded verification:** TEST_PLAN and screenshot inventory describe layout,
tabs, both layouts/themes, narrow-window and scale checks alongside build gates.
**Limitations:** drawing/persistent application preferences were later work at
this milestone; historical disabled controls are not the current toolbar state.

## M1 — Shared in-memory handwriting (2026-10-27)

**Evidence:** [PR #4](https://github.com/fredericwaldmann/XiToc/pull/4),
implementation `53939a1`, controls/verification docs `6472a10`, merge `7608710`.

**Implemented:** shared Canvas input/rendering for both layouts, editable raw
strokes with UUIDs/ownership, midpoint smoothing, whole-stroke eraser, per-editor
undo/redo, navigation, cached meshes and dirty/discard safeguards. Original PDF
bytes and blank schema remain separate from memory-only ink. Browser beforeunload
and dirty-export block prevent misleading save claims.

**Recorded verification:** TEST_PLAN records model/controller/history/storage
regressions and browser drawing/zoom/erase/undo observations; screenshots include
Page ink, Whiteboard ink and discard confirmation. **Limitations:** handwriting
still cannot survive restart through PDF; no pressure/partial erasing/selection yet
at this historical point.

## M1.1 — Eraser performance (2026-10-28)

**Evidence:** issue #14, [PR #16](https://github.com/fredericwaldmann/XiToc/pull/16),
implementation `2f64104`, merge `9b8c488`.

**Implemented:** revision-keyed bounds hierarchy, candidate filtering and cached
narrow-phase paths; batch compaction/restoration rather than repeated vector shifts.
One sweep remains one undo transaction with exact order/identity restoration.

**Recorded verification:** TEST_PLAN compares original/indexed hit behavior at
multiple zooms/layouts and reports synthetic release benchmarks. **Limitations:**
index setup still costs work; benchmarks exclude device/display latency and do not
remove genuine dense-overlap costs.

## M1.1 — Persistent application Settings (2026-10-28)

**Evidence:** issue #6, [PR #17](https://github.com/fredericwaldmann/XiToc/pull/17),
implementation `8978451`, merge `eecab83`.

**Implemented:** General/Appearance/Drawing/Input Devices modal, shared controls,
application preferences in eframe storage, explicit validation/migration and
unreadable-preference guard. Note contents/workspace paths are excluded.

**Recorded verification:** TEST_PLAN records 42 passing Rust tests, one ignored
benchmark and fmt/Clippy/native/WASM/Trunk checks. **Limitations:** native eframe
Storage cannot acknowledge durable disk writes; preferences do not save notes.

## M1.1 — Configurable spatial smoothing (2026-10-28)

**Evidence:** issue #12, [PR #18](https://github.com/fredericwaldmann/XiToc/pull/18),
implementation `326269e`, merge `afc0321`.

**Implemented:** versioned `AdaptiveV1` metadata, 0–100% strength, local convex
filter/quadratic joins with corner protection, bounded incremental trailing work,
128-segment cached mesh chunks and preference schema 2 migration. Legacy/raw
samples retain their geometry and style.

**Recorded verification:** TEST_PLAN reports 50 passing tests, two ignored
benchmarks, all build gates and a separately run release smoothing benchmark.
**Limitations:** no temporal/device latency guarantee or PDF handwriting save.

## M1.1 — Partial erasing (2026-11-01)

**Evidence:** issue #13, [PR #19](https://github.com/fredericwaldmann/XiToc/pull/19),
implementation `5f2bffb`, merge `ab622ae`.

**Implemented:** Stroke/Partial eraser mode preference (schema 3), analytic swept
capsule clipping, merged scalar cuts, path-block index, incremental preview and
one-command replacement by editable `PolylineV1` fragments with new UUIDs.
Undo restores originals; redo reuses exact fragments/order.

**Recorded verification:** TEST_PLAN reports 61 passing tests, four ignored
benchmarks, all build gates, separately run eraser benchmarks and Firefox
whole/partial/undo observations. Its reload check was interrupted; preference
persistence was covered by automated round trips. **Limitations:** no PDF ink
serialization; fragment rendering/setup remains proportional to affected geometry.

## M1.1 — Pressure-sensitive pen input (2026-11-01)

**Evidence:** issue #15, [PR #20](https://github.com/fredericwaldmann/XiToc/pull/20),
implementation `1c77ec5`, merge `217fc09`.

**Implemented:** optional normalized raw force, captured `PressureStyle`, matching
smoothing interpolation and variable-width vector/hit/clip geometry. Browser
primary-pen bridge and native egui force handling; schema 4 preferences.

**Recorded verification:** TEST_PLAN reports 68 passing tests, four ignored
benchmarks, build gates and synthetic Firefox DOM pen input. PRESSURE_INPUT records
backend-source inspection. **Limitations:** physical pen hardware was not tested;
Windows force path is source-verified, Linux/macOS native use constant-width
fallback. Synthetic DOM tests do not prove real pointer capture or hardware force.

## M1.3 — Floating drawing workspace, subsequently superseded (2026-11-01)

**Evidence:** [PR #21](https://github.com/fredericwaldmann/XiToc/pull/21),
implementation `ff43d46`, merge `c488130`.

**Implemented then:** floating contextual drawing palette and pen/eraser popovers.
The next milestone revision replaced its layout; no bottom floating drawing
palette is current behavior.

**Recorded verification:** archived TEST_PLAN section reports 68 passing tests,
four ignored benchmarks, native/WASM/Trunk gates, Firefox layout/drawing smoke and
Linux startup. **Limitations:** historical screenshots describe the old layout;
they are not acceptance evidence for the present top-toolbar placement.

## M1.3 — Top toolbar and pen presets (2026-11-01 UTC merge)

**Evidence:** [PR #22](https://github.com/fredericwaldmann/XiToc/pull/22),
implementation `820a564` (author date 2026-11-02), merge `6276643`.

**Implemented:** fixed toolbar below tabs, five default presets, custom preset
lifecycle, Monoline/Pressure/Highlighter style capture, translucent ribbon meshes
and shared Settings/preset editor. Preferences schema 5 replaces independent pen
defaults and migrates legacy settings.

**Recorded verification:** TEST_PLAN reports 75 passing tests, four ignored
benchmarks, all build gates, release partial-erase benchmarks and actual Firefox
wide/narrow light/dark inspection with preset restart persistence. Linux startup
was observed. **Limitations:** selection was still disabled at that revision;
physical pressure and Windows/macOS interaction were unverified.

## M1.4 — Universal stroke selection and manipulation (2026-11-02)

**Evidence:** issue #23, [PR #28](https://github.com/fredericwaldmann/XiToc/pull/28),
implementation `99d37b1`, merge `aac53e6`.

**Implemented:** `CanvasObject` identity/bounds/placement protocol, page-local or
Whiteboard selection, topmost picking, full-envelope lasso, move/proportional
resize previews and transactional commands, bounded clipboard with session
fallback, duplicate/delete and focus-aware menus/shortcuts. Raw points and style
remain immutable under transforms.

**Recorded verification:** TEST_PLAN reports 95 passing tests, five ignored
benchmarks, all build gates and all ignored release benchmarks; actual Firefox
selection/resize/cross-document paste/context/light-dark/narrow checks and Linux
startup. **Limitations:** page-local selection, 4096 lasso vertices, clipboard
caps, conservative highlighter envelopes; no rotation control or PDF ink save.

## Unified canvas object foundation follow-up (2026-11-02)

**Evidence:** [PR #29](https://github.com/fredericwaldmann/XiToc/pull/29),
implementation `d821eab`, merge `8fc0f94`.

**Implemented:** `ObjectType`, shared tagged `ObjectSnapshot`, object traversal/ID
lookup, domain pressure hit function, normalized local-bound anchors, rotation in
`Placement`, four-corner transformed bounds and undoable z-order. The stroke vector
remains the canonical store/order; no new object variants or GUI controls.

**Recorded verification:** TEST_PLAN reports 98 passing tests, five ignored
benchmarks, fmt/Clippy/Linux native/WASM/Trunk checks and all five release
benchmarks. Controller regression extends both erasers to rotated/scaled ink with
exact undo/redo on both layouts. **Limitations:** no interactive/physical smoke
was performed for this follow-up; object lookup is linear, the store/journal are
still stroke-specific, and PageId migration/M2 persistence remain future work.

## Technical documentation audit (this documentation change)

Created the source-based codebase guide, decision records and this history;
added maintenance rules to AGENTS and corrected conflicting current-state claims
in existing documents. No application code, dependency, workflow or functionality
changed. Documentation checks and their actual results are reported in the PR;
older milestone test counts above remain explicitly historical.

## Selection/Lasso acceptance follow-up (2026-11-02)

**Evidence:** related [issue #23](https://github.com/fredericwaldmann/XiToc/issues/23),
branch `feat/selection-lasso-followup`, based on main `2c24481`. The requested title
“Selection and Lasso Selection” was absent from the complete issue list; #23 is
“[M1.4] Lasso Selection and Object Manipulation,” with no comments at inspection.

**Changed:** retained existing PR #28 Selection/Lasso controls and geometry engine;
canvas Escape now cancels a live gesture and clears selection in one step. Four
new controller tests verify Escape/late release/history/mesh reuse, zoomed and
panned geometry picking, Page versus Whiteboard boundaries, and tool handoff.
No new movement/resize/rotation controls, object variants or PDF persistence.

**Performed verification:** fmt, Clippy with warnings denied, 102 passing tests
(five ignored benchmarks), Linux native build, WASM check, Trunk release build and
all five ignored release benchmarks. Real Firefox release-app smoke exercised
click/Shift-click/lasso, live-lasso Escape and unchanged ink in both layouts,
Ctrl+A/Escape, dark appearance and 480×360 layout. **Limitations:** no interactive
native/physical stylus or Windows/macOS runtime test in this follow-up; future
object types still need adapters in the same canonical store/selection dispatch.

## M1.4 — Object manipulation and clipboard (2026-11-03)

**Evidence:** branch `feat/object-manipulation`, starting from main `50cf36e`,
related [issue #23](https://github.com/fredericwaldmann/XiToc/issues/23). The complete
issue list contained no “Move, Resize, Rotate, Duplicate and Clipboard” title;
#23's complete description/criteria and empty comments were reviewed.

**Implemented:** live group rotation with a screen-sized handle, full-turn angle
unwrapping and dynamic 15-degree Shift snapping; keyboard-accessible rotation
steps; validated-copy-before-delete Cut and logical arrow nudges. Existing move,
proportional resize, duplicate/copy/paste/delete and journal are reused. Handwriting
keeps a uniform nib/pressure envelope, including off-axis resize drags, with a
1-point longest-extent floor. Rotation previews reuse tessellation and cache idle
meshes. Conservative page bounds and explicit ownership remain. A regression
caught contextual controls overlapping the rotation hit target; clearance was fixed.

**Verification:** fmt/Clippy, 111 passing tests (six ignored benchmarks), native
release compilation, WASM check and Trunk release build. All six release benchmarks
passed; the 2,000-stroke/40,000-sample rotation fixture measured 44.01 ms first
preview, 12.15 ms subsequent changed frame, 5.45 ms unchanged frame under concurrent
build load. These are host-dependent CPU measurements, not display latency promises.
A timed Linux release startup stayed alive without emitted errors until deliberate
termination. Interaction observations and remaining platform checks are in TEST_PLAN.

**Limits:** no anisotropic handwriting stretch, new semantic object variants,
connector relations, Arrange controls or PDF ink persistence. Future box-like
objects need their own nonuniform resize policy. System clipboard durability has
no egui acknowledgement; the app session copy protects Cut/Paste within XiToc.

**Browser findings/verification:** actual Firefox release-app manipulation exposed
native page selection/bookmark defaults interfering with canvas A/D gestures.
Canvas-only key-default and native dragstart guards prevent browser DND from
swallowing move/release. Verified rotation commit/undo/redo, duplicate, Cut/Paste,
cross-document Page paste/Shift rotation/nudges, dark/narrow layout and fresh-load
DOM guard assertions. Physical pen and interactive native manipulation remain
unverified; see TEST_PLAN for exact observed scope.
