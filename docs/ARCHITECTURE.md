# Architecture

## Start simple
Use one Rust crate before splitting into workspace crates. Suggested modules (introduce each as needed):

```
src/
  main.rs                # native + WASM bootstrap
  lib.rs                 # shared exports
  app.rs                 # eframe Application
  domain/{mod.rs,document.rs,object.rs,commands.rs}
  canvas/{mod.rs,view.rs,input.rs,render.rs}
  ui/{mod.rs,explorer.rs,toolbar.rs,editor.rs}
  pdf/{mod.rs,encode.rs,decode.rs}
  storage/{mod.rs,native.rs,web.rs}
```

## Dependencies
- egui/eframe 0.37 or same-family compatible verified versions, with supported renderer.
- serde + serde_json: versioned editable document representation.
- pdf-writer: generating visible PDF and embedded file objects; evaluate lopdf for reads or consider a dedicated reader, verify WASM support and security.
- web-sys/wasm-bindgen for browser file APIs; JS interop only at the boundary.
- Use egui tree widgets or small internal recursive tree; adopt third-party tree crate only after checking compatibility with current egui release.

## Boundaries
`domain` has no dependency on egui, filesystem, browser APIs or PDF mechanics. `canvas` draws from domain types and maps logical to viewport coordinates. `pdf` encodes/decodes domain into a single valid PDF. `storage` reads and writes bytes by user-granted handles/paths; UI is agnostic to platform.

## M0.5 implementation boundaries
- `document.rs`: serde schema, UUID identity, `DocumentKind`, immutable `Layout`,
  page geometry and whiteboard viewport/grid settings. No egui, filesystem or
  browser types. `Location` is a separate string locator; moving a PDF changes
  that locator, never the semantic data embedded in the file.
- `pdf.rs`: in-memory blank-document writer (`pdf-writer`) and reader (`lopdf`,
  without native-only defaults). PDF 1.7 attachment name trees carry the payload.
  Input is limited to 16 MiB, decoded payload to 64 KiB, parser stream inflation
  to 16 MiB and attachment traversal to 16 levels/256 nodes. Unsupported or
  malformed metadata is an error, not a silent fallback or rewrite.
- `storage/mod.rs`: shared Explorer tree and loaded-document result types.
  `storage/native.rs`: user-selected workspace capability (`cap-std`), bounded
  real-folder scans, no symlink following or traversal escape, atomic no-clobber
  blank-file creation with synced temporary bytes. IO is triggered only by user
  actions, not per frame. Scans are synchronous and bounded to 4096 entries/32
  directory levels; large workspaces can pause during Open/Refresh. Filesystems
  without hard-link support fail safely instead of falling back to partial writes.
- `storage/web.rs`: explicit browser file import and download export, with file
  size checks before reading. The export callback only confirms dialog closure (including cancel), never
  verified permanent storage. The original imported PDF bytes are retained for export;
  unfamiliar PDFs are never reconstructed or silently converted.
- `editor.rs`: common editor initialized from document layout; page geometry and
  whiteboard camera affect presentation and navigation only. World coordinates
  stay f64; only local offsets become screen f32. No type setter/conversion tool.
- `app.rs`: application state and action coordination, clear read-only/error
  state. Closing a tab does not delete a file. Browser session bytes survive tab
  closure but not reload. Desktop files are reread from their real location. Refresh follows uniquely
  matched native UUIDs after external moves, updates tab locators, and closes
  missing/ambiguous/unreadable tabs. Original PDF bytes are shared across browser
  cache/tabs rather than duplicating each imported byte buffer.

## Core state
`DocumentData { schema_version, document_id, layout }`, with a tagged `Layout`:
`Page { pages }` or `Whiteboard { viewport, grid }`. `DocumentKind` is derived from
that layout, so kind and editor settings cannot disagree. `Document` wraps data,
a file locator and a read-only/external marker. The M0.5 schema permits only one
blank A4 page (or a whiteboard fallback); future object fields/expanded schema
must be versioned deliberately. No objects are flattened or serialized yet.

Later: Object variants Stroke { id, points, color, width }, Text, Shape and Image.
Keep stable IDs, logical f64 coordinates, command-based undo and one rendering
pipeline. Camera changes in M0.5 are session-only; future trusted-model saves
must serialize the current editor snapshot together with visible PDF graphics.

## Storage lifecycle
User opens a workspace. Explorer lists nested folders and actual PDFs. On edit, flag dirty; save serializes editable JSON + generates visible PDF + embeds metadata into PDF bytes. Native writes atomically where possible, web obtains explicit directory write permission or exports a new `.pdf`. Never silently claim a successful save after browser download when there was no write handle.

