# Codex instructions — XiToc

Read `docs/PRODUCT.md`, `docs/ARCHITECTURE.md`, `docs/PDF_FORMAT.md`, `docs/ROADMAP.md`, and `docs/TEST_PLAN.md` before changing architecture or storage.

For UI changes, also read and follow `docs/UI_DESIGN.md`; keep centralized tokens
and the documented design system in sync.

## Mission and non-negotiable requirements

Build a local-first Rust note-taking app, with one shared editor for desktop (Windows, macOS, Linux) and web (WASM). Show a VS Code-style Explorer of real folders and `.pdf` files on the left. Support (1) A4 paginated documents and (2) an infinitely pannable/zoomable whiteboard. Both support freehand pen drawing with the same document model.

Each native XiToc note MUST be a single, valid, independently viewable PDF. All information required to re-edit handwriting and objects must reside INSIDE that same PDF, not in adjacent `.json`, `.db`, `.note`, or other sidecars. Browser temporary storage is allowed for unsaved state, but the final canonical saved note is always one PDF. Do not claim that arbitrary externally edited PDF files retain the private edit payload.

## Platform policy

- Rust is the product language. Use `egui` + `eframe` for the UI and `trunk` for WASM packaging. Pin compatible egui and eframe releases. Use APIs for egui 0.37 (or a consciously verified newer version), NOT the older eframe_template `App::update` example.
- Separate platform-neutral document model, rendering, and PDF codec from platform-specific storage.
- Native storage opens a user-selected real workspace directory and uses actual folders and `.pdf` files. Avoid scanning outside the selected workspace and reject path traversal/symlink escape where appropriate.
- Web storage uses browser directory APIs only when available and permissioned (Chromium where supported); provide import/export fallback for other browsers. Never imply websites can silently scan local disks.
- No user account, remote backend, telemetry, advertising, cloud API, or remote note upload in the MVP.

## Architecture rules

- Shared semantic types must not depend on egui, filesystem, or browser APIs. Keep serde models versioned and testable in memory.
- Use stable object IDs and logical document coordinates, independent of screen pixels.
- Page Notes and Whiteboards are immutable document types chosen at creation, never an editor mode toggle. Derive layout from PDF metadata; no conversion UI. Use a single editor input controller and rendering pipeline; document kind affects page bounds, coordinate transform and export only.
- Explorer Pages/Whiteboards sections are virtual categories over real workspace-relative paths. Preserve folder hierarchy and PDF identity; never manufacture category folders or duplicate files. Ordinary external PDFs classify as Page but remain read-only until explicit conversion is implemented.
- M0.5 may create valid blank PDFs with the named embedded payload. This is only a blank-document subset, not completion of M2 handwriting serialization. Keep future/unknown payloads explicit errors and never rewrite unfamiliar PDF graphics.
- Treat PDF output as TWO synchronized parts: conventional visible PDF graphics/pages, and a named embedded edit-data file (`XiToc/document.json`) described in `docs/PDF_FORMAT.md`.
- Preserve user data: save to a temporary file and atomically replace on desktop when feasible, preserve last valid revision on failure, never overwrite on parse error, and test crash/error paths. On web do not report "saved" until an actual file write/export completes.
- PDF round trips are a milestone gate. Test with a normal PDF viewer AND by loading edits back into the app.
- Existing third-party PDFs without XiToc edit data should initially open read-only or offer an explicit import/conversion. Do not silently discard or rewrite unfamiliar PDF structures.

## Coding agent workflow

1. Before writing code, read relevant docs and say which milestone and files will change.
2. Keep one PR/task small and focused. Prefer tests plus implementation together.
3. Respect crate boundaries; do not mix filesystem IO into egui widgets.
4. Verify desktop and web compatibility for shared code; `cfg` target-specific integrations.
5. Run (as relevant):
   - `cargo fmt --all -- --check`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test --all-targets`
   - `cargo check --target wasm32-unknown-unknown`
   - `trunk build --release`
6. If the environment cannot run a check, report it without claiming success.
7. Do not downgrade security checks or remove tests to make CI green. Keep `Cargo.lock` committed for reproducibility.
8. For each PR summarize behavior, manual test instructions, risks, and any follow-up issues.

## Product quality

- Common shortcuts: Ctrl/Cmd+S save, Ctrl/Cmd+Z undo, Ctrl/Cmd+Shift+Z redo, space-drag pan, mousewheel zoom with modifier (define and document exact gesture); keyboard accessibility.
- Pen input must not change depending on monitor DPI or zoom; erasing individual strokes requires preserving input points, not just a flattened bitmap.
- Canvas must retain precision when moved far from origin. Limit expensive per-frame work and memory allocations.
- MVP priorities are user data safety, correct PDF reopening, and cross-platform build ahead of more tools or visual polish.

## Definition of done

Code formatted, linted, tested; README/docs updated; both native and WASM compile; feature can be verified manually; no loss of data; CI remains green. Feature-specific requirements are documented in `docs/ROADMAP.md` and `docs/TEST_PLAN.md`.
