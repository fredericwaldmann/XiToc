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

## M1 — Canvas + A4 pen
- Device-independent logical coords; strokes and colors; pen, hand, erase entire stroke; zoom/pan; undo/redo; A4 page renderer.
- Unit tests for coordinate transforms and undo operations.

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
