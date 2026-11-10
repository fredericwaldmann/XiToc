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

## M1.4 — Unified Selection tool (#34, 2026-11-04)

Started from main `ac33eaa`; reviewed issue #34's complete body and empty comments.
One top Selection button/dropdown now recalls Rectangle (default) or Lasso mode.
Both modes share topmost clicks, idempotent Shift-add, Alt-remove and selected-group
movement/handles. Rectangle endpoint direction chooses full envelope containment
(solid blue) or crossing (dashed green), with indexed live candidate feedback.
Lasso retains strict geometric containment and stays active after release.

Verification: fmt, Clippy with denied warnings, 117 passing tests (seven ignored
manual benchmarks), native release build/startup, WASM check and Trunk release.
Controller coverage includes all diagonal directions, both document kinds,
zoom/pan, modifiers, direction changes, cancellation, rotated geometry and history.
Toolbar event tests verify dropdown choices and mode recall. Existing manipulation,
clipboard, eraser and focus regressions pass. Release benchmark: 2,000 strokes,
100 warmed rectangle samples in 27.56 ms on this host, excluding renderer costs.
Runtime/browser observations and remaining platform checks are in TEST_PLAN.

Firefox release-app smoke confirmed the one-button dropdown, drawing, live blue
containment/green crossing feedback and retained lasso mode with handles. Captures
are under `docs/screenshots/unified-selection/`; TEST_PLAN records actual scope.


## M1.4 — Horizontal/vertical object flipping (#33, 2026-11-04)

Started from latest main `3963d64` (including unified Selection). Read issue #33's
complete description/acceptance criteria and empty comments. Added positive-scale
local-axis reflection flags to shared Placement, with handedness-aware composition
and inversion. Flip commands use the same validated batch transform/history path,
reflecting around collective selection bounds in document coordinates. Raw points,
pressure, smoothing, styles, IDs and order remain unchanged. Existing rotation
steps and both flips share Edit/context → Transform. Right-click works from all
drawing tools, preserving selected groups or selecting an unselected target.
Empty context clicks retain membership without editing content.

Verification: formatting, Clippy with denied warnings, 124 passing tests (seven
ignored manual benchmarks), native release build/startup, WASM check and Trunk
release build. Tests cover both layouts/axes, groups/singletons, rotated/resized
pressure ink, double flips, exact undo/redo, cache preservation, zoom independence,
page-failure atomicity, far coordinates, clipboard/partial-erasure regression,
all reflection composition parities, legacy snapshots and actual submenu routing.
Firefox release-app pointer smoke verified the nested Transform menu, selected
pair horizontal/vertical reflection, Undo/Redo and the existing eframe canvas
browser-context-menu prevention. Captures are in `screenshots/object-flipping/`.
No interactive Windows/macOS/physical-stylus or device-latency claims are made.
Ink PDF persistence remains M2; future text/groups/connectors are policy only.


## M1.5 — Basic shapes, lines and arrows (creation slice of #24, 2026-11-04)

Started from main `8da20d6`, including unified Selection and reflection transforms.
There is no issue titled “Basic Shape Creation”; reviewed the corresponding #24
“[M1.5] Shapes and Styling” complete body/criteria and empty comments. That issue
also requires full styling and shared embedded text, excluded by this task, so
this creation slice references #24 without closing it.

Implemented all eight requested forms as typed Shape snapshots in the same ordered
store and Replace/Erase journal as Stroke. Arc-only ink/live-ID views preserve
existing renderer/eraser adapters without parallel canonical content. Creation,
selection, mixed clipboard/order/history, local box/endpoint edits and shared
move/proportional-group resize/rotate/reflection now dispatch both content types.
The top picker uses original vector glyphs; Hand moves to View below 400 editor
points. Shape styling data/defaults are captured, with no formatting/text UI.
Freestanding arrows have no connector relationships. PDF persistence is unchanged.