## Build constraint
Make native and WASM compile from the first milestone. If a library is native-only, hide it behind target-specific modules, not behind a broken compile path.

## M0.6 UI boundaries

`ui/style.rs` centralizes light/dark tokens; `ui/icons.rs` paints original vector
icons. `chrome`, `explorer`, `tabs`, `toolbar` and `dialog` are small presentation
components. They return actions; IO and document creation remain coordinated by
`app.rs`. The app assembles panels instead of one large rendering function.
`editor.rs` remains the shared metadata-driven renderer/navigation controller.
Theme and Explorer visibility are application preferences as of M1.1; the
Whiteboard grid toggle remains session-only UI state. No domain, PDF payload, canonical storage or save lifecycle changes.
Follow `UI_DESIGN.md` for dimensions, palette, interaction and responsiveness.

## M1 drawing boundaries

`drawing.rs` is independent of egui, storage and PDF mechanics. `Stroke` contains
its stable UUID, owning document UUID, `Surface::Page(index)` or `Surface::Canvas`,
ordered `InkPoint { position: [f64; 2], pressure }`, RGBA color and width. Coordinates
and widths are PDF points (72/inch); A4 geometry converts explicitly from mm.
Mouse pressure defaults to 1.0; M1 uses constant width. Stroke types derive serde
for future integration, but are not written as a new format or added to schema 1.

Each opened editor owns a `Drawing` with immutable owner/layout, strokes and a
command journal. Insert and Erase commands store affected objects/original indices
using shared references, not whole-document snapshots. Undo reverts; redo reapplies;
a new command truncates the redo branch. One swept eraser gesture commits one
command, restoring erased stroke identity and stacking order on undo. Future
shape/text commands can extend this journal. Dirty means current ink differs from
the blank persisted baseline; undoing/erasing all ink returns to that baseline.

`canvas.rs` is the common input/rendering controller, called with a surface and
f64 `Transform`. It consumes ordered egui pointer samples once per surface/frame,
keeps an active gesture separate from committed strokes, captures pen style at
pointer-down, and finalizes on release, lost focus, view changes or tab changes.
A4 excursions end at the edge and do not bridge margins. Both layouts use the same
controller, smoothing, eraser tests and rounded renderer. Logical coordinates are
independent of DPI; only local screen positions become f32. Quadratic midpoint
interpolation stays within the samples' convex hull; raw samples remain editable.
Logical paths and anti-aliased meshes cache completed strokes by view transform/DPI;
eraser hit tests use the same smoothed path plus stroke radius. Meshes for erased
or abandoned strokes are released; history retains semantic objects for undo.

`editor.rs` supplies page-local transforms/scrolling or Whiteboard camera
transforms/panning. It never changes document kind. `app.rs` guards destructive
navigation: dirty-tab close/workspace change/native quit require confirmation;
external refresh/open preserves dirty tabs even when the source disappears or
changes. `index.html` uses the Rust-maintained dirty flag for browser beforeunload.
Browser/OS forced termination can bypass warnings; no crash recovery exists yet.

M1 deliberately leaves `DocumentData`'s blank schema and `pdf.rs` unchanged. PDF
export of dirty tabs is blocked rather than exporting stale blank bytes. M2 must
version a complete document snapshot, write conventional visible ink plus matching
embedded edit data, and implement safe writes/round trips and a saved revision.

## M1.1 eraser performance

`eraser_index.rs` maintains a balanced bounds hierarchy over immutable strokes,
keyed by the drawing's committed revision. The first eraser sample after an edit
rebuilds it; subsequent samples query swept document-space bounds without query
allocations. Raw-point bounds conservatively contain the smoothed path and include
ink half-width. Only candidates reach cached smoothed-path intersection tests;
already-erased IDs are skipped. Segment clipping against expanded bounds keeps
long diagonal sweeps from selecting their entire bounding rectangle. Segment bounds reject distant geometry before
distance calculations. The screen-space eraser radius still divides by zoom.

Movement changes only the gesture's ID set; the document/history changes once on
release. Erase/redo compacts the stroke vector once and undo merges backwards once,
preserving identity and stacking order with shared stroke references. Empty erase
gestures do not scan history or prune caches. Existing completed-stroke meshes stay
cached during erasing; pointer movement does not invalidate their transform/DPI.
The hierarchy is transient, shared by both layouts and targets; no PDF/schema or
partial-erasing behavior changes. Future partial erasing should reuse this broad
phase and invalidate the committed revision when replacing stroke geometry.

