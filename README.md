# XiToc

Local-first Rust handwriting app, shared by desktop and WASM. M1 adds in-memory
vector drawing to the M0.6 light/dark interface and M0.5 foundation: distinct
**Page Notes** and **Whiteboards**, real native workspace discovery/creation, and
one-file blank PDF persistence. Uses pinned egui/eframe 0.37.0. No account,
backend or full drawing serialization exists yet. **M1 ink is memory-only:**
PDF saving/reopening of handwriting arrives in M2.

Technical orientation: [Codebase guide](docs/CODEBASE_GUIDE.md),
[Architecture decisions](docs/ARCHITECTURE_DECISIONS.md),
[Development log](docs/DEVELOPMENT_LOG.md).

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

## Create and open documents (M0.5)

On desktop, choose **Open workspace…** and select a real folder. The resizable
Explorer groups actual PDFs under **Pages** and **Whiteboards** with type icons, preserving
the actual folder hierarchy. These are virtual views, not physical category
folders. Select or right-click a folder and choose **New Document → New Page
Note / New Whiteboard**; enter a filename. Creation writes a valid blank PDF
atomically without overwriting an existing file, then opens it in a tab.

The type indicator near the title comes from PDF metadata. There is no editor
mode switch: a Page Note stays a Page Note, and a Whiteboard stays a Whiteboard.
Page Notes show A4 boundaries and page scrolling. Whiteboards are borderless:
use Hand, space-drag or middle-drag to pan; Ctrl/Cmd-wheel or pinch to zoom around the pointer,
or use +/− and Reset view. Navigation remains session-only through M1.

Close and reopen tabs, or restart XiToc and select the workspace again. Both
UUID and type are inside `XiToc/document.json`, a named PDF attachment. Ordinary
viewers display a blank A4 page for both types; that page does not constrain the
Whiteboard editor. Move/rename a file with your OS file manager, then **Refresh**
the Explorer: its location changes but its embedded identity/type does not.
Rename/move/delete controls within XiToc are deferred to M3.

On web, use **New Document**, **Create & export PDF**, **Import PDF…** and
**Export PDF…**. Browser documents are a session list, with no arbitrary folder
access. Confirm the browser download before leaving/reloading; after reload,
import the exported PDF to restore its type. Export status only confirms the dialog closed, including cancellation; the app
cannot verify completion of a browser disk save. Closing a tab
retains its bytes in the session so it can be reopened from the Explorer.

External PDFs classify as read-only Page Notes. PDF content rendering/conversion
is deferred; XiToc explicitly reports unavailable content instead of showing a
fake blank copy. Malformed/future XiToc metadata is an error. Visible external
edits with recognized metadata are also treated conservatively as read-only.
The app does not rewrite these PDFs; web export retains original bytes.

Limits: PDF input 16 MiB, edit payload 64 KiB, native workspace 4096 entries and
32 folder levels, UTF-8 filenames, no symlink traversal. Scanning occurs only on
Open/Refresh/create and is synchronous, so large workspaces can pause. Native
creation requires a filesystem supporting atomic hard-link publication; an
unsupported filesystem returns an error and preserves existing files. Linux's
workspace picker uses an installed XDG desktop portal; no GTK dependency is added.

Drawing and undo/redo work in memory in M1. Saved ink/camera state,
existing-document saves and multi-page creation remain later milestones. See the milestone checklists in
[TEST_PLAN.md](docs/TEST_PLAN.md) for native/web/restart/ordinary-viewer tests.

## Interface foundation (M0.6)

Use **Appearance → Light / Dark** to change the persisted application theme.
The top sidebar button hides/shows the resizable Explorer. Document tabs show type icons, scroll
horizontally, switch documents and close without deleting files. Hover truncated
labels to see full paths; the bottom **…** button opens the complete status message.

Whiteboards offer zoom, Reset view and an optional dot-grid toggle; camera/grid
changes are session-only. Narrow toolbars put history/zoom controls in the View menu.
Selection/Lasso, Hand, Shapes, pen presets, both erasers and Undo/Redo work in the top bar. Pen settings edits the shared preset library; eraser settings adjusts the
per-tab radius. Text remains planned; no document conversion is offered.

See [the design system](docs/UI_DESIGN.md) for palette, spacing, typography,
component boundaries and interaction conventions. UI changes must follow it.