Verification: formatting, Clippy with warnings denied, 132 active tests pass,
seven ignored release benchmarks all explicitly pass, native release build/startup,
WASM check and Trunk release. New tests cover every shape/quadrant/layout/zoom,
constraints/cancel, exact mixed history/order/clipboard, region geometry, local
handles after rotation/reflection, and idle mesh reuse. Review also caught and
fixed cached-preview cleanup after cancellation/degenerate release. Benchmarks
and actual Firefox pointer observations are recorded in TEST_PLAN.

Firefox packaged-WASM smoke verified all eight forms on a Whiteboard, endpoint
and rectangle corner editing, Select All/Duplicate/Undo, dark appearance,
480×360 top picker with Explorer visible and Page Note rectangle/arrow creation.
Captures: `docs/screenshots/basic-shapes/`. Interactive native gestures,
Windows/macOS hardware/stylus and universal device-latency claims remain unverified.


## 2026-11-05 — Shapes drag quality before merge

Continued `feature/basic-shapes` without discarding original feature commits.
Closed #37 unmerged as requested; the replacement PR will reference it. #24
remains open because its broader styling/text acceptance criteria are not part
of this creation milestone.

Reproduced hidden axis-aligned singleton resize hit targets, off-center handle
snapping and per-update shape tessellation as regression failures. Singleton
shapes now use only their visible local handles; handle targets preserve the
original grab offset. Shapes and handwriting share the cached graphics-layer
movement/scale preview. Existing stable document-space pointer-start displacement
and immutable source snapshots remain the movement basis; release commits once.
No document/storage format or toolbar changes.

Verification: 138 active tests pass (eight ignored benchmarks), formatting,
Clippy, native release build/startup, WASM check and Trunk release pass. Both
release preview benchmarks pass, including 1,000 ellipses retaining mesh Arcs over
40 movement frames. Real Firefox WASM gestures covered all eight forms, reversal,
cancellation and undo/redo; repeated-position captures matched exactly. Native
interactive visual performance was not tested. See TEST_PLAN for matrix,
measurements, manual checks and limits.


## 2026-11-05 — Persistent group dragging and viewport boundaries

Continued the corrected Shapes branch and existing PR #38. Reproduced two red
regressions before changing production code: inverse preview clipping intersected
with screen-space clipping (progressive disappearance during large group moves),
and PointerGone cancelled a held gesture on canvas hover loss. Pointer displacement
was already derived correctly from the original document-space press.

The common ink/shape preview now replaces its layer-space clip. Transform sessions
capture IDs, immutable source snapshots, view/revision and original geometry;
rendering/commit use captured membership, never visibility or re-hit-testing.
Ignore hover loss, retain document-wide eframe mouse move/up handling, and forward
only its missing explicit browser mouse cancellation event. Contact/focus/view
cancellation and absent button state safely discard previews; release samples its
final position and commits once. No smoothing, animation, auto-pan or PDF changes.

143 active tests, formatting, Clippy, native release, WASM check and Trunk release
pass; both explicit release preview benchmarks pass. Real Firefox tests verify
fast five-object drags, leaving/returning to canvas, exact repeated positions,
oversized selection at 400%, cancellation and history. Native release startup
works, but Wayland desktop capture/input verification was unavailable. TEST_PLAN
records failures, matrix, timings, visual evidence and manual checks.

## 2026-11-06 — Shape styling and formatting (M1.5 slice of #24)

Started feature/shape-styling from latest origin/main (75e7689). The requested
issue title does not exist; read the complete matching #24 “[M1.5] Shapes and
Styling” and its empty comments list. Its embedded/shared-text criteria remain
outside this task, so the PR references #24 without closing it.

Added independent optional fill/color/alpha, border enablement/color/width/alpha/
solid-dashed-dotted pattern, segment start/end/both/no heads and clamped rounded
radius. Captured semantic styles stay on the shared Shape objects. Compatible
multi-selection patches use one validated Replace journal transaction, retaining
identity, geometry/Placement, order, ink and unrelated objects. Arrowhead contours
follow directed endpoints through existing transforms and handle edits.

