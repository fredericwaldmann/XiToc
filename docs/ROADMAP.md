# Development roadmap (one Codex task / PR per slice)

This file preserves original milestone scopes; later implemented slices supersede
older exclusions. For current behavior see [CODEBASE_GUIDE.md](CODEBASE_GUIDE.md)
and for merged delivery evidence see [DEVELOPMENT_LOG.md](DEVELOPMENT_LOG.md).
M2 ink persistence is still unimplemented; M1.4 selection and PR #29 object APIs
are implemented. M3–M6 descriptions include both early groundwork and future work.

## M0 — Bootable project
- Native app on Linux/Windows/macOS and WASM via Trunk.
- Resizable Explorer placeholder, toolbar, tabs/header, gray work area, switch A4/Whiteboard preview.
- CI tests and static Web deploy. No fake save behavior.
- Acceptance: cargo fmt/clippy/test/check wasm + trunk build pass.
- M0 implementation: two source modules, current 0.37 `App::ui`/`Panel` APIs,
  fitted A4 page and full-area whiteboard grid, native and Trunk entry points,
  native CI matrix and Pages workflow. This is a UI preview only; file access,
  drawing and document persistence remain in later milestones. See
  `TEST_PLAN.md` for visual smoke checks; build success alone is not visual verification.

## M0.5 — Document types, creation workflow and Explorer
- Page Notes and Whiteboards are distinct document types chosen only at creation.
  No editor-level type toggle or conversion action.
- Shared versioned, UI-independent model: stable UUID, immutable typed layout,
  A4 page geometry or whiteboard viewport/grid settings; location is a separate
  workspace-relative locator and is never the document's identity.
- Native: select a real workspace, display its actual folder hierarchy beneath
  virtual Pages/Whiteboards sections, create a named blank PDF in the selected
  folder, open/close tabs and refresh after external moves. No physical category
  directories or duplicate files. Full rename/move/delete UI remains M3.
- Web: explicit PDF import and export fallback, with session-only grouping and
  honest download status; directory handles and real web workspace trees remain M5.
- Minimum PDF gate: valid blank PDF 1.7 with one conventional A4 page and the named
  `XiToc/document.json` attachment. Both kinds/UUID/settings survive reopen and
  restart/reimport. Whiteboards use an A4 fallback page in ordinary viewers.
- Page editor: A4 boundaries and page scrolling, with a list-based page layout.
  Whiteboard: borderless grid, drag/space-drag pan, Ctrl/Cmd-wheel/pinch zoom.
  Navigation is session-only; no content editing, camera save or conversion yet.
- Ordinary PDFs default to read-only Page classification. Bad/unsupported XiToc
  payloads fail explicitly. Visible external edits with recognized metadata are
  also read-only; never render them as a blank editable note or rewrite them.
- Acceptance: classification/identity/relocation/blank-PDF round trips, safe
  creation/collision/failure/symlink tests and shared editor initialization pass;
  independently parse/rasterize samples; native and WASM/Trunk builds pass.
- This blank-document subset does not satisfy the M2 handwriting round-trip gate.

## M0.6 — UI/UX design foundation
- Shared light/dark design tokens and original vector icons, documented in
  `UI_DESIGN.md`. Compact application area, Explorer, scrollable typed tabs,
  document toolbar, editor and bottom status bar.
- Resizable/hideable Explorer with virtual typed categories, folder selection,
  existing creation/context menus, hover/active feedback and truthful empty states.
- Tabs activate/close existing documents; no fabricated dirty state or type switch.
- Future drawing tools disabled with milestone hints; existing view/export controls
  work. Narrow layouts use menus rather than overlapping controls.
- Centered white A4 pages, neutral background, margins and subtle shadow;
  borderless Whiteboard with optional session-only dot grid.
- Acceptance: existing document/storage/PDF tests plus UI regression checks,
  fmt/Clippy/native/WASM/Trunk pass. Open the real app and capture Explorer,
  Page and Whiteboard screenshots; check resize, scaling and both themes.
- No drawing engine, PDF/schema changes, advanced filesystem features or saved
  UI preferences. M1 and later milestones retain their existing scope.

## M1 — Shared drawing engine and basic handwriting
- One mouse-input controller and vector renderer for Page Notes and Whiteboards.
  Smooth freehand ink, rounded caps/joins, opaque color and logical pen thickness;
  mouse samples now use pressure=None (unavailable), with constant-width fallback.
- Stable stroke/document UUIDs, explicit page index or canvas ownership, f64 PDF-
  point coordinates. A4 bounds are enforced; Whiteboards have no page bounds.
  Transforms subtract world centers before screen conversion. Completed stroke
  meshes are cached per transform/DPI; offscreen strokes are culled.
