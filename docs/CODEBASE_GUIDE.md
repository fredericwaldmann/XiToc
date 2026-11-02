# XiToc codebase guide

This is an implementation map audited against main commit `8fc0f94` (2026-11-02,
including PR #29). It describes shipped source, not the entire product roadmap.
See [architecture decisions](ARCHITECTURE_DECISIONS.md) for constraints and
[development log](DEVELOPMENT_LOG.md) for evidence of how the code evolved.
[ARCHITECTURE.md](ARCHITECTURE.md) retains milestone contracts;
[TEST_PLAN.md](TEST_PLAN.md) contains historical verification and manual procedures.

## Current capability boundary

XiToc edits vector handwriting in memory on Page Notes and Whiteboards. Pen
presets, smoothing, pressure where delivered, both erasers, selection/lasso,
move/proportional resize, clipboard operations and undo/redo are implemented.
Rotation and explicit z-order changes exist as domain APIs without GUI controls.
Stroke is the only implemented canvas-object content type.

The PDF codec creates and reopens **blank** native documents. It does not save or
reload handwriting. Dirty drawing disables browser PDF export; Ctrl/Cmd+S explains
the M2 limitation. Ordinary PDFs open as read-only with no PDF preview renderer.
Shapes, connectors, text, sticky notes, formulas, images, multipage creation,
browser directory handles, ink autosave and crash recovery remain planned.

## Crate and module map

The repository is one binary crate, `xitoc`, not a Cargo workspace or a library
with `src/lib.rs`. Semantic boundaries are Rust modules, not separate crates.
[Cargo.toml](../Cargo.toml) uses edition 2024, minimum Rust 1.95, exactly pinned
egui/eframe 0.37.0, and eframe's Glow rendering and persistence features.
`Cargo.lock` records transitive versions. No application backend is configured.

| Source | Responsibility and principal types/functions |
| --- | --- |
| [main.rs](../src/main.rs) | Module declarations, native `main`/`run_native`, WASM `WebRunner::start` |
| [app.rs](../src/app.rs) | `XiTocApp`, `Opened`, `Pending`; `show`, `act`, `open`, `refresh`, `creation_dialog`, preference persistence and discard guards |
| [document.rs](../src/document.rs) | `DocumentData`, `DocumentKind`, `Layout`, `PageGeometry`, `Viewport`, `Document`, `Location`, `pdf_filename` |
| [editor.rs](../src/editor.rs) | `Editor`; layout, page scrolling, Whiteboard navigation/grid, calls into `Canvas` |
| [drawing.rs](../src/drawing.rs) | `Stroke`, `InkPoint`, `Surface`, `PressureStyle`, `Smoothing`, `Transform`, `Drawing`, private `Command`; semantic mutations/history |
| [object.rs](../src/object.rs) | `CanvasObject`, `ObjectId`, `ObjectType`, `ObjectSnapshot`, `ObjectBounds`, `Placement`, `hit_stroke_path` |
| [canvas.rs](../src/canvas.rs) | `Canvas`, `Tool`, `Gesture`, `Erasing`, `Cached`, `PartialPreview`; ordered input, previews, cached mesh rendering |
| [canvas_selection.rs](../src/canvas_selection.rs) | Private `canvas::selection_ui` submodule via `#[path]`; `SelectionGesture` and additional `impl Canvas` methods for selection/clipboard/previews |
| [selection.rs](../src/selection.rs) | UI-independent `Selection`, `SelectionCommand`, `Membership`, `LassoRegion`, `move_by`, `scale_about` |
| [object_clipboard.rs](../src/object_clipboard.rs) | `ObjectBundle`; capture, bounded validation/encode/decode, paste/duplicate with fresh IDs |
| [smoothing.rs](../src/smoothing.rs) | Incremental `Preview::update`, `path`; local adaptive quadratic geometry/pressure |
| [partial_erase.rs](../src/partial_erase.rs) | `Cuts`, `PartialCut`, `clip_segment`, `clip_tapered`, `path_pressures`, `fragments` |
| [eraser_index.rs](../src/eraser_index.rs) | `EraserIndex`; revision-keyed object hierarchy, path-block hierarchy and bounded spatial queries |
| [pen.rs](../src/pen.rs) | `PenType`, `PenStyle`, `PenPreset`, `Presets`; validated style library and lifecycle |
| [settings.rs](../src/settings.rs) | `Preferences`, `DrawingPreferences`, `Theme`; migration, validation, eframe storage adapter |
| [pdf.rs](../src/pdf.rs) | In-memory `encode`/`decode` for blank PDF 1.7 and named edit attachment |
| [storage/mod.rs](../src/storage/mod.rs) | Shared `Entry`, `Folder`, `Loaded`; cfg-selected storage adapters |
| [storage/native.rs](../src/storage/native.rs) | `Workspace`; selected-root capability, load/scan/create, `write_new_in` |
| [storage/web.rs](../src/storage/web.rs) | Async import/export fallback and `Event` completion messages |
| [web_pressure.rs](../src/web_pressure.rs) | WASM-only primary-pen PointerEvent bridge; `install`, `inject` |
| [ui/mod.rs](../src/ui/mod.rs) | Presentation modules and `Action` routing contract |

`document`, `drawing`, `object`, `pen`, `selection`, smoothing and clipping operate
without egui or filesystem/browser APIs. `settings` includes framework storage;
it is not a pure domain module. `canvas` deliberately combines input and egui
rendering. `storage/web` also takes an egui context to wake the app on completion.
These are current boundaries, rather than the proposed directory tree in older docs.

## Bootstrap, state and frame lifecycle

Native `main` calls `eframe::run_native` with a 1100×760 initial viewport and
480×360 minimum, constructing `XiTocApp::new(cc.storage)`. WASM `main` starts a local
async task, finds `#xitoc-canvas` in [index.html](../index.html), starts `WebRunner`,
and installs the pen bridge. Startup removes the loading status on success or
shows a WebGL-related failure message. [Trunk.toml](../Trunk.toml) builds locked
Rust dependencies; local URLs use `/`, Pages builds override this with `/XiToc/`.

`XiTocApp` implements eframe 0.37's `App::ui`, not the older `App::update` API.
`ui` calls `show`, then `persist_preferences`; `save` stores application
preferences, **not notes**. `persist_egui_memory` returns false. On WASM,
`raw_input_hook` injects queued pen samples before the shared controller runs.

Main app state comprises:

- Application preferences, Settings dialog, write-permission guard/warnings and
  a preference-change timestamp.
- Explorer `Folder` tree, selected creation folder, open `Vec<Opened>`, active
  tab index, creation dialog and status text. Each `Opened` has original `Loaded`
  bytes/document and a separate `Editor`.
- Chrome/tab presentation state and pending close/workspace/quit confirmation.
- Native `Option<Workspace>`, or WASM session `browser_files` plus an mpsc
  sender/receiver for async import/export completion.

`show` polls browser events, guards native close requests, applies the theme,
builds menus/status/Explorer and the document area, dispatches actions, then shows
creation/Settings/discard dialogs. Settings or pending discard disables the
underlying editor. Canvas shortcuts defer when egui text input is focused.
Switching tabs finalizes active ink/erase gestures; transient selection transforms
are cancelled by `Canvas::finish`.

## Documents, identity and storage

### Semantic data versus editor session

`DocumentData` schema 1 has private `schema_version`, UUID `document_id` and
`layout`. Kind derives from `Layout`; there is no independently mutable type
switch. `blank(Page)` creates one 210×297 mm A4 page. `blank(Whiteboard)` creates a
viewport centered at zero, zoom 1, with grid enabled. Validation currently permits
only one default A4 page or valid Whiteboard settings (zoom 0.1–8).

`Document` wraps data, a `Location` string and an `external` read-only marker.
Paths are locators, not native-note identity. `Loaded` retains original bytes as
`Arc<[u8]>`; cloned browser cache/tab entries share the byte buffer. An external
PDF without XiToc metadata receives a fresh Page `DocumentData` on decoding; that
classification UUID is temporary and is not a persistent identity from the file.

The `Editor` owns its own viewport/grid/navigation and `Canvas`; `Canvas` owns
`Drawing`. Ink is not a field in schema-1 `DocumentData`. Changing camera, grid,
selection or pen does not update the PDF attachment. There is no saved-ink revision.

### Creation and opening

Native actions call `Workspace::choose/open`, `scan`, `load` or `create` through
`app.rs`. `pdf_filename` validates a single portable filename (including reserved
Windows names); creation chooses type once. `create` encodes blank data and
publishes a fully written file without replacing an existing filename.
`open` deduplicates tabs by location and refuses to replace dirty ink when source
bytes changed. Native `refresh` rescans and follows uniquely matched document
UUIDs after external moves. Missing/ambiguous/unreadable clean tabs can close;
dirty tabs are retained even if source files changed or vanished.

Native storage uses `cap_std::fs::Dir` rooted at the user-selected directory.
Relative paths reject traversal/non-normal components and symlinks; subsequent
IO remains capability-relative. Scans are synchronous, capped at 4096 entries and
32 nested levels, skip symlinks, require UTF-8 names and decode discovered PDFs
to classify them. `write_new_in` writes a unique temporary file, syncs bytes,
atomically hard-links the destination with no-clobber semantics, removes the temp,
and syncs the directory on Unix. This is **new blank-file publication**, not an
implemented atomic update/save mechanism. Unsupported hard links fail safely.

Web uses `rfd::AsyncFileDialog`, `spawn_local` and channel events. Import checks
file size before reading its ArrayBuffer, decodes it and adds a flat session entry;
duplicate names are rejected. Creation builds blank bytes in memory and requests
export. Export writes original bytes through the dialog adapter. Completion means
the dialog/write path completed; the status explicitly asks users to confirm any
browser download and does not certify permanent storage. No directory handles,
recursive disk scanning or note uploads exist. Closing/reopening session tabs can
reuse original bytes; browser reload loses the session list and ink.

### PDF boundary and data safety

`pdf::encode` uses `pdf-writer` to emit one blank A4 page and an EmbeddedFiles name
tree entry `XiToc/document.json` containing schema-1 JSON. Whiteboards also use this
finite blank A4 viewer fallback. `decode` uses strict `lopdf` parsing, rejects
unsupported/encrypted/corrupt input, validates metadata and detects duplicate
attachments. Limits are 16 MiB input/parser decompression, 64 KiB edit payload,
and attachment traversal depth 16/budget 256 nodes.

Without the attachment, readable PDFs classify as external Page documents.
Recognized metadata is still read-only unless the codec's conservative blank-page
check succeeds: one page, an empty content stream and no `Annots` entry. This is
not general PDF graphics rendering or proof that arbitrary third-party edits
preserve XiToc data. Unknown schema/fields are errors, not silent blank fallback.

`Drawing::dirty` tests whether committed ink is nonempty; `Canvas::dirty` also
includes active pen samples. Thus returning to empty ink returns to the blank
baseline. Dirty close/workspace/quit paths prompt before discard; WASM publishes
`xitocHasUnsavedChanges` for `beforeunload`. Forced termination or suppressed
browser prompts can still lose ink. No recovery journal, note autosave or PDF ink
save is implemented. M2 must version the complete editable snapshot and emit
matching visible graphics/resources and edit data inside the same PDF.

## Coordinates and ownership

Page geometry is millimeters; stroke points/widths use PDF points (72/inch),
converted by `POINTS_PER_MM = 72 / 25.4`. `Surface::Page(usize)` is explicit
page-local ownership; `Surface::Canvas` is Whiteboard ownership. Page ownership
is never inferred from position. Page indices are not stable PageIds for future
page reordering.

`drawing::Transform` maps document points to logical screen points:

```text
screen = origin + (document - center) * scale
```

Subtracting centers in f64 before egui f32 conversion preserves local precision
far from Whiteboard origin. This view transform is separate from object placement:

```text
world = position + R(rotation) * scale * (local - origin)
```

`Placement` retains raw stroke coordinates and uses a positive uniform scale
(1e-4–1e4) plus finite radians. `then` composes around a raw reference point;
`inverse` maps partial-erase survivors back to local coordinates. Ink width scales
with placement. `ObjectBounds::placed` encloses all four transformed corners.

`Editor::pages` fits paper width with margins (210–794 screen points before zoom),
uses a two-axis `ScrollArea`, and clips each surface to its A4 rectangle. Page zoom
changes paper size/layout; it does not implement the same pointer-anchored camera
formula as Whiteboard. `Editor::whiteboard` uses camera-centered transforms,
Hand/Space or optional middle-button pan and pointer-anchored zoom. Both consume
egui zoom deltas (Ctrl/Cmd-wheel or pinch), clamped to 0.1–8. View controls zoom
around the viewport center. Navigation is session-only.

## Drawing pipeline

### Input and stroke capture

`Canvas::begin_frame` guards cumulative frame number, updates selection, processes
focus-aware commands and finishes ink if focus/UI enablement is lost.
`surface` processes ordered egui events once per surface/frame and checks clip
rect, top UI layer, tool, Space state and ownership before starting a gesture.
Primary pointer-down creates `Stroke::new`, captures effective pen type/RGBA/
width/pressure response and `AdaptiveV1` smoothing strength, then appends its
first logical `InkPoint`. Moves and release pass through `sample`; release
samples its final position before finishing. Page excursions end ink rather than
bridging margins. `end_frame` catches releases outside a visible surface.

Raw `InkPoint { position: [f64; 2], pressure: Option<f32> }` samples remain editable.
`None` means unavailable, including mouse; zero is a real pressure measurement.
`normalize_pressure` clamps finite input to [0,1], rejecting invalid numbers.
Native Touch events pair force with the primary contact's pointer stream and
exclude secondary-contact force. WASM's JS bridge captures primary pen
PointerEvents, prevents duplicate compatibility input, queues pressure/position,
wakes repaint and converts samples through `pen_events`/`raw_input_hook`.
Mouse and ordinary touch remain on eframe's path. Linux/macOS native force falls
back to constant width; Windows has a source-verified winit force path, without
hardware certification. See [PRESSURE_INPUT.md](PRESSURE_INPUT.md).

### Smoothing and pressure

`Stroke::path` dispatches `Smoothing::Legacy` midpoint quadratics, `PolylineV1`
explicit survivor paths, or `AdaptiveV1 { strength }`. Adaptive smoothing uses a
local convex three-point filter and trimmed quadratic joins. Smoothing fades at
sharp turns toward 75 degrees; endpoints stay exact, and 0% preserves raw input.
`Preview::update` only revisits the bounded trailing vertices and returns the
first changed path offset. Pressure follows the same interpolation weights.
Release uses the preview geometry rather than a different fitting algorithm.

`PressureStyle::factor` computes `1 + strength * (minimum + (1-minimum)*p - 1)`
with percentage settings normalized to fractions. Missing/disabled force uses 1.
Actual width is captured nominal width × placement scale × pressure factor.
Original pressure samples and captured response remain independent of later preset
changes. Legacy semantic samples have defaults; JSON round trips are tested, but
these are not PDF ink round trips.

### Presets and rendering

`PenStyle` captures type, RGB, width, opacity, smoothing and pressure.
`Presets` holds at most 32 stable-ID entries and selected ID, including five
editable/reorderable defaults (Black/Blue/Red Monoline, Highlighter, Pressure).
Only custom entries may be deleted. `effective_pressure` enables force thickness
only for Pressure Pen. The toolbar applies the selected style for subsequent
strokes; pointer-down snapshots insulate active/existing ink from preference edits.

`Cached` holds document-space path/pressures, placement, view/DPI key, path bounds
and `Arc<egui::Mesh>` chunks. `Canvas::paint` clips/culls surfaces and strokes;
`Cached::prepare_meshes` converts logical geometry into view-dependent meshes.
Chunks cover 128 path segments. Adaptive live ink invalidates only changed trailing
chunks; idle frames and release reuse stable meshes. Camera/DPI changes rebuild
mesh coordinates without rewriting semantic points. Opaque ink uses round
constant/variable-width geometry; translucent ink uses joined ribbon strips to
avoid overpainting each sample join/chunk seam. Genuine crossings and separate
strokes composite normally. Highlighter hit envelopes remain conservatively rounded.

### Erasing

Both modes use screen-point eraser radius divided by view scale, plus visible ink
width in the narrow phase. `EraserIndex::sync` builds a revision-keyed hierarchy
from conservative transformed raw-point bounds. Queries filter by surface and
swept bounds before cached-path tests; empty distant movement avoids full path scans.

Whole-stroke erasing accumulates touched IDs and hides them during preview;
`hit_stroke_path` handles pressure-aware swept hits. Release calls `Drawing::erase`
once. Partial erasing lazily creates `PartialCut` with an index over blocks of
16 rendered segments. `clip_tapered` analytically intersects the swept capsule
with variable-width segments; `Cuts` merges scalar path intervals. Preview only
rebuilds affected mesh chunks. Release creates fragments once, maps endpoints
back through placement inverse, tags them `PolylineV1`, and assigns fresh UUIDs.
Fragments retain captured style and interpolated pressure. `replace_partial`
validates and journals one replacement; undo restores original IDs/raw samples,
redo restores the same fragments and order. No raster flattening occurs.

### From pointer movement to visible ink

```mermaid
flowchart TD
    A[Native pointer or touch through eframe] --> D[Ordered egui input events]
    B[Browser primary pen PointerEvents] --> C[web_pressure queue and raw_input_hook]
    C --> D
    E[Browser mouse and touch through eframe] --> D
    D --> F[XiTocApp::show calls Editor::show]
    F --> G[Canvas::begin_frame and surface]
    G --> H[Transform::to_document and sample]
    H --> I[Active Stroke raw InkPoint samples]
    I --> J[update_preview and smoothing::Preview::update]
    J --> K[Cached path and pressure; invalidate trailing mesh chunks]
    K --> L[Canvas::paint and Cached::prepare_meshes]
    L --> M[egui painter meshes through eframe Glow]
    I --> N[Release or finish]
    N --> O[Drawing::add and Command journal]
    O --> P[Committed Arc Stroke; existing preview cache retained]
    P --> L
    O --> Q[Dirty tab and discard guards]
```

Drawing never passes through PDF encoding in this flow. Erasing and selection
consume the same document paths/caches through their own gesture handlers.

## Objects, selection and history

`CanvasObject` provides identity, document/surface, local/world bounds, placement,
object type, point hit testing and normalized local-bound anchors. `ObjectId` is a
UUID alias, `ObjectType`/tagged `ObjectSnapshot` have only Stroke. There is no
heterogeneous canonical object container yet: `Drawing` stores `Vec<Arc<Stroke>>`,
which is also the sole back-to-front z-order. `objects()` is a trait-object view;
`object(id)` currently performs linear lookup. `set_z_order` is an internal
undoable API without Arrange controls. `anchor([u,v])` maps normalized local
bounds through placement, preparing future ID-based connector attachments without
implementing a connector or dependency graph.

`selection.rs` stores transient IDs, scoped surface and derived bounds; it is not
saved content. `Canvas::pick` in `canvas_selection.rs` queries the hierarchy and
checks cached pressure envelopes, taking the highest vector index. Selection
click tolerance is 4 logical screen points converted by zoom; Shift toggles and
Alt subtracts. Lasso uses a closed even-odd polygon and requires full rounded
pressure-envelope containment; boundary touch/crossings are excluded. Concave and
self-crossing cases use segment/edge tests, not only bounds. The gesture caps at
4096 vertices; overly complex/degenerate/cancelled gestures do not alter content.

Move/proportional corner-resize gestures preview cached geometry without changing
committed objects on every pointer move. Release commits one `transform_objects`
transaction, retaining IDs/raw ink/style. Escape, focus loss or view changes
cancel selection previews. Page moves/resizes stay on the source surface with
centerlines constrained to its bounds; rendering clips ink edges. Rotation works
in the domain and erasers but has no handle/control. Nonuniform scaling is absent.

Copy/duplicate/paste use the same `ObjectBundle` with tagged snapshots and version
1. Validation caps payloads at 4 MiB, 512 objects and 200,000 raw points; it rejects
invalid geometry, duplicate IDs and unknown versions/variants. Paste remaps object,
document and surface identities, preserving style/order and fitting the target
page when needed. Clipboard data is transient interchange, not a native note file.
An egui-context session bundle supplements system clipboard text. Native menu
Paste requests a paste event then falls back after a short timeout; web menu Paste
uses the session bundle directly, while DOM paste can provide system payloads.

`Drawing` owns an unbounded command vector, cursor and monotonically changing
revision. Private `Command::{Insert, Erase, Replace}` retains affected Arc strokes
and indices. Replacement supports partial erasing, batch insertion and transforms;
z-order replacement retains its affected interval. Undo/reapply use linear
compaction/backward insertion to restore exact sequence. New commands truncate
redo. There is one history per open editor, not an application-global or separate
selection history. `Canvas::undo/redo` finalizes ink first, prunes obsolete caches
and refreshes selection. Revision changes invalidate derived indexes. History and
selection are not persisted.

## GUI composition and application preferences

| Presentation source | Actual role |
| --- | --- |
| [ui/chrome.rs](../src/ui/chrome.rs) | Menu/status chrome, theme/Explorer controls, welcome state |
| [ui/explorer.rs](../src/ui/explorer.rs) | Recursive real `Folder` tree projected into virtual Pages/Whiteboards, selected creation folder, file/error actions |
| [ui/tabs.rs](../src/ui/tabs.rs) | Horizontally scrolling typed tabs, activation/close, dirty marker, tooltip and visibility state |
| [ui/toolbar.rs](../src/ui/toolbar.rs) | Fixed top toolbar directly below tabs: Selection/Lasso/Hand, presets, preset editor, eraser, history, view/export overflow |
| [ui/drawing_controls.rs](../src/ui/drawing_controls.rs) | Shared preset/style and eraser-radius widgets used by Settings and toolbar |
| [ui/settings.rs](../src/ui/settings.rs) | Scrollable modal General/Appearance/Drawing/Input Devices sections |
| [ui/selection_controls.rs](../src/ui/selection_controls.rs) | Shared menu state/actions and constrained contextual Duplicate/Delete/Shrink/Grow controls |
| [ui/dialog.rs](../src/ui/dialog.rs) | Named, typed creation dialog and validation feedback |
| [ui/style.rs](../src/ui/style.rs), [ui/icons.rs](../src/ui/icons.rs) | Central light/dark tokens/dimensions and original painted vector icons |

Explorer categories do not create directories or duplicate PDFs; both retain real
folder hierarchy, including empty folders. Mutation controls for rename/move/delete
are not implemented. `ui::Action` returns coordination requests to the app; widgets
also directly modify transient editor/preferences where appropriate. No UI widget
performs filesystem or PDF IO. Whiteboard grid/paper backgrounds are painted in
`Editor`, handwriting/selection in `Canvas` and its private selection adapter.

`Preferences` schema 5 persists separately under `XiToc/preferences` in eframe
storage (native app data, browser localStorage). Versions 1–4 migrate, preserving
legacy ink configuration as a Migrated pen. The app debounces changes while
Settings is open and flushes on close/normal save hooks; a malformed/future blob
is preserved until an explicit preference change. Storage warnings are surfaced;
browser read-back detects blocked/quota failure, while native eframe storage
cannot report durable disk success through its trait. Workspace paths, note ink
and camera are not preferences. No user account or remote note service exists.

## Tests, CI and practical navigation

Rust tests live in `#[cfg(test)]` source modules. There is no standalone `tests/`
integration suite. Many unit-module tests are cross-component integration tests:
synthetic egui frames feed actual pointer/force/key events, app tests open real
temporary workspaces, storage tests inject IO failures and PDF tests write with
`pdf-writer` and parse with `lopdf`. [selection_tests.rs](../src/selection_tests.rs)
is included by the private selection adapter via `#[path]`; it is not a second
editor implementation. Tests assert cache Arc reuse, index pruning, exact history,
legacy decoding, clip geometry, ownership, layout/focus and discard behavior.

At the audited revision there are 103 Rust tests: 98 normally run and five ignored
release benchmarks (whole erase, partial document, partial geometry, smoothing,
indexed lasso). Historical recorded results are in TEST_PLAN, not proof of hardware
latency or a fresh run of every platform. [scripts/test_release.py](../scripts/test_release.py)
adds Python unittest fixtures for archive completeness, corruption/path safety,
CPU headers, executable bits, manifest/tag matching and Cargo metadata discovery.
Header fixtures do not prove that binaries launch.

| Workflow | Trigger and checks |
| --- | --- |
| [ci.yml](../.github/workflows/ci.yml) | Push/PR/manual: Linux/Windows/macOS fmt, locked Clippy with warnings denied, tests, native build; Linux wasm32 check and Trunk release build for `/XiToc/` |
| [pages.yml](../.github/workflows/pages.yml) | Main push/manual: WASM check, Trunk build, Pages artifact; deploy job restricted to main, Pages/id-token permissions isolated from build |
| [release.yml](../.github/workflows/release.yml) | Version tag/manual tag: validate Cargo metadata/tag, fmt/Python tests; native tests/Clippy/release builds for Windows x64, Linux x64, macOS arm64/x64; verify all archives then create draft, verify assets and publish |

PR CI does not run interactive browsers, physical stylus tests, Python packaging
tests or ignored benchmarks. Packaging tests run in release preparation. Pages
builds are not note storage or cloud upload. Release archives are unsigned
standalone executables, not installers/.app bundles. See [scripts/release.py](../scripts/release.py).

For code exploration, follow `main → XiTocApp::show → Editor::show → Canvas::surface`.
For geometry changes, read `Stroke::path`, smoothing/clipping, `Cached` and the
regression tests together. For persistence work, start at `DocumentData::validate`
and `pdf::encode/decode`; do not mistake clipboard serde or retained original PDF
bytes for a complete saved document.

## Documentation audit findings

The audit compared code and tests with the existing milestone documents. Corrections
in this change are documentation-only; old performed-verification records and
screenshots remain attributed to their original milestone.

| Earlier documentation claim or ambiguity | Source-based finding and resolution |
| --- | --- |
| ARCHITECTURE's proposed `lib.rs`/`domain/` tree looked like a source map | Actual binary uses flat modules; proposal is now explicitly labeled and linked to this guide |
| ARCHITECTURE suggested evaluating lopdf and described full saves in present tense | lopdf already implements the blank-only reader; full ink save/update and browser directory permissions are future work |
| PRODUCT/README still put selection in M6 or called it disabled | M1.4 enables Selection/Lasso and transformations; current passages corrected |
| PRODUCT listed pressure/partial erasing as future and autosave/save without a current boundary | Force paths and both erasers exist in memory; note autosave, recovery and ink persistence do not |
| README/PRODUCT described the removed Ink control and independent per-tab pen settings | Current toolbar and Settings share application preset state; active strokes capture style, eraser radius is per-tab |
| README called theme session-only | Preferences persist it independently of notes |
| ROADMAP said mouse pressure defaults to 1.0 | `InkPoint` uses None; fallback width is constant, not invented hardware force |
| CREATIVE_TOOLS described intersection lasso | Implemented rule requires full strict envelope containment; touch/crossing excludes strokes |
| Generic zoom descriptions suggested identical pointer anchoring | Whiteboard uses camera anchor math; Page zoom changes paper layout in its ScrollArea |
| M0_REVIEW described two source files and an editor mode toggle | Correct historical record, now explicitly labeled historical, not current architecture |
| TEST_PLAN's general checklist looked like all features were tested/implemented | Added scope note; ink PDF round trips, multipage and browser directory acceptance remain future gates |
| Older milestone scopes/counts appeared alongside newer ones | Retained as historical records, with this guide for current behavior and DEVELOPMENT_LOG for attribution |

Additional roadmap prose still intentionally states future requirements. In
particular PDF_FORMAT's example full edit payload is conceptual, not the
schema-1 `DocumentData` accepted by code. UI_DESIGN is a specification plus milestone
interaction contract, not evidence of hardware behavior. The development log
preserves the superseded floating palette rather than treating its screenshots as
current design. See the decision record's audit follow-ups for architectural risks.

Useful verification commands (run selectively for the change being made):

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked
cargo check --locked --target wasm32-unknown-unknown
trunk build --release
cargo test --locked --release -- --ignored --nocapture
python3 -m unittest discover -s scripts -p 'test_*.py'
```

For this documentation audit, validation checks documentation links, recorded
symbols/test inventory, source/history agreement and the documentation-only diff.
Historical successful application checks above are not presented as fresh runs.