## Drawing (M1)

Select a pen preset and drag on either document type. Use **Pen settings** for
color, width (document points), opacity, smoothing and pressure. The **Eraser**
popover selects Stroke Eraser / Partial Eraser and radius (screen points).
Stroke Eraser deletes touched strokes;
Partial Eraser removes intersecting portions and retains editable vector pieces.
Its ring shows the actual eraser radius; detection includes pen thickness.
The selected mode persists in application Settings. One sweep is one undo action.
Each pen stroke or eraser drag is one undo operation. **Ctrl/Cmd+Z** undoes and
**Ctrl/Cmd+Shift+Z** redoes in the active tab. **Hand** or **Space-drag** pans;
Whiteboards also allow middle-drag. View offers +/− and Reset; **Ctrl/Cmd-wheel**
or pinch zooms. Page Notes scroll and constrain ink to A4 boundaries; their 100%
zoom fits width. Whiteboards have arbitrary world coordinates and no page edge.

**Ink is not saved to PDF in M1.** Dirty tabs show a dot and the status warns that
changes are memory-only. Closing a dirty tab, changing native workspace or
quitting asks before discarding; browser reload/leave requests its standard
warning, subject to browser restrictions. Forced shutdown can still lose ink.
Refresh retains unsaved tabs after external PDF changes/removal. PDF export is
disabled for dirty tabs; Ctrl/Cmd+S explains that persistence is deferred. Blank
PDF creation and original-PDF export still work. No sidecars or alternative note
format are written. Discard/restart loses drawing; M2 will add synchronized visible
PDF graphics and embedded editable strokes, safe updates and full round trips.

M1.1 accelerates whole-stroke erasing with spatial candidate filtering and batched
undo/redo updates. To repeat the synthetic original-versus-indexed benchmark:

```sh
cargo test --release eraser_performance -- --ignored --nocapture
```

## Settings

Open **Edit → Settings**. General controls Explorer visibility; Appearance chooses
Light/Dark; Drawing edits the shared pen preset library and eraser defaults/mode;
Input Devices controls middle-button panning on Whiteboards. Changes apply
automatically. Use **Done** or **Escape** to close. Toolbar Pen settings shares
the same library; toolbar eraser radius applies to the active tab.

Application preferences persist separately from notes via eframe storage:
Linux `$XDG_DATA_HOME/xitoc/app.ron` (default `~/.local/share/xitoc/app.ron`),
macOS `~/Library/Application Support/XiToc/app.ron`, Windows
`%APPDATA%/XiToc/data/app.ron`; web uses the same-origin localStorage key
`XiToc/preferences`. Clearing browser site data resets preferences. Browser
storage restrictions or native storage failures can prevent persistence.
**Stroke Smoothing** in each preset editor controls subsequent strokes in all tabs:
0% Off, 25% Light, 50% Medium (default), 75% Strong and 100% Maximum. It reduces
small irregularities while preserving sharp turns and endpoints. Existing ink
keeps its captured strength. **Eraser mode** also persists in Drawing settings;
pressure settings are implemented under Drawing (see below).

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
`.app` bundles. The current release still has the M0.5 blank-document limitations above.

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

### Pressure-sensitive pen (M1.1)

Settings → Drawing offers pressure enable, sensitivity strength and minimum width.
Pressure Pen strokes use optional normalized force with the existing smoothing,
rounded vector renderer, both erasers and undo/redo. Mouse and unavailable force keep a
constant width. Preferences persist; ink still does not persist until M2.
Browser pen PointerEvents are supported where force is reported. Windows has a
source-verified egui/winit pen-force path; native Linux/macOS tablets currently
fall back to constant width. No physical pen hardware was tested. Platform details
and future native integration: [Pressure input](docs/PRESSURE_INPUT.md).

M1.3 drawing workspace: tools stay in the **top toolbar directly below tabs**.
Select a pen preset with one click; use Pen settings to edit its name, Monoline /
Pressure Pen / Highlighter type, color, width, opacity, smoothing and pressure.
Use + to create a custom copy, or manage ordering/deletion/defaults in the popover.
Settings → Drawing shares the same locally persisted preset library. Hand,
both erasers, history and zoom remain available. Selection and Lasso are functional as of M1.4.
Ink remains memory-only until M2. See [UI specification](docs/UI_DESIGN.md) and
[future creative tool dependencies](docs/CREATIVE_TOOLS.md).

