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

## Desktop releases

Download archives from [GitHub Releases](https://github.com/fredericwaldmann/XiToc/releases).
No Rust installation is needed to run these builds. Archives contain only the
executable; fonts are embedded and there are currently no external app resources.
These are portable binaries, without installers, automatic updates or macOS
`.app` bundles. The current release still has the M0 preview limitations above.

| Platform | CPU | Archive | Executable |
| --- | --- | --- | --- |
| Windows | x86_64 | `XiToc-windows-x64.zip` | `xitoc.exe` |
| Linux | x86_64 | `XiToc-linux-x64.tar.gz` | `xitoc` |
| macOS | Apple Silicon (ARM64) | `XiToc-macos-arm64.tar.gz` | `xitoc` |
| macOS | Intel (x86_64) | `XiToc-macos-x64.tar.gz` | `xitoc` |

Windows: extract the ZIP, then double-click `xitoc.exe` on a Windows 10/11 x64
machine with working graphics drivers. The unsigned executable may trigger
SmartScreen or antivirus warnings; only allow it if you trust the download.

Linux: extract with `tar -xzf XiToc-linux-x64.tar.gz`, then run `./xitoc` in the
extracted directory. Builds target Ubuntu 22.04's glibc 2.35 baseline; use Ubuntu
22.04 or newer, or a compatible x86_64 distribution. A graphical X11/Wayland
session and OpenGL drivers are required. On Ubuntu, required runtime libraries
can be installed with:

```sh
sudo apt-get install libgl1 libegl1 libx11-6 libxcb1 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0
```

macOS: choose the matching CPU archive, extract it, and run `./xitoc` from
Terminal. Builds target macOS 13 or newer. They are unsigned and not notarized;
Gatekeeper may block the download. For a download you trust, follow
[Apple's instructions for opening an unidentified developer's app](https://support.apple.com/en-gb/102445).
Do not disable system-wide protections. Executable permissions are preserved in
both Linux and macOS tar archives.

### Publish a version

After merging the release workflow into `main`, update `Cargo.toml`'s package
version and regenerate/commit `Cargo.lock` if needed. The tag must equal `v`
followed by that version. For the current `0.1.0` version, once the version commit
is pushed:

```sh
git switch main
git pull --ff-only origin main
git tag -a v0.1.0 -m "XiToc 0.1.0"
git push origin v0.1.0
```

Only create a tag if that version has not already been released. Pushing any
`v*` tag starts `.github/workflows/release.yml`. The workflow discovers the binary
name using Cargo metadata and rejects tags that differ from the manifest version.
It checks formatting, tests the packaging helper, then runs Rust tests, Clippy
and a release build on all four native targets. Linux installs its windowing and
OpenGL development dependencies. Both macOS architectures use matching native
runners. Each archive is checked for its executable name, bytes, permissions and
CPU format, then uploaded using the official artifact actions.

Only after **all four** builds succeed does the publishing job download and
reverify the complete archive set. Using the built-in `GITHUB_TOKEN` and GitHub
CLI, it creates a draft with generated release notes and all four assets, verifies
the uploaded names/sizes, and then publishes. Pre-release manifest versions such
as `0.2.0-beta.1` are marked as GitHub prereleases. Published releases are never
overwritten. If uploading fails, an incomplete draft can remain unpublished;
inspect/delete that draft in GitHub before rerunning the failed workflow.

To retry an existing version tag, use **Actions → Desktop release → Run workflow**
and enter the existing tag. Select `main` as the workflow ref; that version tag
must contain the release scripts. Manual runs build the tagged commit, not the
selected branch's application. Do not move release tags to different commits.
Per-tag concurrency prevents overlapping publish jobs for the same version.

Repository setup: enable GitHub Actions and allow the actions used by the
workflow under repository/organization policies. The publishing job requests
`contents: write`; no personal access token, external publishing service, signing
certificate or extra repository secret is needed. Existing CI and Pages workflows
remain separate. Actual Windows/macOS startup and the first end-to-end release
need verification on GitHub and on desktop machines.

For local packaging checks, run:

```sh
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo build --locked --release --target x86_64-unknown-linux-gnu --bin xitoc
python3 scripts/release.py package --target x86_64-unknown-linux-gnu
```
