# XiToc architecture decisions

Audit baseline: main `8fc0f94`, 2026-11-02. These records describe decisions visible
in code and repository contracts. They are not reconstructed meeting minutes.
“Documented” means the rationale is present in repository guidance or design docs;
“inference” identifies a consequence supported by the implementation. Where the
original selection rationale is unavailable, it is explicitly unknown.

See [CODEBASE_GUIDE.md](CODEBASE_GUIDE.md) for implementation flow and
[DEVELOPMENT_LOG.md](DEVELOPMENT_LOG.md) for commits/PRs. Source links below are
relative to the repository, not an external library API reference.

## ADR-001 — Rust with pinned egui/eframe and one binary crate

**Decision/status:** Implemented. One edition-2024 binary crate with egui and
eframe pinned to 0.37.0; Rust minimum 1.95. Native and WASM entry points share
modules. The app uses `App::ui` and Glow. No `lib.rs`, workspace crates or separate
web frontend exist. Evidence: [Cargo.toml](../Cargo.toml), [main.rs](../src/main.rs),
[app.rs](../src/app.rs), [AGENTS.md](../AGENTS.md).

**Rationale:** Rust, egui/eframe and native/web parity are explicit product
constraints. ARCHITECTURE's “Start simple” guidance supports a single crate before
splitting. The original comparative rationale for egui versus other toolkits,
Glow versus alternatives, and the exact release selection is unknown.

**Benefits:** One editor implementation and pinned UI API; Cargo.lock supports
reproducible dependency resolution. **Tradeoffs:** Platform capability gaps and
immediate-mode UI/input details remain application concerns; module boundaries
are conventions rather than enforced crate dependency barriers.

**Future consequences:** Keep shared modules target-compatible. A crate split
should follow actual isolation needs, not assume the old proposed directory tree
already exists. UI upgrades must consciously verify native and WASM APIs together.

## ADR-002 — Shared editor with explicit platform adapters

**Decision/status:** Implemented. `main` selects `run_native` or `WebRunner` via
cfg; storage selects capability-relative native IO or browser dialog import/export.
The same `XiTocApp`, `Editor`, `Canvas`, geometry and journal run on both targets.
Evidence: [main.rs](../src/main.rs), [storage/mod.rs](../src/storage/mod.rs),
[storage/web.rs](../src/storage/web.rs), [web_pressure.rs](../src/web_pressure.rs).

**Rationale:** Documented platform policy requires one Rust editor, local-first
operation and no backend. Websites cannot silently inspect a user's disk.

**Benefits:** Geometry and behavior fixes share tests across layouts/platforms.
**Tradeoffs:** Web import/export is not filesystem parity; native dialogs/scanning
and web completion events have different lifecycles. Some adapter code uses egui
context for repaint, so not every non-widget module is platform-neutral.

**Future consequences:** Browser directory handles need permission/capability
semantics and fallback; they cannot be documented as an existing feature. Future
platform work must preserve shared semantic types and truthful completion status.

## ADR-003 — Immutable document kind, identity separate from location

**Decision/status:** Implemented. `DocumentData` owns a UUID and tagged `Layout`;
`kind()` derives Page/Whiteboard. `Document` adds `Location` and read-only state.
Blank creation fixes the kind. Evidence: [document.rs](../src/document.rs),
[app.rs](../src/app.rs), [PRODUCT.md](PRODUCT.md).

**Rationale:** Documented requirement: type is chosen at creation, not an editor
mode, and moving a real PDF must not change its semantic identity.

**Benefits:** No competing type/layout state; refresh can recognize moved native
notes by UUID. **Tradeoffs:** Schema 1 is limited to one A4 page; ordinary external
PDFs get temporary classification UUIDs, not stable native identities. Tabs are
initially deduplicated by location; refresh handles only unique UUID matches.

**Future consequences:** Adding pages requires a deliberate schema/ownership
contract. Stable PageIds must replace index-based ownership before page reordering.
Ambiguous duplicate native UUIDs need explicit handling rather than guessed paths.

## ADR-004 — Real native folders, virtual Explorer categories