## M1.1 application preferences (#6)

`settings.rs` owns versioned, validated `Preferences` and `DrawingPreferences`,
separate from `DocumentData`. `ui/settings.rs` owns modal presentation/navigation.
`ui/drawing_controls.rs` shares the existing Ink controls with Drawing defaults.
The app owns all application preferences and initializes each opened editor's
pen style/eraser radius from defaults. Existing tabs keep their tool selections;
input navigation preferences apply on every editor frame. Document/PDF schemas
and the current rendering interpolation remain unchanged.

Enable eframe's built-in persistence feature. Startup reads JSON from the
`XiToc/preferences` key in `CreationContext::storage`. Desktop eframe stores it
inside its app.ron in the OS application-data directory; web uses same-origin
localStorage. Changes debounce for 500 ms or flush on dialog close; eframe's
save hook also saves on normal shutdown/periodic autosave. Generic egui memory
persistence is disabled. No workspace paths, document contents or ink are stored.
The browser read-back check detects blocked/quota-failed writes; native disk
failures are reported by eframe's logging (its Storage trait has no write Result).
Forced termination may interrupt a pending write. Clearing browser site data
removes preferences. Unknown/malformed versions use defaults with a warning and
remain untouched until the user explicitly changes a preference.

Smoothing strength is implemented in #12 below. Future pressure options and
eraser modes belong in typed preference groups with validation/defaults and an explicit schema migration.
Add behavior and effective controls together; do not add unused pressure
fields now. Smoothing changes must share renderer/eraser geometry and invalidate
geometry caches; application defaults remain independent from raw document ink.

## M1.1 configurable smoothing (#12)

`Stroke` retains raw `InkPoint` samples, pressure, identity, surface and style.
A serde-defaulted `Smoothing` field records `Legacy` (the original M1 midpoint
path) or `AdaptiveV1 { strength }` (0–100). Missing stroke metadata selects Legacy,
so old semantic stroke JSON does not change appearance. New controller strokes
capture AdaptiveV1 at pointer-down. `Stroke::path()` deterministically reconstructs
the render/hit-test polyline from raw input and this versioned metadata. Enable
serde_json's float_roundtrip feature so arbitrary f64 input coordinates survive
semantic JSON round trips exactly. This does not enable PDF ink persistence.

`smoothing.rs` uses a local convex three-point filter followed by trimmed quadratic
Bézier joins. Strength increases both filtering and curve trimming; 0 returns the
original polyline exactly. Turn-angle adaptation fades both operations to zero
at 75 degrees, retaining intentional corners/reversals and small sharp hooks.
First/last points always remain exact. Calculations use relative f64 document
coordinates, independent of zoom, DPI and camera. Convex combinations keep the
path within raw input bounds, preserving A4 clipping and the eraser bounds index.
Pressure samples remain untouched for future pressure interpolation/input work.

The approach draws on quadratic corner-cutting rather than a temporal low-pass
filter. Background: [Chaikin/quadratic spline notes](https://www.cs.unc.edu/~dm/UNC/COMP258/LECTURES/Chaikins-Algorithm.pdf).
We also evaluated the [1€ filter](https://gery.casiez.net/publications/CHI2012-casiez.pdf),
which trades jitter against temporal lag; spatial local processing fits the
current ordered position-only input without inventing per-sample timestamps.
Our angle adaptation and prefilter are application choices, not a claim to
implement the unmodified Chaikin subdivision algorithm.

`Preview` retains per-vertex path offsets. Appending a sample revises at most the
previous three vertices plus new input; unchanged input does no geometry work.
The current pointer point is always visible while only the trailing geometry
settles as samples arrive. Release uses exactly the same path, with no new fitting
pass. Paths/meshes are cached for committed strokes. Mesh chunks cover 128 path
segments with shared endpoint caps drawn once; only changed trailing chunks are
retessellated during drawing. Stable chunks reuse Arc meshes; the completed cache
is retained at release. View/DPI changes invalidate mesh coordinates, not stroke
geometry. The eraser pipeline/index/radius/history semantics are unchanged and
consume the same per-stroke path as rendering.

Preferences schema 2 adds `drawing.smoothing_strength` (50 by default), migrates
schema 1 with the new default while preserving other preferences, and keeps the
existing eframe storage mechanisms. It applies to every open tab's next stroke;
changing it never invalidates existing geometry. M2 must version the PDF payload
separately and preserve raw points plus processing metadata (or exact generated
vectors) alongside matching visible PDF graphics. Unknown future smoothing
algorithms must not be silently reinterpreted. No note sidecars are introduced.
