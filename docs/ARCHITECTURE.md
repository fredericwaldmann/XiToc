# Architecture

For the current source map and data flow, see [CODEBASE_GUIDE.md](CODEBASE_GUIDE.md).
[ARCHITECTURE_DECISIONS.md](ARCHITECTURE_DECISIONS.md) records decision evidence;
[DEVELOPMENT_LOG.md](DEVELOPMENT_LOG.md) distinguishes historical milestones.
Sections below retain milestone contracts; later sections supersede earlier scope.

## Start simple
The implementation is one binary crate with flat source modules (no `lib.rs` or
`domain/` directory). The following tree is an original proposed organization,
not the current filesystem layout; consult CODEBASE_GUIDE for actual modules:

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
- pdf-writer generates blank PDF pages/embedded file objects; lopdf is the implemented reader (default features disabled, wasm_js enabled). Visible handwriting serialization remains M2.
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
must be versioned deliberately. No objects are flattened or serialized into PDFs yet.

Stroke and the shared object protocol are implemented in memory (see M1/M1.4 below).
Text, Shape, Image and other semantic content variants remain future work.
Keep stable IDs, logical f64 coordinates, command-based undo and one rendering
pipeline. Camera changes in M0.5 are session-only; future trusted-model saves
must serialize the current editor snapshot together with visible PDF graphics.

## Storage lifecycle: implemented and planned
Currently, opening a native workspace scans real folders/PDFs. Creation publishes
blank PDFs; loading retains original bytes and starts an in-memory Drawing.
Editing marks ink dirty but does not save it. Web uses explicit import/export
fallback, with no directory handles. Dirty export is blocked.