The existing selection toolbar now offers contextual formatting menus and the
canvas right-click menu exposes the same Format hierarchy. Mixed values are
explicit; shared slider rails use existing tokens. Shape creation defaults live
separately in application settings and are captured on press. No text, connectors,
PDF persistence, sidecars or Explorer/tab/main-toolbar redesign.

Real browser testing exposed a final-release sample creating a second undo step.
A focused mixed-selection controller test reproduced it before the fix. Formatting
gesture cleanup now runs in end_frame after all surface/popup controls, allowing
the last sample to merge. A second UI regression keeps mixed-to-common slider IDs
and positions stable. Existing movement sessions and preview clipping are unchanged.

See TEST_PLAN for final checks, actual browser observations, manual procedures and
platform limits. CODEBASE_GUIDE/UI_DESIGN/ARCHITECTURE and the decision record
specify model ownership, command coalescing, defaults and future connector work.


## 2026-11-06 — Correct joined borders, perimeter patterns and color controls

Continued feature/shape-styling and PR #39 without merging. Source investigation
found solid contours repeated their start vertex but were tessellated as open
lines, leaving cap joins at the seam. egui's dashed helper carried phase across
edges, but emitted independent line segments at vertices, losing connected joins;
neither its dash nor dot helper balanced the closing seam. Width-derived screen
patterns also prevented independent formatting.

Added UI-independent cumulative path measurement with joined cross-corner dash
slices, whole-period closed fitting/gap-centered seam, even closed dot placement
and global overlap/density bounds. One cached border adapter uses explicit closed
stroke tessellation and limited miter/bevel joins across every shape/shaft/head.
ShapeStyle captures independent logical-point pattern values with defaults and
validation; shared Style controls expose conditional dash/gap/spacing/diameter.

Actual Firefox reproduction confirmed that clicking the old swatch closed Fill
without leaving a usable picker. The nested color button opened another root menu
popup. Replaced it with an inline RGB/HSV picker, alpha-aware preview and hex RGB
entry; RGB edits retain independent opacity. Automated real-widget tests exercise
inline expansion, hex color commits, conditional pattern controls and unchanged
alpha/border state. Model/history/clipboard tests preserve custom pattern values;
mesh/path tests check closure, corners, uniform periods/dots and zoom independence.

Existing drag sessions, layer preview clipping, IDs/order, pen/erasers and one-PDF
save guards remain intact. No connectors, text or PDF persistence were introduced.
See TEST_PLAN for completed checks, screenshots and platform limits.


## 2026-11-07 — M1.6 straight magnetic connectors (#25 slice)

Created feature/magnetic-connectors from latest main 23e21b2. Read complete #25 and
its empty comment list plus #24/#23 dependencies and repository docs. The requested
long title corresponds to “[M1.6] Magnetic Connectors”; its orthogonal-route gate
remains outside the user-requested straight-only scope, so the PR will not close it.

Added Connector/Endpoint/Port semantic types, shared store/protocol/selection/style
adapters, ID/reverse relationship indexes and atomic linked graph updates. Drag
previews resolve attached ends against current shape geometry/placement without
canonical writes or whole-document geometry clones. Port and endpoint gestures
support snap feedback, free ends, reconnect/detach and persistent hover-loss/viewport
ownership. Ctrl/Cmd+Shift+K/context creation connects selected shapes. Existing
ink/shape movement mesh layers and clipping remain unchanged.

Node deletion detaches surviving endpoints; undo restores node/links/order.
Clipboard remaps copied internal IDs and frees external references. All edits use
the same journal; primary selection metadata avoids selecting incident wires on
undo. Tests also preserve styles on temporarily coincident/zero-length routes.
Page-margin ports remain clickable while connector content stays on its page.

