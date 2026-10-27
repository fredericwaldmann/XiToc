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
cargo install trunk --version 0.22.0 --locked
trunk serve --open
```

The browser needs WebGL2. The existing Trunk entry point starts eframe on the
full-window canvas; eframe observes canvas resizing and adjusts its drawing size.
Desktop and browser use the same `XiTocApp` implementation.

## GitHub Pages

Build for the repository subdirectory (case-sensitive):

```sh
trunk build --release --public-url /XiToc/
```

Trunk writes `dist/index.html`, JavaScript and WASM with `/XiToc/` asset URLs.
`Trunk.toml` keeps local serving at `/` and enforces Cargo.lock; the command-line
public URL overrides that local default. No hard-coded HTML base URL is needed.

In the GitHub repository, select **Settings → Pages → Build and deployment →
Source → GitHub Actions**. Push this configuration to `main`, or run **GitHub
Pages** from the Actions tab with `main` selected. The site will be available at
<https://fredericwaldmann.github.io/XiToc/> after deployment succeeds.

`.github/workflows/pages.yml` installs stable Rust, the WASM target and Trunk
0.22.0, builds the release bundle, uploads `dist/` and deploys with the official
Pages actions. Deployment permissions are scoped to the deploy job; no personal
access token or `gh-pages` branch is needed. Concurrent deployments are serialized.
Manual runs on other branches build only; only `main` can deploy.

`.github/workflows/ci.yml` retains native checks for Linux, Windows and macOS
and a web build on pushes and pull requests. It does not deploy, so the two
workflows cannot publish competing Pages artifacts. Repository settings and the
live deployment must be verified on GitHub after pushing.

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