- Whole-stroke swept eraser with a visible screen-space radius; one undo command
  per eraser gesture. Reversible add/erase commands, multiple undo/redo operations
  and branch truncation. Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z affect only the active tab.
- Pen/Eraser/Hand, Ink settings, Undo/Redo and View zoom controls extend the M0.6
  toolbar. Space-drag pans; Whiteboard also supports middle-drag. Ctrl/Cmd-wheel
  or pinch zooms. A4 fits width at 100%; page scrolling is retained.
- Ink is memory-only in M1. Dirty tabs/status, explicit discard confirmations for
  tab close/workspace changes/native quit, and browser beforeunload warnings.
  Refresh/reopen never silently replace unsaved ink. PDF export is disabled for
  dirty documents; Ctrl/Cmd+S explains the M2 limitation, never claims a save.
- Acceptance: stroke/model/transform/ownership/bounds/eraser/history tests, actual
  pointer-event controller tests, discard/refresh/quit safeguards, existing tests,
  native/WASM builds and browser drawing/zoom/erase/undo smoke verification.
- No ink persistence, autosave/recovery, pressure hardware integration, selection,
  text, partial erasing, multipage creation or advanced M4 Whiteboard features.

## M1.1 — Eraser performance (#14)
- Reuse a revision-keyed spatial bounds hierarchy for swept eraser candidates;
  skip already-erased strokes and reject distant segments before exact tests.
- Preserve whole-stroke behavior and one command per gesture; erase/restore in
  linear passes while retaining original identities and stacking order.
- Regression gates compare against the original full scan for 1/100/10,000
  strokes on both layouts at 0.1×/1×/8× zoom. Include sparse-query pruning,
  large-coordinate tangency and native/WASM builds.
- Release benchmark compares the original scan and indexed pointer sampling.
  Manual desktop/browser responsiveness remains a device-dependent check.

## M1.1 — Application Settings (#6)
- Edit → Settings modal, General/Appearance/Drawing/Input Devices sections.
- Persist Explorer visibility, theme, drawing defaults and middle-button pan
  through eframe desktop/browser storage, separate from document contents.
- Share ink controls; validate versioned preferences; preserve unreadable/future
  preferences until explicit changes. No new smoothing/pressure/eraser-mode UI.
- Gates: menu/open/close and live preference tests, restart round trip, drawing
  default/document isolation and input behavior tests, native/WASM/Trunk builds.

## M1.1 — Configurable stroke smoothing (#12)
- Local corner-aware filtering and quadratic interpolation in document points;
  exact endpoints, true 0% raw input, 50% default and a persistent 0–100% slider.
- Capture a versioned algorithm/strength per stroke; preserve raw editable samples,
  pressure-ready data, identity/style and existing ink when preferences change.
- Incremental trailing geometry and reusable mesh chunks for responsive preview;
  identical release geometry. Preserve the indexed eraser and undo/redo behavior.
- Gates: strength/jitter/corners/dots/endpoints, coordinate independence,
  incremental/final equality and bounded work, cache reuse, raw JSON round trips,
  settings migration/persistence, actual controller/history, native/WASM/Trunk.
- No pressure input, new tools, partial erasing or PDF ink persistence.

## M1.1 — Stroke and partial erasers (#13)
- Compact toolbar eraser-mode dropdown and shared radius controls/cursor;
  persist Stroke/Partial selection with schema-1/2 preference migration.
- Analytic swept capsule clipping of the rendered centerline, including ink
  width; retain explicit editable polyline fragments without re-smoothing.
- Reuse document BVH and a temporary path-block index; merge cuts, reuse stable
  preview chunks and commit one validated replacement command per gesture.
- Gates: endpoints/middle/dots/thickness/multiple/smoothed strokes, exact history
  and stacking, branch truncation, pan/zoom/far origin/both layouts, vectors/JSON,
  menu/settings persistence, index work bounds and release benchmarks.
- No new drawing tools, pressure input, PDF serialization or sidecars.

## M2 — One-file PDF roundtrip (hard gate)
- Generate valid visible PDF with vector ink; embed JSON payload; import it back into same document model.
- Independent PDF parsing validation; zero sidecars, even across reopen and autosave.
- Do not move on until roundtrip tests pass.

## M3 — Desktop real Explorer
- Select user directory, recursive folders, `.pdf` listings, create/rename/delete/move (confirm destructive operations), open/save using real filesystem, atomic writes when possible.
- File paths restricted to workspace; integration tests in temporary directories.

