# M0 review notes

The shared shell uses eframe/egui 0.37.0, `App::ui`, and `Panel::left`.
There are two Rust files: platform entry points and the shared UI. No domain,
PDF or storage modules have been introduced. Both direct UI dependencies are
pinned; Cargo.lock records the resolved dependencies. Rust 1.95 is required by
eframe 0.37; local checks used Rust 1.99.1 and Trunk 0.22.0.

Explorer is a resizable empty placeholder. A single preview header and toolbar
switch between a fitted 210:297 white page and a grid covering the canvas.
Drawing tools are visibly disabled. The application performs no file reads,
writes, saves or note uploads. There are no fabricated file entries.

## Validation

Local commands (Linux):

- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --all-targets -- -D warnings`: exit 0.
- `cargo test --all-targets`: exit 0; 0 tests (no domain logic yet).
- `cargo check --target wasm32-unknown-unknown`: exit 0.
- `cargo build --locked`: exit 0; native executable links.
- `trunk build --release`: exit 0; optimized WASM and static assets in `dist/`.
- `trunk build --release --public-url ./ --locked`: exit 0; Pages asset-path build.

Initial sandboxed dependency downloads failed with DNS resolution errors;
approved network retries fetched the dependencies. Initial compilation also
caught the removed `SidePanel` API; the final code uses 0.37's `Panel` API.

## Manual verification and limitations

Follow the M0 checklist in TEST_PLAN.md using `cargo run --locked` and
`trunk serve`. No native or browser visual smoke test was performed in this
session. Windows/macOS builds await CI. The workflow is configured, not yet
observed running. Live Pages deployment requires repository Pages configuration
and a default-branch push; no remote settings or deployment were changed here.

The whiteboard is a static preview, with no pan/zoom or document coordinates.
Modes are session UI state only. Future document creation must choose and persist
a note's mode as specified in PRODUCT.md. PDF round trips remain the M2 gate.

## GitHub Pages deployment follow-up

Added `Trunk.toml` with locked dependency resolution and local `/` serving.
`pages.yml` builds explicitly for `/XiToc/`, uploads the Pages artifact and
uses the official deployment action. It triggers on pushes to `main` and manual
dispatch; deployment is restricted to `main`. Existing native/web CI remains
in `ci.yml`, with its competing deployment removed. Cargo.toml, Cargo.lock,
the shared application, platform entry points and HTML needed no changes.

Follow-up checks on Linux: fmt, Clippy with warnings denied, tests (0 tests),
WASM check, native build, and `trunk build --release --public-url /XiToc/` all
exited 0. Both workflow YAML files parsed, and trigger, job dependency and
permission assertions passed. Local Trunk serving returned HTTP 200 for HTML,
JavaScript and WASM. The release bundle mounted at `/XiToc/` also returned
HTTP 200 for all three, with the expected URL prefix and valid WASM magic
bytes. `--open` was not exercised, and browser rendering/resizing
and live GitHub Actions deployment remain manual checks in TEST_PLAN.md.