M2 must serialize editable objects plus matching visible graphics in the same PDF
and implement safe updates; future browser directory support needs explicit
permission. Never claim permanent save merely from browser dialog completion.

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
Pressure is now optional (see #15 below); unavailable input uses constant width. Stroke types derive serde
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

Smoothing strength, eraser modes and pressure are implemented in #12/#13/#15
below, with typed preferences, validation/defaults and explicit schema migrations.
Add behavior and effective controls together. New smoothing algorithms must share
renderer/eraser geometry. Changing the preset smoothing setting affects future
strokes only; it must not invalidate or change existing captured stroke geometry.

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
Raw pressure samples remain untouched; #15 interpolates derived force using the
same local smoothing weights.

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

## M1.1 vector partial erasing (#13)

`EraserMode` selects Stroke (default) or Partial. The toolbar passes the shared
application preference into the current canvas; changing it finalizes the old
gesture. Each eraser gesture captures its mode. Preferences schema 3 adds
`drawing.eraser_mode`, migrates schema 1/2 to Stroke and retains smoothing/style
settings. Existing radius defaults and per-tab Ink controls are reused.

`partial_erase.rs` analytically clips each candidate rendered centerline segment
against the swept eraser capsule: two disks and their connecting oriented strip.
The hit radius is screen radius / view scale + ink width / 2. Calculations use
relative f64 document coordinates. The same `Stroke::path()`/cached geometry is
used for rendering, whole-stroke detection and partial clipping. Scalar intervals
(segment index + local parameter) represent erased portions. Adjacent/overlapping
cuts merge; zero-length tangencies do not split vectors. One continuous retained
range yields one fragment, not one object per input point or pointer event.

The document BVH from #14 stays unchanged for whole-stroke queries. Partial
candidates lazily receive a temporary immutable BVH over blocks of 16 rendered
segments, reusing the existing bounds hierarchy/segment-clipping broad phase.
Queries visit local blocks; merged cuts avoid redoing already erased geometry.
No document mutation/index rebuild or fragment UUID creation occurs on movement.
Preview caches only changed 128-segment mesh chunks and reuses untouched original
meshes; even empty erased chunks are cached. Temporary indexes/preview meshes are
released with the gesture. Extremely dense overlapping content still requires
processing its genuine local candidates. Initial path-block indexing is linear
in rendered points plus hierarchy setup; commits and first fragment rendering
scale with affected geometry, not just the pointer sample count.

On release, `Drawing::replace_partial` validates all replacements and commits one
Replace command. It stores only original/fragment Arc objects with their old/new
indices. Apply/undo use linear compaction/backwards merging and preserve stacking
order; invalid replacements leave data/history untouched. Each surviving piece
gets a new UUID at commit, retained across redo. Untouched IDs remain unchanged;
undo restores the original IDs and raw smoothing data. New IDs also ensure that
old/new geometry cannot collide in render caches keyed by stroke identity.

Survivors use `Smoothing::PolylineV1`: their points are the exact retained rendered
polyline vertices plus analytic cut endpoints. They are not smoothed a second
time. Color, width, document/surface identity and vector editability are retained.
Missing/uniform force uses a constant pressure map. #15 now interpolates adaptive
pressure with the spatial smoothing weights; legacy ink retains the original
normalized-arc-length mapping. Cut endpoints interpolate matching path pressure. M2 must version/serialize these explicit vectors and
processing tags in the same PDF as their visible graphics. The blank schema,
canonical PDF storage and unsaved-ink guards remain unchanged.

## M1.1 pressure input and geometry (#15)

`InkPoint.pressure: Option<f32>` distinguishes absent force from normalized
measurements, including a real zero. Finite device values clamp to [0,1]; invalid
values become None. Old numeric sample JSON still decodes; missing/null values
mean unavailable. Serde-defaulted `PressureStyle { enabled, strength, minimum }`
captures the response at pointer-down. Missing style defaults to disabled, keeping
old constant-width strokes unchanged. Preferences schema 4 migrates versions 1–3.
No document/PDF schema changes or ink persistence are introduced.

Width is nominal width times `1 - strength * (1 - minimum) * (1 - pressure)`
(percent settings converted to fractions). Missing force always gives nominal
width. Raw points stay editable. Adaptive smoothing applies identical convex
weights to positions and pressure, updating only the bounded trailing geometry;
preview and release agree. Partial fragments contain explicit interpolated force
and captured style, preserving width through subsequent cuts and history.

Cached paths retain corresponding force samples. Variable rendering uses the
external tangents of adjacent round disks and round joins/caps, in document
coordinates before the view transform. Constant-width input retains the existing
line renderer. Mesh chunk caching, document bounds hierarchy and path-block
partial-erasing index remain in use. Maximum nominal width bounds are conservative;
pressure narrow-phase clipping solves piecewise quadratics with local radii, no
per-segment heap allocation. Completed geometry and idle previews remain cached.

Native egui-winit 0.37 forwards winit normalized/calibrated touch force before its
emulated primary-pointer event. The controller pairs the first contact's force
with the existing pointer stream and ignores secondary contacts for ink force.
No force is inferred from mouse movement, zoom or DPI. See PRESSURE_INPUT.md for
backend investigation and limitations.

`web_pressure.rs` is a small wasm-bindgen inline-JS boundary using existing
dependencies. Primary pen PointerEvents are captured, queued in order, and injected
through eframe's raw-input hook as force plus primary-pointer events. Pointer
capture retains off-canvas releases; cancellation/focus loss finishes safely.
A page-lifetime Rust callback requests repaint when pen samples arrive.
Preventing default on pen-down suppresses duplicate compatibility mouse input;
ordinary mouse/finger events remain on eframe's path.
Browser CSS positions divide by egui's UI zoom, matching eframe's coordinates.

## M1.3 top toolbar, pen types and preset library

This supersedes the earlier bottom-palette UI slice. The 40-point toolbar remains
directly below tabs. `pen.rs` contains UI-independent PenType, PenStyle, PenPreset
and Presets with stable UUIDs, bounded validation and library lifecycle operations.
Only Monoline, Pressure and Highlighter are implemented. `Canvas::apply_pen`
projects the selected style into transient controller fields; strokes capture
pen type, RGBA opacity, width, smoothing and effective pressure at pointer-down.
Partial fragments retain type/style and exact undo/redo snapshots.

Preferences schema 5 replaces independent live ink defaults with a single preset
library (selected ID and ordered entries), retaining eraser defaults/mode. Versions
1–4 migrate through a private decoder-only legacy structure; original style is
preserved in a stable-ID Migrated pen. No parallel legacy settings state survives.
Settings and top-toolbar popovers share the same preset widgets/state. Registry
validation rejects duplicates, missing defaults/selection, unsupported pen types,
out-of-range/nonfinite values and oversized names/libraries. Local preferences
are bounded to 64 KiB; note content is never included. Unsupported preferences are
preserved by the existing app guard until an explicit user change.

Opaque ink keeps the existing rounded renderer. Translucent strokes use joined,
anti-aliased vector ribbon strips to avoid overpainting every segment/cap. Adjacent
cached chunks include neighbor samples for matching seam normals, preserving the
incremental mesh architecture. Width response comes from the same PressureStyle
and hit-testing pipeline; erasers conservatively use existing rounded envelopes
around flat-nib endpoints. Genuine self-crossings and distinct strokes composite
normally. Cached meshes are reused on idle frames. No offscreen full-canvas render
or per-frame stroke reconstruction is introduced.

The blank PDF codec/schema and save lifecycle are unchanged; ink remains in
memory until M2. M2 must serialize semantic pen types and style snapshots together
with matching visible transparency/pressure geometry. No preset lookup may be
required to reopen ink. See `CREATIVE_TOOLS.md` for remaining object dependencies.

## M1.4 selection contract (settled before implementation)

`object.rs` defines the shared CanvasObject protocol (stable ID, document/surface
ownership, local bounds, placement and world bounds), ObjectBounds and a validated
f64 uniform placement transform. Stroke implements this protocol without adding
unimplemented content variants. The existing ordered stroke collection is the
initial object-store adapter: its sequence remains the single z-order authority.
Future Shape/Text/Formula/Connector/Image variants implement this same protocol
and enter the same command, selection and rendering dispatch, not a second editor.

Placements map immutable raw document-point samples into current document space;
positive uniform scale and anchored translation preserve shape/pressure/pen style.
Only proportional corner resizing is offered in M1.4. Raw samples and captured
style are never refit by selection. Committed transforms replace only affected
objects in the shared journal, preserving IDs and order; duplicate/paste insert
fresh IDs in one transaction. Selection/handles/lasso/previews are transient.
Movement preview reuses cached geometry; no per-pointer mutation of the document.
Escape in the canvas cancels a live selection gesture and clears transient IDs,
bounds and outlines in the same frame, so subsequent pointer release cannot
commit it. It creates no command and does not invalidate stroke meshes. Popup
and text-input focus retain priority over canvas shortcuts.

Scope is Surface::Page(index) or Surface::Canvas. Page selections never span
pages or silently change ownership; moving/resizing keeps source centerlines
inside the same A4 surface and paper clips ink edges as before. #11 must migrate
page indexes to stable PageIds before page reordering is introduced; schema 1 is
still blank-only. Whiteboards retain f64 precision far from the origin.

Lasso uses an explicitly closed even-odd polygon, rejects degenerate gestures,
and requires the entire rounded pressure-width stroke envelope to lie strictly
inside it. This conservative envelope includes flat highlighter end caps too;
touching/intersecting strokes are excluded. Segment/edge checks prevent concave
polygons from selecting strokes that leave and re-enter the region. Future
partial-intersection selection is a separate policy, not a silent behavior change.

Clipboard bundles are bounded, versioned, tagged object snapshots, shared within
the current app session; supported system clipboard text carries the same payload.
This is a transient interchange payload, never a native note format or sidecar.
Paste remaps object/document/surface identities and fits the target page; repeated
paste/duplicate offsets are logical document coordinates. Future linked-object
bundles must remap included IDs and detach references to uncopied targets.

On native, menu Paste requests a platform paste event, falling back to the session
bundle. On web, menu Paste uses the session bundle directly because eframe's web
backend does not implement RequestPaste; keyboard DOM paste events accept the
same validated payload. Arbitrary clipboard text does not create canvas objects.

## Unified canvas object foundation (M1.4 follow-up)

`object::CanvasObject` is the UI-independent semantic protocol: UUID identity,
object type, document/surface ownership, local geometry bounds, placement, world
bounds, point hit testing and normalized local-bounds anchors. `ObjectType` and
serde-tagged `ObjectSnapshot` contain only Stroke today; the latter is shared by
clipboard interchange rather than defining a second stroke payload. Future shape,
line/arrow, connector, text, sticky, formula and image milestones add real variants
and dispatch implementations together. No placeholder tools or payloads are added.

`Drawing::objects` exposes objects in canonical back-to-front order and `object`
resolves stable IDs (currently linear lookup; do not call it per sample in a large
selection). The stroke vector remains the sole store and order authority, retaining
existing Arc identity, caches and indexes. When a second content type ships, extend
this store and journal to the tagged content representation; do not create a
parallel collection/order. `set_z_order` journals only the affected interval and
restores exact sequence and object Arcs on undo/redo. Existing insert, erase,
partial replacement and transform transactions remain shared commands.

Placement now supports translation, positive uniform resize and rotation:
`world = position + R(rotation) * scale * (local - origin)`. Radians are measured
in logical coordinates; screen orientation follows the existing view transform.
Composition reanchors at a raw sample for far-origin precision. Rotation is finite,
scale remains bounded to 1e-4..1e4, and inverse mapping preserves local ink.
World AABBs enclose all four transformed local-bound corners. Bounds are
conservative raw-ink envelopes, while narrow-phase hits reuse the existing
pressure-aware vector test moved from the UI into `object.rs`. Cached selection
and erasing call that function directly, avoiding per-frame path reconstruction.
No rotation UI or nonuniform stretch is introduced.

Future connectors can resolve `(ObjectId, normalized local-bounds anchor)` through
`Drawing::object` and `CanvasObject::anchor`, so movement/resize/rotation derive
new endpoint positions without mutating raw geometry. Anchors range from [0,0]
to [1,1], including center [0.5,0.5]; invalid anchors return None. Missing IDs
return None, requiring a deliberate detach/delete policy in the connector task.
No relations, connector object, routing or reactive graph are implemented here.
Page ownership remains explicit Page(index); stable PageId migration is required
before page reordering (#11), as already specified in M1.4.