## M4 — Whiteboard mode
- Infinite coordinate space, zoom and pan; content-bounded visible PDF page; export/reopen restores world positions and camera. Large coordinate / empty board tests.

## M5 — Browser storage
- Chromium permissioned directory picker where supported; same explorer semantics. Other browsers: explicit import/download fallback. Never simulate permanent disk write. Test in browser.

## M6 — More tools + polish
- Move/select strokes and associated shortcuts were delivered earlier in M1.4.
  Text/images, shapes, page templates, PDF import background, autosave recovery
  and signed installers remain future work. Extend existing smoke/accessibility
  coverage with each feature.

## M1.1 pressure-sensitive input (#15)

Optional normalized per-sample force, captured response settings, incremental
pressure-aware smoothing, round variable-width vectors and width-aware erasing
are implemented for both document types. Settings schema 4 persists enable,
strength and minimum width. Mouse and missing force use constant nominal width.
Browser primary pen PointerEvents and native egui touch-force input are consumed;
Windows has a source-verified winit pen path. Native Linux/macOS tablet integration
remains future work with constant-width fallback. See PRESSURE_INPUT.md.
Synthetic tests cover geometry, erasers, history, smoothing and zoom; physical
hardware has not been tested. M2 PDF serialization remains a separate milestone.

## M1.3 — Top drawing toolbar, pen types and presets

Supersedes the previous bottom-floating-palette design. Keep drawing controls in
a compact top panel immediately under document tabs; preserve Explorer, tabs,
menus and Settings. Selection was disabled in M1.3 and is enabled by M1.4 below.
Implement Monoline, Pressure Pen and semi-transparent Highlighter using shared
input/smoothing/erasers/history and cached vector rendering. No realistic textures.

Persist a bounded local preset library with five defaults, stable IDs, selection,
create/edit/delete-custom/reorder/restore operations and full style snapshots.
Migrate preferences 1–4 to 5; Settings and toolbar share one library. Tests cover
migration/validation/lifecycle, pressure fallback/type behavior, alpha joins/cache,
partial erasing/history and actual UI interactions. Gates: fmt/Clippy/tests,
native/WASM/Trunk and narrow/wide visual smoke in both layouts/themes.
No PDF persistence or note sidecars; M2 must version stroke pen type and opacity.

## M1.4 — Universal selection and lasso (#23)

Shared object identity/bounds/placement protocol with strokes as the only complete
content type; page-local or Whiteboard scope, topmost click and full-envelope lasso,
add/subtract/select-all, proportional resize and movement previews, batch journal
commands for transform/duplicate/paste/delete, focus-aware shortcuts and Edit/canvas
menus, constrained contextual controls. Preserve drawing caches/erasers and top
main toolbar. No PDF persistence or other object creation. Validate geometry,
identity/style/order/history, clipboard limits, page bounds and zoom/far-origin
input on native/WASM; record real UI verification and any unmet #23 criteria.

### Unified canvas object architecture — M1.4 foundation follow-up

Extend the existing semantic protocol with object types, shared tagged snapshots,
ID lookup/ordered traversal, pressure-aware hits, normalized local anchors,
rotation-aware placement/bounds and undoable z-order. Preserve raw vectors/style,
caches/indexes, current UI and both layouts. Tests cover lookup/unique IDs,
ownership, rotation/inversion/composition/bounds/anchors, pressure/dots, legacy
semantic decoding, stacking and exact history including transformed erasing.
Future content types extend the same protocol/store/journal. Rotation/arrange UI,
connectors and all new object creation remain separate tasks; M2 saving is unchanged.

### Selection/Lasso acceptance follow-up (#23)

Selection/Lasso already shipped in PR #28. Canvas Escape now cancels the active
selection gesture and clears membership in one step, including during a lasso.
Add controller coverage for clearing, zoom/pan envelope picking, Page boundaries
versus unbounded Whiteboards and handoff to Pen/Hand/both erasers. Preserve the
existing top toolbar, object protocol, cached geometry and journal; add no tools,
transforms or PDF persistence.

### M1.4 — Object manipulation and clipboard completion

Expose the existing rotation placement through a group handle, Shift snapping,
rotated bounds/mesh preview and keyboard-accessible 15-degree menu actions.
Add Cut with a validated internal clipboard before deletion and logical arrow/
Shift-arrow nudges. Preserve existing movement, proportional handwriting resize,
duplicate/paste/delete and exact transactional undo. Handwriting remains uniform-
scale; off-axis resizing preserves its aspect ratio/pressure nib and a 1-point
longest-extent minimum. Future nonuniform boxes require their own geometry policy.
No future object creation, Arrange UI or PDF persistence is introduced.