**Decision/status:** Implemented. Explorer projects a single `Folder` tree into
Pages/Whiteboards; it does not create category directories or duplicate files.
`Workspace` uses a selected-root `cap_std::fs::Dir`, bounded synchronous scans,
relative-path validation and symlink rejection. Evidence:
[storage/native.rs](../src/storage/native.rs), [ui/explorer.rs](../src/ui/explorer.rs).

**Rationale:** AGENTS/PRODUCT require real user files and no scanning outside the
selected workspace. Capability-relative IO supports that boundary even during
path replacement races; storage tests exercise failure/escape paths.

**Benefits:** Familiar files/folders and bounded traversal. **Tradeoffs:** Up to
4096 entries are scanned and PDFs decoded synchronously; large or slow workspaces
can pause the UI. UTF-8 filenames and hard-link publication restrict compatibility.

**Future consequences:** An asynchronous scanner would need result generations,
cancellation and dirty-tab reconciliation. Rename/move/delete UI must preserve
workspace-relative identity and the existing discard/read-only policies.

## ADR-005 — Single PDF with two synchronized representations

**Decision/status:** Product contract with a partial implementation. Blank schema-1
PDF creation/decoding is implemented; editable ink persistence is not. Eventual
output must contain conventional visible graphics and matching embedded edit data
named `XiToc/document.json`, with all required resources in the same file.
Evidence: [pdf.rs](../src/pdf.rs), [PDF_FORMAT.md](PDF_FORMAT.md), AGENTS.

**Rationale:** Explicit requirement: a native note must be independently viewable
as one PDF and re-editable without proprietary adjacent sidecars.

**Benefits:** One portable canonical file; ordinary viewers can display future
visible output without understanding private edit data. **Tradeoffs:** Every
supported object needs both visible PDF output and semantic decoding, with
version/resource validation and safe updates. External editors may remove or
invalidate the private attachment.

**Future consequences:** M2 must introduce a complete versioned snapshot, blank
schema migration, saved-revision tracking, matching normal-viewer/semantic round
trips and failure-safe update writes. Current serde-ready strokes/clipboard do
not satisfy this gate. Do not add ink fields to the blank codec without both
representations and tests. No PDF/A conformance is claimed.

## ADR-006 — Preserve unknown PDFs and unsaved ink rather than imply a save

**Decision/status:** Implemented safeguards. External PDF content has no preview
renderer and is read-only. Unknown/malformed edit payloads error. Dirty tabs block
stale-original-byte export; Ctrl/Cmd+S reports missing M2 persistence. Close,
workspace replacement and native quit ask before discarding; browser beforeunload
uses the app's dirty flag. New native blank files use synced no-clobber publication.
Evidence: [app.rs](../src/app.rs), [pdf.rs](../src/pdf.rs),
[storage/native.rs](../src/storage/native.rs), [index.html](../index.html).

**Rationale:** Explicit data-preservation requirements and a deliberately staged
blank-only codec. Browser dialog completion cannot establish a durable disk save.

**Benefits:** Avoids rewriting unfamiliar graphics or presenting blank original
bytes as current ink. **Tradeoffs:** Current handwriting can be lost on forced
exit; prompts are not recovery. No general existing-file atomic update exists.

**Future consequences:** Recovery, autosave and saved revision need a real content
lifecycle. Creation failure after hard-link publication (for example later
unlink/directory-sync failure) can leave the new destination present despite an
error result; update/create recovery semantics should distinguish publication
from acknowledgement. This is a code-ordering observation, not a reproduced bug.

## ADR-007 — Editable local vector geometry with separate placement and view

**Decision/status:** Implemented. `Stroke` keeps raw f64 points, optional pressure,
captured style and smoothing tag. `Placement` maps immutable local content through
anchored uniform scale/translation/rotation; `Transform` independently maps world
to view. Partial survivors are explicit `PolylineV1` vectors in local space.
Evidence: [drawing.rs](../src/drawing.rs), [object.rs](../src/object.rs),
[partial_erase.rs](../src/partial_erase.rs).

**Rationale:** Documented need for editable ink, DPI/zoom independence, precision
far from origin and whole/partial erasing without flattening. Reanchoring and local
f64 arithmetic support precision; screen conversion occurs at rendering boundaries.