See TEST_PLAN for performed checks and actual visual observations. No orthogonal/
curved routing, text, magnetic links to ink/arrows, PDF persistence or unrelated
Explorer/tab/top-toolbar changes were added.

## 2026-11-07 — Advanced magnetic connector routing (#25)

Started feature/advanced-connector-routing from latest main c0a20e0, retaining
merged straight connectors. No issue titled Advanced Connector Routing exists;
read complete #25 “[M1.6] Magnetic Connectors” and empty comments. It covers
orthogonal routing and explicitly excludes labels; no duplicate issue was created.

Added orthogonal and cubic modes, endpoint-specific automatic/fixed preference,
anchor hysteresis, semantic manual rail/control offsets and Reset Route. Reused
shared object identity/selection/journal, reverse dependencies, persistent gestures,
joined border/pattern meshes and contextual formatting. Derived paths use one
measured shaft; arrowheads use path tangents. Cached routes avoid idle recomputes.

Tests cover local obstacle clearance, alignment, transformed attachments, manual
edits, clipboard/history, curve geometry/picking, patterns, live hysteresis and
real route-handle controller input in both layouts at 50/100/200/400% zoom.
See TEST_PLAN for actual completed build/visual checks and known limitations.
No global obstacle avoidance, text labels, PDF saving or unrelated UI changes.

Visual 400% group-drag inspection caught a filled-region stroke artifact; bounded
near-reversal bevels and a corridor mesh regression were added, then the rebuilt
package was rechecked. Toolbar wrapping reserves extra clearance for Routing.
Analytical direction transforms preserve normals independently of far-origin
translation. Final checks and screenshots are recorded in TEST_PLAN.

## 2026-11-08 — Standalone editable text boxes (#26 slice)

Confirmed advanced routing PR #41 merged, then created feature/editable-text-boxes
from main 58a80ad. Read complete #26 and #24 with empty comments. #26's embedded
shape-text dependency is not implemented, and user scope excludes that host; this
PR references #26 without claiming all broader acceptance criteria.

Added TextObject to the shared canonical store/protocol/history/clipboard; portable
TextContent/TextStyle, bundled fonts, reusable layout/editor APIs and a top T tool.
Implemented immediate caret creation, double-click editing, natural width reflow,
formatting, shared selection/transforms, readable flips, typed/source clipboard
routing and bounded history. Raw text/style Arcs remain shared during transforms.

Tests cover source/Unicode/IME, normalized zoom/DPI metrics, font fallback, styles,
wrap/reflow, selection, copy/duplicate/history, focus, menu routing, cache reuse,
page overflow, mixed offscreen dragging and source preservation. Browser review
caught initial focus handoff and independent screen-font wrapping defects; both
received fixes and focused regressions. See TEST_PLAN for performed checks and
actual visual evidence. Native OS IME/hardware coverage remains unverified.

No shape text, labels, sticky notes, rich spans, PDF persistence, sidecars or
unrelated Explorer/tab redesign. Fonts are compiled resources with licenses.

## 2026-11-10 — Rich text upgrade on open text-box PR #42

Confirmed feature/editable-text-boxes and PR #42 are still open and continued that
branch. Read actual #26 and its shape-host/platform acceptance criteria; reference
it without closing it. The existing egui engine supports a semantic TextBuffer and
mixed LayoutJob sections, so no replacement editor or unfocused-only rich rendering
was needed. Added grapheme-safe character runs, paragraph metadata, a transient
caret typing style, mixed controls and rich session paste. Full semantic edits use
the shared journal; the plain-string framework undoer is cleared.

Fontdb catalogs native families and lazily resolves/caches faces through a shared
font provider for display/normalized metrics. WASM retains bundled fonts; the
experimental permission-gated Local Font Access API is documented, not implemented.
A reversible font session retains source/range, previews without writes, and commits
one operation. A virtual font list renders only visible family-name previews.
Eight local resize handles reuse captured source/pointer/view/revision ownership.
Reflow preserves glyph sizes; corner Scale preserves aspect and mixed proportions.
Per-box and persisted new-box defaults remain independent. Text/Block/Box controls
separate character, paragraph and container patches; borders reuse shape rendering.