## M1.5 — Basic shape creation slice (#24)

Implemented semantic rectangle/rounded rectangle/circle/ellipse/triangle/diamond/
line/arrow, top picker, normalized drag preview, constraints/cancel, shared mixed
store/selection/transforms/clipboard/history and local box/endpoint editing.
Captured styling fields prepare future controls. #24 also requires formatting and
embedded/shared text, so remains open; no separate “Basic Shape Creation” issue
exists. Connectors, text and PDF persistence are not part of this slice.


### M1.5 — Shape styling and formatting slice (#24)

Implemented independent fill color/0–100% opacity/no-fill, border color/width/
opacity/pattern/no-border, start/end/both/no arrowheads and adjustable rounded
radius. Contextual menus near the selection and right-click Format share property
patches, compatible mixed-selection batches and atomic undo/redo. Continuous
pointer adjustments coalesce; movement/resize/rotation/flip and cached dragging
retain their existing controller. Creation defaults persist separately through
Settings → Drawing and are captured when creating new objects. No embedded text,
magnetic attachment or PDF saving; #24 remains open for its shared text criteria.

## Advanced connector routing (M1.7 request, #25)

Straight, bounded local orthogonal and editable cubic routes extend the same
magnetic objects. Automatic/fixed local ports, stable live attachment, manual
rail/control handles, Reset Route and shared formatting/history/selection operate
on both layouts. Global obstacle avoidance and labels remain future work.
This does not change the M2 PDF persistence gate.

## Standalone editable text boxes (#26 slice, M1.8 user request)

Text boxes now share object identity, selection, transforms, clipboard and history.
The top T tool and egui editing/layout core support wrapping, portable fonts,
rich character runs, paragraph alignment including Justify, retained formatting
selection, installed native fonts,
reversible live font previews, eight local resize handles and Unicode source.
Embedded shape text and labels remain separate hosts; #26 stays open for its broader shape-text dependency.
PDF text/font persistence and round trips remain M2.

## M1.9 — Text inside shapes

Supported closed shapes can host editable rich text as part of the same shape
object. The existing rich editor, formatting runs, selection, clipboard and
history are reused; shape geometry defines a conservative text region, with
horizontal paragraph and vertical alignment. Rotation, reflection, shape
transforms and magnetic connectors retain the canonical shape identity. Text
remains in-memory until M2 PDF persistence; text inside open line/arrow shapes
is not supported.

## M1.10 — Sticky Notes

Sticky notes reuse the existing Shape snapshot and ShapeText editor as one
object. A top-toolbar click creates a ready-to-type note; appearance uses
pastel/custom backgrounds, optional shadow and automatic height. Eight local
resize grips, transforms, clipboard/history and magnetic connectors stay on the
canonical shape ID. Per-device defaults affect future notes only. No PDF
persistence, grouping, or other planned tools are included in this slice.

## M1.11 — Grouping, locking and stacking contexts

Shared Group CanvasObjects own ordered child IDs; nested editing, proportional
move/resize/rotate/flip, clipboard/duplicate/delete and journal restore hierarchy.
Object/ancestor locks protect ordinary edits and offer right-click recovery.
Arrange changes immediate sibling order with consistent geometry hit priority.
Both layouts share the same implementation. See ARCHITECTURE/UI_DESIGN for exact
ownership, text-entry, child-lock and connector policies. All content remains
memory-only pending M2; Alignment & Smart Snapping is separate future work.

## M1.12 — Alignment, smart snapping and distribution

Screen-space object/center/edge and focused equal-spacing snapping use captured
candidates and per-axis hysteresis on existing drag/resize sessions. Explicit
alignment and equal-gap distribution share the object/connector journal and obey
group scopes/locks. TOP Snap and Settings provide independent object/grid/guide
controls, sensitivity and grid spacing on both layouts. This does not implement
automatic diagram layout, constraint solving or PDF persistence. Exact policies
and verification are in ARCHITECTURE, UI_DESIGN and TEST_PLAN.

## M1.13 — Editable standalone LaTeX formulas

Implemented as a source-preserving Formula CanvasObject with top-toolbar creation,
live math preview, color/scale controls, and compatibility with selection, transforms,
groups, clipboard, locks, alignment and snapping. This in-memory feature does not
alter the blank PDF codec or claim formula persistence. Full package loading, inline
formulas and automatic equation solving remain out of scope. See
FORMULA_RENDERER_EVALUATION.md and TEST_PLAN for dependency constraints, syntax
coverage and verification.