**Benefits:** Transforms preserve input/style; future PDF output can use vector
semantics. **Tradeoffs:** Derived paths and meshes must agree with erasing/hits;
raw and derived geometry consume memory. Uniform scale avoids a more complex
stroke-width rule, but nonuniform stretch is unavailable.

**Future consequences:** Rotation now has a group handle/menu; Arrange remains
an internal API. New object
geometry must define local bounds, inverse transforms, width/resize semantics and
page ownership. Page clipping and pressure envelopes must agree across rendering,
selection, erasing and future PDF graphics.

## ADR-008 — Local incremental smoothing and shared pressure geometry

**Decision/status:** Implemented. Adaptive smoothing uses spatial local filtering
and quadratic joins, fades near sharp turns, preserves endpoints and only updates
a bounded trailing region. Pressure interpolation follows position weights.
Monoline/Highlighter ignore force thickness; Pressure Pen captures response.
Evidence: [smoothing.rs](../src/smoothing.rs), [pen.rs](../src/pen.rs),
[PRESSURE_INPUT.md](PRESSURE_INPUT.md), ARCHITECTURE's smoothing section.

**Rationale:** Documented comparison favored local position-only processing over
inventing timestamps for a temporal filter. Versioned tags preserve old appearance;
synthetic pressure tests are explicitly separate from hardware verification.

**Benefits:** Predictable bounded incremental work, stable release geometry,
consistent pressure rendering and clipping. **Tradeoffs:** It is an application
algorithm, not unrestricted brush simulation; force delivery depends on backend,
OS and device. Linux/macOS native currently use constant-width fallback.

**Future consequences:** New algorithms need explicit tags, deterministic geometry
and legacy handling. Hardware certification and native tablet adapters remain
separate work; compile success cannot establish pressure quality or latency.

## ADR-009 — Cached geometry and spatial broad phases

**Decision/status:** Implemented. `Canvas::Cached` holds paths/pressures and
view/DPI-keyed Arc mesh chunks. `Preview` invalidates trailing chunks; the object
hierarchy keys off committed drawing revision, and partial paths use blocks of
16 segments. Evidence: [canvas.rs](../src/canvas.rs),
[eraser_index.rs](../src/eraser_index.rs), [partial_erase.rs](../src/partial_erase.rs).

**Rationale:** Documented eraser-lag issue #14 and smoothing performance goals;
benchmarks compare indexed work against the original scan.

**Benefits:** Idle frames reuse meshes and distant eraser samples avoid full-path
scans. **Tradeoffs:** First-query index setup and changed-object geometry still cost
work; genuinely dense overlap cannot be pruned away. Cache invalidation must follow
placement/view/revision changes. Large documents/history remain memory-intensive.

**Future consequences:** New object types must extend indexing/render dispatch
without rebuilding every object on each sample. Benchmarks are CPU regressions,
not cross-device display-latency guarantees; tune only with recorded workloads.

## ADR-010 — Per-editor reversible delta journal

**Decision/status:** Implemented. `Drawing` has `Command::{Insert, Erase, Replace}`,
cursor and revision. Commands retain affected Arc strokes/indices, with one command
per gesture/batch; new edits truncate redo. Selection is transient, not its own
canonical undo stream. Evidence: [drawing.rs](../src/drawing.rs),
[canvas.rs](../src/canvas.rs), [canvas_selection.rs](../src/canvas_selection.rs).

**Rationale:** ARCHITECTURE explicitly calls for command-based history rather than
whole-document clones per pointer event. Tests require exact IDs/order/style on
undo/redo and isolated document histories.

**Benefits:** Exact reversible edits with shared unaffected objects and efficient
bulk erase/restore. **Tradeoffs:** No history size limit, persistence or crash
recovery; transformed targets clone affected stroke payloads on commit. A future
saved baseline cannot be modeled by the present “any ink means dirty” predicate.

**Future consequences:** New semantic object commands must restore relations,
ownership and order atomically. Add saved revision/recovery policy before M2
save/autosave; define memory policy if long sessions become a problem.

## ADR-011 — Small object protocol before implementing future content types

