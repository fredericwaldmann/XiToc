# Development roadmap (one Codex task / PR per slice)

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
  pressure-ready samples default to 1.0 for mouse input.
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
- Move/select strokes, text/images, shapes, page templates, PDF import background, keyboard shortcuts, autosave recovery, smoke screenshots, accessible toolbar. Package signed installers eventually.
