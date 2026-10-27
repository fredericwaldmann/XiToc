# XiToc

M0 bootable Rust UI shell, shared by native and WASM. Uses exactly eframe/egui
0.37.0 with the Glow renderer and the current `App::ui` API. No account,
backend, note storage, filesystem access or PDF implementation exists yet.

## Run

Install current stable Rust (minimum 1.95), then:

```sh
cargo run --locked
```

Native requires a graphical desktop and OpenGL support. Linux builds require
the system C compiler and development libraries required by winit/glutin.
Windows and macOS builds are included in CI; they need testing on real machines.

For web, install Trunk and the WASM target:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
trunk serve --open
```

The browser needs WebGL2. `trunk build --release --locked --public-url ./`
produces static files in `dist/`, suitable for hosting under a subdirectory.
CI builds native targets on Linux, Windows and macOS and uploads the static
web artifact. To deploy it, configure the repository's Settings → Pages source
to **GitHub Actions**; pushes to the default branch deploy after checks pass.
No hosting settings are modified by this change.

## Try the preview

- Drag the Explorer's right border to resize it.
- Select A4: a white, portrait 210:297 page fits the gray work area.
- Select Whiteboard: a grid fills the work area without page edges.
- Resize the window; switch modes with the pointer or Tab and Space/Enter.
- Reload/restart: the preview resets. No files are read or written.

The header represents a single preview, not an opened note. Drawing tools are
disabled placeholders. Drawing, pan/zoom, file tabs, save/undo shortcuts and
PDF persistence belong to later milestones. The preview mode is UI state,
not a document model or a saved note mode.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo check --target wasm32-unknown-unknown
trunk build --release
```

See [the roadmap](docs/ROADMAP.md) and [test plan](docs/TEST_PLAN.md).