**Decision/status:** Implemented foundation. `CanvasObject`, `ObjectType` and tagged
`ObjectSnapshot` describe the common interface; only Stroke exists. The canonical
store/journal remain stroke-specific. ID lookup/ordered traversal, uniform
transforms, rotation, z-order and normalized anchors are exposed internally.
Evidence: [object.rs](../src/object.rs), [drawing.rs](../src/drawing.rs),
[CREATIVE_TOOLS.md](CREATIVE_TOOLS.md), PRs #28/#29.

**Rationale:** Documented scope favors a simple extensible architecture and avoids
placeholder future tools or a large rewrite. Connector identity/geometry must be
available before connectors themselves.

**Benefits:** Existing ink/caches remain intact; future attachments can resolve
IDs and local anchors after transforms. **Tradeoffs:** This is not yet a fully
heterogeneous store. `object(id)` is linear; rendering/selection still dispatch
through strokes. Normalized anchors do not implement routing or relation updates.

**Future consequences:** When a second type ships, extend the same store/journal
and rendering/hit dispatch. Do not create a separate per-type order/history.
Connector copy/delete needs remapping/detach policy; page-index migration and
versioned PDF resource contracts remain prerequisites for their related features.

## ADR-012 — Transient scoped selection and bounded clipboard interchange

**Decision/status:** Implemented. Selection IDs/derived bounds and gestures stay in
the editor. Picking chooses topmost envelope; lasso requires full strict envelope
containment. Move/resize previews reuse caches and commit on release.
The Selection/Lasso follow-up makes canvas Escape cancel a live gesture and clear
membership together, as explicitly requested; focus-loss cancellation retains
membership. This changes transient interaction only, without adding history. Clipboard
version 1 shares snapshots with limits and a session fallback. Evidence:
[selection.rs](../src/selection.rs), [canvas_selection.rs](../src/canvas_selection.rs),
[object_clipboard.rs](../src/object_clipboard.rs), [UI_DESIGN.md](UI_DESIGN.md).

**Rationale:** M1.4 explicitly settles page-local scope, full-envelope lasso,
non-destructive transforms, bounded payloads and native/web clipboard restrictions.

**Benefits:** Canonical content excludes handles/hover; cancellation is cheap;
copy/paste has explicit fresh identities and no arbitrary-text object creation.
**Tradeoffs:** Conservative rounded highlighter envelopes differ from exact flat
nib pixels. Selection is page-local and scale is proportional; clipboard caps
limit large transfers. Session fallback is not an OS clipboard guarantee.

**Future consequences:** New content variants need meaningful hit/lasso policies,
copy/reference validation and focus/shortcut behavior. Never serialize derived
selection state as saved object content.

## ADR-013 — Local application preferences independent of notes

**Decision/status:** Implemented schema-5 preferences store theme, Explorer,
preset library, eraser defaults/mode and middle-button navigation. Framework
storage is keyed `XiToc/preferences`; schema 1–4 migrations are explicit. Note
contents/workspace handles are excluded; generic egui memory persistence is off.
Evidence: [settings.rs](../src/settings.rs), [pen.rs](../src/pen.rs), [app.rs](../src/app.rs).

**Rationale:** Documented application-settings milestone separates device defaults
from portable note data and preserves unreadable future preferences until a user
explicitly changes something. Original choice of eframe storage over alternatives
has no recorded comparative evaluation.

**Benefits:** Existing ink captures complete style rather than depending on preset
identity at reopen. **Tradeoffs:** Clearing browser site data removes preferences;
native Storage has no durable-write Result. Preset state, schema-1 document data,
and clipboard versioning are separate contracts.

**Future consequences:** Do not place future note data in application preferences.
M2 must store self-contained captured object styles/resources inside the PDF.

## Audit risks and documentation follow-ups

These are observations/prerequisites, not authorization to refactor:

- **Ink durability:** session ink has neither save nor recovery. Preserve current
  guards until both PDF representations and failure-safe writes exist.
- **Synchronous work:** workspace scan/load/PDF parsing runs on the UI action path;
  existing count/size limits bound inputs but do not guarantee responsiveness.
- **Growth:** command history, raw ink and caches lack aggregate memory budgets.
- **Evolution:** semantic modules are not isolated crates; object store/journal
  still know Stroke, and page ownership is index-based. Future types/pages must
  resolve these seams without parallel canonical state.
