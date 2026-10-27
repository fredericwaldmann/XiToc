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