Actual browser testing reproduced an existing egui lock failure when clicking
outside active text: a nested layer query ran inside an input read. Added a real
canvas/widget regression, moved the query outside that lock, rebuilt and verified
selected-word formatting. Also tested focus-preserving controls, immediate typing
style handoff, preview cancel/commit, Unicode/rich clipboard, padding-cache reflow,
future resize defaults and rotated eight-handle sessions across four zoom levels.
See TEST_PLAN for final executed checks and GUI evidence/limits. No shape-contained
text, labels, PDF serialization or unrelated tools were introduced.

Final review also covered rich replacement into an empty box: the first copied
paragraph now retains its alignment, with the multi-paragraph paste regression
extended to include this case. Partial insertion keeps destination block styling.

The same final regression pass checks that Block controls display the caret's
paragraph (including non-first-paragraph spacing), and replacement typing inherits
the first selected character's style.


## 2026-11-10 — Text/Paragraph correction and persistent text selection (PR #42)

Continued the existing open feature/editable-text-boxes branch/PR. The user clarified
that Box was a misunderstanding of Justify. Removed the Box UI, decorative model,
shape proxy/renderer, padding and opacity handling. TextLayout now owns resize only;
legacy clipboard/session objects keep source, styles, ID, geometry and resize through
a compatibility reader that discards known decorations. Both layouts still have
invisible bounds, selection outlines, eight grips and Reflow/Scale defaults/overrides.

Renamed Block to Paragraph, added Justify and isolated paragraph patches from all
character styles. Canonical word-gap expansion justifies eligible wrapped rows
without letter stretching; final/single-word rows and explicit paragraph breaks
remain natural. Scaled glyphs, underlines, caret positions and selection use the
same geometry and update after width changes.

Reproduced egui's collapse of its unfocused selection cache in full Canvas tests.
The session retains the single exact directional range; the adapter synchronizes
and rehydrates widget state, disables editor input during controls and paints an
inactive selection. Font preview keys cannot move the caret; preview range comes
from that same suspended session. Font/size/bold/color and paragraph/spacing controls
can be chained without reselecting. See TEST_PLAN for checks and GUI evidence.

Verification: 240 active tests, formatting, Clippy with warnings denied, native
release/startup, WASM check and isolated Trunk release pass. Packaged Firefox
verified the no-reselection formatting chain on new and double-clicked existing
text, all four alignments, natural final lines, justified left-edge reflow and
undo/redo. New captures are in screenshots/text-formatting-correction. Native
physical IME/clipboard and full RTL remain explicit verification/engine limits.

## Mixed font-size scaling correction — 2026-11-10

The selected-object −/+ controls now scale every run's stored font size by
reciprocal 0.9/1.1 factors for text-only selections; other selections retain
geometric scaling. Character properties, paragraph styles, object identity and
placement are preserved. Mixed-size Text controls display `Mixed` without a
representative-value slider, and panel initialization adds no history operation.
Regression tests cover proportional/reciprocal scaling, mixed-state UI rendering,
panel read-only behavior, and one-step undo/redo.

## M1.9 — Editable rich text inside shapes

Shapes now optionally own `ShapeText` with the existing rich `TextContent` and
vertical alignment. Double-click starts the same TextSession using a transient
adapter with the shape's ID; semantic edits patch the one Shape snapshot. Closed
shape geometry supplies a conservative wrapping region, cached layout follows
shape transforms, and overflow is visibly indicated without discarding text.
Shape styling and connectors retain their existing model and ID relationships.
Coverage includes shape text-region geometry, formatting scope, stable identity,
clipboard/history, transforms, and magnetic connector behavior. Text remains
memory-only pending the PDF persistence milestone.