- **Trust boundary:** serde decoding alone does not validate a semantic object.
  Drawing/clipboard validate accepted mutations, but M2 needs its own complete
  bounded decoder/migration/resource validation before enabling saved ink.
- **Platform verification:** CI compiles/checks platforms; no committed automated
  real-browser or physical-stylus harness certifies input delivery/latency. Existing
  screenshots/manual records are milestone evidence, not current universal proof.

Future documentation should accompany real implementation decisions: M2 snapshot
schema/PDF graphics/resources and recovery protocol; stable PageId/multipage
migration; second-object store/journal dispatch; connector attachment deletion and
copy semantics; text/font/image resource limits; formula renderer evaluation;
browser directory capabilities; measured asynchronous IO and hardware support.
The original toolkit/renderer selection comparison remains unknown rather than
being backfilled with invented historical reasons.

## ADR-014 — Uniform handwriting resize and cached rotation previews

**Decision/status:** M1.4 manipulation implements a rotation handle with logical
center pivot, angle unwrapping and 15-degree Shift snapping. Existing movement,
uniform corner resize, duplication and journal commands remain shared. Cut stores
a validated bounded clipboard bundle before deletion; arrow nudges reuse page-
constrained translation. Evidence: `canvas_selection.rs`, `selection.rs`,
`canvas.rs`, `manipulation_tests.rs` and the M1.4 manipulation interaction contract.

**Rationale:** The requested manipulation task prioritizes editable raw ink,
pressure appearance, one command per drag and responsiveness. Proportional resize
preserves the circular pressure envelope; an anisotropic ink stretch would need
a deliberately different nib/render/hit/erase contract. Future box-like types can
opt into nonuniform resize via semantic geometry rather than silently distorting
handwriting. The original toolkit choice is not reconsidered by this decision.

**Benefits:** Raw samples/style/IDs remain unchanged; all edits use the same journal.
Move/resize keep their layer-transform fast path. Rotation reuses tessellated mesh
vertices, updates once per changed frame and shares idle preview Arcs.
**Tradeoffs:** egui's layer transform cannot rotate; changed rotation frames must
visit visible selected vertices and may copy renderer-held mesh Arcs. Page rotation
uses conservative centerline bounds and can stop before a more exact contour would.
Handwriting does not support independent X/Y stretch; a 1-point longest-extent
minimum prevents further collapse. System clipboard completion is not acknowledged
by egui, so Cut relies on the session copy as well as the standard clipboard request.

**Future consequences:** New object types must supply geometry, bounds, resize
policy, rendering/hits and relation-aware journal adapters in the existing store.
They must not create a second selection/history engine. Box/text resizing can
change intrinsic geometry under its own rules; any general affine ink transform
needs an explicit elliptical-envelope contract. Connector anchors already resolve
through placement but routing/remapping/deletion policies remain separate work.

**Browser integration evidence:** Firefox page Select All included the HTML canvas;
starting an object drag could initiate native browser DND and lose move/release
input. A canvas-only A/D key-default/dragstart guard in `index.html` prevents that
without suppressing clipboard events or eframe's separate text-agent shortcuts.
This addition follows a reproduced runtime failure, rather than a toolkit rewrite.

## 2026-11-04 — Unified Selection modes (#34)

Keep the existing Select/Lasso dispatch values as transient Rectangle/Lasso modes
behind one top-toolbar button. Both modes share picking, handles and transform
commands; the dropdown mode is recalled when returning from another drawing tool.
Rectangle geometry uses the same transformed path/pressure cache and object bounds
index as lasso/erasers. Its exact predicate covers the rounded tapered envelope,
using horizontal endpoint direction for containment versus crossing. Zero horizontal
displacement is containment; changing direction recomputes candidates immediately.

This avoids a second object store/history engine and preserves immutable raw ink.
Candidate IDs reuse vector capacity during pointer sampling; preview outlines use
cached bounds instead of allocating copies of every path each frame. Lasso keeps
its existing indexed strict containment. The first content adapter remains Stroke;
future object types need exact geometry dispatch in the same selection pipeline.
Modifiers are captured at pointer-down, Alt before Shift; handles take priority
except Alt bypasses them. Shift-click addition is idempotent. Region previews do
not change canonical content or add history entries. No persistence schema changes.