M1.4 selection: click Selection in the top toolbar, click ink, Shift-click to
add strokes or Alt/Option-click to remove them. Selection’s dropdown chooses
Rectangle (default) or Lasso and retains that mode. Drag empty canvas left-to-right
for full containment (solid blue), right-to-left for crossing (dashed green),
or draw a fully enclosing Lasso. Both modes support Shift-add/Alt-remove. Drag
selected ink to move; corner handles always resize proportionally.
Escape clears selection and cancels a live lasso/transform without editing ink;
release after cancellation cannot commit the preview.
Ctrl/Cmd+A/C/V/D selects all/copies/pastes/duplicates; Delete removes selected ink;
Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z undo/redo. Edit and canvas context menus provide
the same commands. A small toolbar near the selection duplicates/deletes/scales.
Selection is page-local; copied objects can be pasted into another document.
Clipboard limits are 512 objects, 200,000 points and 4 MiB; the in-app session
clipboard is used when system integration is unavailable. Paste centers objects
in the visible scope and fits oversized groups to a page; Duplicate preserves
size and clamps its 12-point offset to the page. Ink remains memory-only until M2.

The shared canvas-object foundation in `src/object.rs` exposes stable identity,
logical geometry, placement/rotation, pressure-aware hit testing and future
connector anchors. Handwriting remains editable vector ink, with canonical order
and transactional history in `Drawing`. Rotation uses the round group handle (Shift snaps to 15°); Arrange is an internal API;
new object tools and PDF ink persistence remain future milestones. See
`docs/ARCHITECTURE.md` for the extension contract.

M1.4 manipulation: drag selected ink to move; corner handles resize handwriting
proportionally (including off-axis drags/Shift) with uniform thickness scaling.
Drag the round handle above the group to rotate, or use Edit/context Transform
rotation steps. Ctrl/Cmd+X cuts only after a validated internal copy; Ctrl/Cmd+C/V/D copy,
paste and duplicate. Arrows move 1 document point, Shift-arrows 10. Each completed
drag is one undo operation; Escape cancels and clears selection. Page ownership
stays fixed; rotation stops at conservative page bounds. No PDF ink saving.

Right-click ink → **Transform → Flip Horizontally / Flip Vertically** mirrors the
selection around its collective center. Right-click preserves a selected group
or selects an unselected target. Each flip is one undo step; raw handwriting,
styles and IDs remain unchanged. Ink still remains memory-only until M2.


Shapes in the top toolbar opens an eight-form picker: rectangle, rounded rectangle,
circle, ellipse, triangle, diamond, straight line and arrow. Drag to create; Shift
constrains proportions/angles and Escape cancels. The new shape becomes selected.
Use local corner handles to resize boxes or endpoint handles to edit lines/arrows;
move/rotate/flip/copy/paste/delete and undo/redo share the ink object pipeline.
Select shapes to edit fill, border, independent opacity, width and solid/dashed/
dotted styles in the nearby toolbar or right-click → Format. Lines/arrows support
start/end/both/no arrowheads; rounded rectangles expose radius under More.
Compatible multi-selections format in one undo step. Settings → Drawing stores
separate defaults for new shapes. Text inside shapes, magnetic connectors and PDF
persistence remain future work. Shapes and handwriting are currently memory-only.


Select a closed shape to reveal four circled-plus connection ports. Drag a port
to another shape to create a magnetic straight connector, or into empty space for
a free end. Wires follow node movement, resizing, rotation and flips. Select a
wire to drag/reconnect/detach its endpoint rings or use the existing line-format
menus. Select two shapes and press Ctrl/Cmd+Shift+K for keyboard creation. Deleting
a node detaches surviving wires; Undo restores its attachments. Copied linked
groups refer to the copied nodes. Connectors remain memory-only until M2; advanced
routing, labels and editable PDF saving are not implemented.

Connector Routing offers Straight, Orthogonal and Curved paths. Drag diamond
handles to adjust rails or Bézier controls; Reset Route restores automatic geometry.
Automatic anchor choices are optional per endpoint. These edits remain in memory
until the future PDF persistence milestone.
