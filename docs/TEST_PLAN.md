# Tests and CI release gates

## Always
- cargo fmt --all -- --check
- cargo clippy --all-targets -- -D warnings
- cargo test --all-targets
- cargo check --target wasm32-unknown-unknown
- trunk build --release
- Native cargo check/build on Linux, Windows, macOS via CI.

## Document correctness
- A4 page size and 1, 2, and multiple pages.
- Empty PDF / corrupted PDF / absent native payload / unsupported newer schema.
- Round-trip ink points, stroke IDs, modes, camera, page bounds, negative coordinates.
- Independent PDF parser can parse outputs; manually open sample in common reader and inspect appearance.
- Export/reopen 10 times; one attached current JSON payload; no hidden sidecars.
- Simulated interrupted save preserves original file (desktop); browser shows actual save state.

## Explorer and UI
- Renaming folders preserves path consistency, no traversal out of root.
- Opening/saving notes nested in real workspace.
- Resizing left sidebar, switching tabs, zoom gestures; undo/redo.
- Browser with directory API: permissions prompted and writes verified.
- Browser without API: import/download explicitly; do not report disk save.

## Milestone checklist
Each PR must show checks executed and manual observations. Only mark a milestone complete once the relevant tests pass in desktop and web build.

## M0 shell smoke check (current UI includes M0.5)
- Native: `cargo run --locked`; browser: `trunk serve`, open the localhost URL.
- Resize Explorer by dragging its divider; shrink and expand the window.
- Create/open one document of each type and switch tabs using keyboard focus.
  Do not change a document's type: the old M0 preview toggle was removed in M0.5.
- Follow the detailed M0.5 workflow below for creation, persistence and navigation.
- Inspect browser console for startup errors. A WebGL2-disabled startup should
  show an error instead of leaving the loading message indefinitely.
- CI compile checks do not substitute for native/browser visual smoke tests.

## GitHub Pages deployment follow-up (M0)
- Run `trunk serve --open`: the app loads at the local root; resizing retains a
  full-window canvas, and both preview modes still work.
- Run `trunk build --release --public-url /XiToc/`: inspect `dist/index.html`
  and confirm generated JavaScript/WASM URLs start with `/XiToc/`.
- Serve `dist/` mounted at `/XiToc/`; request the HTML, JavaScript and WASM and
  confirm successful responses. In a WebGL2 browser, confirm startup, resize
  and A4/Whiteboard switching, with no failed asset requests.
- Set Pages source to GitHub Actions. Push to `main` or dispatch the GitHub Pages
  workflow on `main`; verify build, artifact upload and deploy succeed, then
  open `https://fredericwaldmann.github.io/XiToc/` and repeat the browser checks.
- CI remains responsible for native builds; Pages deployment uses `pages.yml`
  only. Manual dispatch on another branch must not deploy.

## Desktop release pipeline (M0 distribution follow-up)
- Run `python3 -m unittest discover -s scripts -p 'test_*.py'`. These test archive
  contents, missing/empty output, CPU mismatches, unexpected paths, corrupt ZIPs,
  executable permissions, manifest/tag mismatch and Cargo binary-name discovery.
  Format-header fixtures are not substitutes for compiling or launching apps.
- Validate `.github/workflows/release.yml` with actionlint. Run the usual fmt,
  Rust tests, Clippy, native build, WASM check and Trunk build.
- Build Linux with the explicit release target and run `scripts/release.py package`;
  inspect the tar contents, bytes, CPU format and execute permissions.
- On GitHub: push a new version-matching `v*` tag containing the workflow/scripts.
  Confirm all four native jobs pass before publish starts, the release notes are
  generated, and all four correctly named archives are attached.
- Introduce a failed build in a test version: publication must be skipped. A
  mismatched tag/version must fail in prepare before native builds start.
- Interrupt an asset upload: any release created must remain a draft. Inspect
  and remove an incomplete draft before retrying. An existing published release
  must not be overwritten. Manual dispatch must check out the specified tag.
- Download/extract each real archive, launch on the matching OS/CPU, resize the
  window and switch preview modes. Check unsigned Windows/macOS warnings and
  Linux runtime requirements as documented in README.md.
- These remote failure scenarios and Windows/macOS runtime checks require GitHub
  Actions/desktop machines; local archive fixture tests do not prove them. 4f1710cb3ec1ae9b8aeea959ea04b6c79c16731c

## M0.5 document identity and creation
Automated coverage includes kind/UUID/settings classification, one-file blank PDF
round trips, independent writer/reader interoperability, ordinary PDF Page/read-only
fallback, corrupt/future payload rejection, real workspace creation/restart/reopen,
external relocation with open-tab path reconciliation, filename collision and failed-write preservation, symlink and
path traversal rejection (including a swapped directory after validation), both
headless editor layouts, and navigation without document conversion.

Manual native check:
1. Open a small real workspace with nested folders. Verify Pages/Whiteboards are
   virtual sections with the same real folder hierarchy; no category folders appear.
2. Select a nested folder; New Document → New Page Note → enter a filename. Repeat
   using a folder's context menu → New Whiteboard. There must be one `.pdf` per
   document, no sidecars, and each file appears in exactly one category.
3. Page Note scrolls through an A4 page boundary. Whiteboard has no page boundary;
   drag or space-drag pans, Ctrl/Cmd-wheel/pinch and +/− zoom; Reset view recenters.
   No editor control changes the document kind. Close/reopen each tab.
4. Restart XiToc, reselect the workspace, reopen both PDFs. IDs and types must be
   preserved. Move/rename a PDF using the OS file manager, Refresh and reopen at
   its new path; type and identity remain unchanged. Full mutation UI is M3.
5. Open the PDFs in an ordinary PDF viewer. Each displays a blank A4 page. Inspect
   the embedded `XiToc/document.json` attachment. A Whiteboard's finite PDF page is
   a fallback only. Poppler checks can be repeated with:

   ```sh
   XITOC_PDF_REVIEW_DIR=/tmp/xitoc-blank-pdfs cargo test blank_types_round_trip
   pdfinfo '/tmp/xitoc-blank-pdfs/Page Note.pdf'
   pdfinfo /tmp/xitoc-blank-pdfs/Whiteboard.pdf
   pdfdetach -list /tmp/xitoc-blank-pdfs/Whiteboard.pdf
   pdftoppm -singlefile -scale-to 500 -png /tmp/xitoc-blank-pdfs/Whiteboard.pdf /tmp/xitoc-whiteboard
   ```
6. Attempt a duplicate filename and an unwritable folder. Show the error; preserve
   existing bytes and keep the dialog open for correction. Open a corrupt PDF and
   an ordinary third-party PDF: errors/read-only state must never overwrite them.

Manual web check (`trunk serve`, WebGL2 browser):
1. No real workspace is implied. Create both types with explicit filenames and
   confirm the requested browser PDF downloads, or retry with Export PDF.
2. Close/reopen from the session tree. Reload the browser: session state resets;
   explicitly import each exported PDF, confirm original type/layout returns.
3. Import an ordinary PDF: read-only Page classification, no fake blank content.
   Export preserves its original bytes. Duplicate import names are rejected.
4. Cancel file selection/download, block downloads, and deny permissions. Never
   show a verified permanent-save status. No backend or directory API is involved.

M0.5 does not implement content editing, existing-PDF updates, saved camera changes,
auto-restoring workspace handles, background scans, filesystem mutation controls,
or browser directory handles. Those stay in their later milestones.

## M0.6 UI foundation

Automated regression coverage includes metadata-derived page/board rendering,
transient view controls preserving document data, pointer activation of tab filenames,
and application layout at
narrow/wide logical sizes with both themes and display scaling.

Manual (desktop and WebGL2 browser):
1. Open/create Page and Whiteboard PDFs using the M0.5 flows above. Capture an
   Explorer-visible screenshot, a Page screenshot and a Whiteboard screenshot.
   Check neutral paper workspace, centered A4 edge/shadow and borderless dots.
2. Open several long filenames. Switch by tab and Explorer, close active/inactive
   tabs, reopen from Explorer. Types stay fixed; closing does not delete anything.
3. Expand/collapse category and folder chevrons; select a nested creation folder;
   use New/context menus. Drag the Explorer divider, hide/show Explorer.
4. Resize from 1100×800 through 800×600 to 480×640. Check scrolling tabs, toolbar
   overflow menus, file truncation, status details and the creation dialog. At
   narrow sizes hide Explorer for more workspace. No overlapping controls.
5. Switch Light/Dark; repeat at 150% and 200% scaling. White paper remains white,
   labels readable, focus/hover/selection distinct. Use Tab/Enter/Space to reach
   tabs, close buttons, sidebar toggle, menus and zoom; inspect focus outlines.
6. Disabled tools never edit content. Whiteboard +/−, Reset and grid toggle work;
   pan/zoom still works. Reopen restores PDF metadata, not transient navigation.
7. Import an external PDF: read-only explanation, no fake blank preview. Export
   cancellation never claims a permanent save; long messages remain accessible.

Browser screenshots verify actual rendering, not native platform startup. Native
OS dialogs, high-DPI displays and Windows/macOS runtime still require matching
machines; record precisely which checks were executed in each PR.

## M1 drawing engine

Automated gates: vector stroke identity/style/pressure/serde readiness; page and
document ownership validation; transform round trips under zoom/pan/resize/DPI and
large coordinates; swept eraser segment/dot hits; add/erase command undo/redo,
stacking order and redo-branch truncation; document isolation. Actual ordered egui
pointer events exercise both layouts, taps, erasing, keyboard shortcuts and A4
margin excursions. Dirty-tab Escape, external replacement/deletion refresh and
native close cancellation protect in-memory ink. Navigation ignores pre-press
motion and layout retries; settings remain open during adjustments. Existing PDF/storage/UI tests
must continue passing; no M1 test should claim PDF ink persistence.

Manual browser/native:
1. Create a Page Note. Draw curves, fast diagonals and dots using Pen. Change Ink
   color/width. Check rounded caps/joins and continuous input. Draw across each
   paper edge and back: no ink outside the page or bridge across the margin.
2. Zoom in/out (View +/−, Ctrl/Cmd-wheel/pinch), scroll, resize and change UI scale.
   Ink stays attached to paper. Select Hand/Space-drag to scroll without drawing.
3. Erase a stroke and sweep across several; ring matches the eraser radius. Undo
   restores the full gesture and original order; redo erases again. Undo several
   pen strokes, redo, then draw after undo: the abandoned redo branch disappears.
4. Create a Whiteboard and repeat drawing, erasing and undo. Hand, Space-drag and
   middle-drag pan; zoom preserves world attachment. Switch tabs/Explorer: each
   document retains independent ink, camera, style and history.
5. Verify dirty dots/status, disabled dirty-PDF export and truthful Ctrl/Cmd+S.
   Close a dirty tab: Keep editing/Escape retains it; explicit Discard closes it.
   Reopen from Explorer: only the original blank PDF returns. Dirty native quit
   and workspace replacement also warn. Canceling a folder picker preserves work.
6. Externally replace/move/delete a source PDF, then Refresh: unsaved ink is never
   overwritten or dropped. Missing-source tabs stay open until explicit discard.
7. Browser reload/leave should request its standard warning after drawing. Prompt
   availability depends on browser/user activation; forced termination cannot be
   prevented. No crash recovery or ink persistence is claimed.
8. External PDFs stay read-only, with preview unavailable; no drawing or rewrite.
   Check narrow toolbar menus, both themes and high-DPI hardware where available.

Capture actual Page/Whiteboard drawings and the discard dialog, and record the
browser/native environments used. Physical stylus pressure, prolonged handwriting
performance, OS dialog interaction and Windows/macOS startup require real devices.

## M1.1 eraser performance (#14)

Automated: compare indexed erasing with the original smoothed-path full scan
for 1, 100 and 10,000 strokes (dots and curves, varying widths), both document
types and 0.1×/1×/8× zoom. Verify complete gesture undo/redo, original stacking
order and index invalidation after history changes. A deterministic sparse-query
test verifies only one candidate out of 10,000, surface isolation and inclusive
width tangency at coordinates near 1e12. A diagonal sweep through a 100×100
stroke grid selects the 100 diagonal candidates rather than all 10,000. Existing ordered pointer-event and
keyboard undo/redo tests still cover the real controller.

Run `cargo test --release eraser_performance -- --ignored --nocapture` for timings.
The test uses 100-point strokes, reports the original full-scan hot path versus
indexed sampling for 200 pointer samples, index construction separately, and a
long sweep plus commit. These are CPU sampling timings, not end-to-end frame
latency or universal performance thresholds. Regular CI uses correctness/work
counts rather than flaky wall-clock limits.

Manual native and WebGL2 browser:
1. Create a Page Note and a Whiteboard, draw small and dense sets of strokes.
2. Select Eraser; move through empty areas, repeatedly over already erased ink,
   and sweep quickly across several strokes. Check the ring stays responsive
   and only intersected whole strokes disappear, including dots and thick ink.
3. Repeat zoomed out/in using Ctrl/Cmd-wheel or View controls and after panning;
   check ring/hit alignment. Undo once restores the complete sweep in original
   order; redo erases it again. Draw after undo to exercise branch truncation.
4. Switch tabs and repeat; check isolation and drawing after erasing.

Ink remains memory-only until M2; these tests do not imply PDF ink persistence.

Local release-mode reference results (Linux, synthetic CPU-only workload; timings
vary with host load):

| Workload | 100 strokes | 10,000 strokes |
| --- | ---: | ---: |
| Original scan, 200 empty-space samples | 245.7 ms | 43,127.7 ms |
| Index construction | 0.115 ms | 14.607 ms |
| Indexed, 200 warm empty-space samples | 0.017 ms | 0.017 ms |
| Original half erase + undo (selection excluded) | 0.005 ms | 10.038 ms |
| Batched half erase + undo (selection included) | 0.012 ms | 8.741 ms |
| Indexed long diagonal sweep + commit, including rebuild | 0.115 ms | 40.312 ms |

The diagonal sweep covers 20,000 document points on each axis. The original
algorithm is retained as a test-only oracle so the same fixture can compare it
with the indexed implementation. These sampling measurements exclude rendering.
The hierarchy's segment clipping reduced the 10,000-stroke diagonal case from
527.6 ms in the initial rectangle-only implementation to 40.3 ms in the final
run (host load varies). Interactive native/browser smoke tests were not performed
in this run; follow the manual procedure above on actual devices.

## M1.1 Settings (#6)

Automated: open via Edit → Settings, change theme and Explorer visibility, close
with Done and Escape; serialize/save/recreate the app with every implemented
preference; apply drawing defaults to both document types without changing
existing ink or PDF metadata; test middle-button pan enabled/disabled using real
pointer events. Validate ranges/opacity, malformed and future JSON, missing-field
defaults, and preservation of unsupported settings on automatic save.

Manual native and browser:
1. Open Edit → Settings without opening a note. Visit all four sections using
   pointer and Tab/Enter. Change theme and Show Explorer; close using Done and
   Escape, reopen. Check narrow windows, both themes and 150%/200% UI scaling.
2. Set default color, width and eraser radius, then create/open a Page Note and
   Whiteboard. Draw and erase; the default style/radius should be used. Change
   defaults again: newly opened tabs inherit them, existing ink is preserved.
3. Toggle middle-button pan off/on in an existing Whiteboard; verify navigation
   changes immediately. Hand/Space-drag and Ctrl/Cmd-wheel zoom still work.
4. Close/restart native XiToc or reload/reopen the browser at the same origin.
   All preferences should survive; notes and workspace handles are not restored.
   Browser note ink remains memory-only until M2.
5. Block browser localStorage or fill its quota: check the Settings warning and
   session behavior. Clearing site data resets defaults. Native disk errors are
   logged by eframe. Inspect the OS app.ron / localStorage preference key to
   confirm that it contains no document content or workspace paths.
6. Stroke Smoothing (#12) and eraser modes (#13) are enabled; pressure controls are documented in the #15 section below.

Local validation for this change: fmt, Clippy with warnings denied, 42 tests
passed (the existing eraser benchmark is ignored), native build, WASM check and
Trunk release build. Firefox headless/WebGL2 smoke verified opening the actual
modal, all four sections, theme/Explorer live changes, Drawing width adjustment,
Input Devices, and localStorage restoration after reload. Native platform restart
and Windows/macOS runtime checks remain manual; app reconstruction/storage-hook
round trips are covered automatically.

## M1.1 stroke smoothing (#12)

Automated gates cover raw 0% geometry, dots/two-point strokes, 25/50/75/100%
strengths, monotonically reduced jitter (including improvement over the original
M1 curve at the default), exact endpoints and 90-degree corners/tiny hooks,
convex bounds, far-origin precision and zoom/pan/resize transforms. Every prefix
of a 1,000-sample stream must match the final reconstruction, rewrite only bounded
trailing geometry, and perform no work for unchanged input. A 1,200-sample actual
controller test verifies stable mesh reuse during drawing/idle/release. Both
surfaces at 0.1×/1×/8× verify capture at pointer-down, preview/release equality,
raw JSON/geometry round trip, settings changes affecting only future strokes,
and undo/redo. Eraser reference comparisons include legacy and adaptive paths.
Preferences tests cover schema-1 migration, every strength and range validation.

Repeat `cargo test --release smoothing_performance -- --ignored --nocapture` for
CPU geometry timings on 100- and 100,000-sample streams. It reports incremental
append processing and a single full reconstruction separately. Timings exclude
input delivery, GPU drawing and display latency. CI uses deterministic bounded
work/cache-identity assertions rather than wall-clock thresholds.

Manual native/WebGL2 browser:
1. Create a Page Note or Whiteboard. Draw the same shallow zigzag, loops/cursive,
   circles, corners, tiny hooks, dots and long continuous line at 0%, 25%, 50%,
   75% and 100% through Edit → Settings → Drawing → Stroke Smoothing.
2. Check that the endpoint follows the pointer, turns stay deliberate, and mouse
   release does not refit the stroke. Compare how jitter changes with strength.
3. Change strength with existing ink present: previous strokes must stay exactly
   as drawn. Repeat after zoom/pan/resize and in another tab/document type.
4. Undo/redo drawing and erasing. Eraser alignment and responsiveness must remain
   consistent with visible strokes. No partial-erasing behavior is added.
5. Reload/restart to verify the preference. Draw again to verify its effect;
   existing note ink is still memory-only until M2 and will not survive restart.

Local validation: formatting, Clippy (`-D warnings`), 50 passing tests (two manual
benchmarks ignored by default), native build, WASM check and Trunk release build.
The release smoothing benchmark passed separately: 100 samples took 0.200 ms
incrementally / 0.080 ms for one rebuild; 100,000 samples took 129.149 ms
incrementally / 49.616 ms for one rebuild (about 1.3 µs per appended sample).
These are local geometry CPU measurements, not GPU/frame/input latency guarantees.

Firefox headless/WebGL2 smoke drew matching shallow zigzags at 50%, 0% and 100%
in one Whiteboard while retaining earlier ink. Visual comparison showed raw
corners at 0% and increasingly rounded small irregularities at higher strengths;
a separate right-angle stroke remained sharp at 100%. Slider interaction and
keyboard undo/redo worked. After discarding the generated scratch ink and
reloading, localStorage/preferences schema 2 retained 100% smoothing. Automated
controller tests cover Page Notes and all five strength/persistence levels.
Physical handwriting latency, native graphical smoke and Windows/macOS runtime
still require matching devices. Local comparison screenshot:
`/tmp/xitoc-smoothing-comparison.png` (not a repository artifact).

## M1.1 stroke and partial erasers (#13)

Automated: analytic capsule disk/strip/end-cap/stationary intersections compared
against independent point-distance samples; middle/end/dot/full/thick cuts;
legacy/adaptive curve survivors preserving every unaffected visible vertex;
repeat/overlap cut merging and tangency without fragment explosion; exact JSON
fragment round trips and pressure interpolation; large origins/long segments.
Controller events cover multiple smoothed strokes in both layouts at 0.25×/1×/4×,
transient previews without document mutation, single-command commit, exact
original/fragment history and stacking, whole-erasing generated polylines and
branch truncation. Tests verify stable/idle preview mesh reuse, rejection of
invalid replacement IDs/style/data, menu mode/size, preference migration and
persistence. Existing #14/#12/PDF/storage/UI tests remain required.

Deterministic performance checks: query only two 16-segment blocks from a
100,000-segment path, preserve original mesh Arcs during partial previews and
reuse cut meshes for repeated identical samples. Run
`cargo test --release partial_ -- --ignored --nocapture` for 100/10,000-stroke
sampling/commit timings and 100/100,000-segment setup/sampling. These CPU timings
exclude GPU rendering/event/display latency; commits and initial fragment meshes
scale with affected vector geometry. Genuine thousands of separated holes can
require thousands of fragments; continuous/repeated cuts must coalesce.

Manual native/WebGL2 browser:
1. Create Page and Whiteboard notes; draw curves, dots, thick strokes and several
   crossing lines at different smoothing strengths.
2. Open the eraser dropdown between Pen and Hand, select Stroke Eraser, sweep
   across several strokes. Whole strokes disappear; one undo restores the sweep.
3. Select Partial Eraser, sweep across the middle and ends of multiple strokes.
   Only intersected sections disappear; visible fragments keep their shape/style.
   Undo once restores all originals in their stacking order; redo restores cuts.
4. Adjust radius in the same dropdown and compare the cursor ring/gap. Repeat
   after zoom/pan/resize and on another tab/type, then erase a survivor again.
5. Reload/restart to verify the selected mode; only preferences persist until M2.
   Current ink remains memory-only; discard only the scratch drawings for reload.
6. Check narrow Tools/document submenus, both themes and keyboard navigation.

Local validation for #13: formatting, Clippy (`-D warnings`), 61 tests passed
(four manual benchmarks ignored by default), native build, WASM check and Trunk
release build. Both partial benchmarks and the existing whole-stroke eraser
benchmark passed separately in release mode.

| Partial CPU workload | Result |
| --- | ---: |
| 100 strokes: 2,000 pointer samples / commit | 10.109 ms / 0.090 ms |
| 10,000 strokes: 2,000 pointer samples / commit | 5.742 ms / 7.846 ms |
| 10,000-stroke document index setup | 9.766 ms |
| 100,000-segment path-block index setup | 2.196 ms |
| 100,000-segment path: 2,000 local samples | 9.133 ms |

Host load affects these measurements; the smaller fixture is not guaranteed
faster in one run. Path-block setup replaced an initial per-segment hierarchy
that took 50.684 ms on the same long-path fixture. The #14 whole-stroke benchmark
still completed 200 warm empty-space samples on 10,000 strokes in 0.006 ms, with
an 8.216 ms document index build. Benchmarks exclude rendering/display latency;
real overlapping content and affected geometry size still matter.

Firefox headless/WebGL2 smoke: created a scratch Whiteboard, drew straight and
smoothed strokes, selected Partial Eraser and swept through all three. Matching
local gaps appeared with preserved survivors and the effective-radius ring.
Keyboard undo/redo restored/reapplied the entire sweep. Stroke Eraser removed
all three originals with one sweep, and undo restored them. The browser
localStorage key contained the selected mode in preferences schema 3. A browser
reload check was interrupted by server recovery; restart persistence is covered
by automated preference round trips. Local screenshot
`/tmp/xitoc-partial-eraser.png` is a session artifact, not a note format or repo file.
Native graphical/physical-device and Windows/macOS runtime checks remain manual;
both layouts and pan/zoom/far-origin behavior have automated controller coverage.

## M1.1 pressure-sensitive input (#15)

Automated: finite/clamped/invalid/absent/zero pressure; old numeric and missing
sample/style JSON; mouse/disabled/0%-strength constant-width fallback; increasing
width and round cap radii; exact local smoothing pressure interpolation and
bounded incremental reconstruction; actual ordered synthetic force/pointer events
on Page and Whiteboard at 0.25×/1×/4×; preview/release/cache equality; local-width
whole/partial erasing, interpolated surviving fragments and exact undo/redo;
randomized tapered clipping versus independent distance samples; settings schema
1–3 migration, range validation, restart persistence and application to new ink.
Existing eraser-index pruning, mesh reuse and smoothing tests remain required.

Manual browser and compatible Windows hardware:
1. Create Page and Whiteboard notes. In Drawing enable pressure, strength 100%,
   minimum 20%. Draw slowly varying light/heavy curves and dots. Check continuous
   rounded thickness, endpoint response, and no release refit. Mouse stays constant.
2. Try 0/50/100% strength, 1/20/100% minimum, enable off/on and smoothing
   0/50/100%. Existing ink must retain its exact appearance. Change settings via
   keyboard and in a narrow dialog with both themes and UI display scaling.
3. Zoom/pan, draw far from origin, erase thin and thick portions with both modes,
   undo/redo and repeat cuts. Eraser ring alignment and responsiveness must hold.
4. Test pen leaving canvas, release outside, contact cancellation, focus loss,
   switching tabs and finger/mouse navigation. Secondary contacts must not supply
   force to the primary stroke. Test browser UI zoom as well as document zoom.
5. Reload/restart only after discarding scratch ink. Preferences must persist;
   note ink remains memory-only until M2. Verify constant-width tablet fallback
   on Linux/macOS and Windows drivers delivering only mouse/legacy touch events.

Physical device pressure/latency and Windows/macOS runtime are unverified here.
Synthetic pressure does not certify hardware, driver support or display latency.

Local validation for #15: fmt, Clippy with warnings denied, 68 tests passed
(four manual benchmarks ignored), native build, WASM check and Trunk release
build. Both existing release partial-erasing benchmarks passed separately:
10,000-stroke index build 8.43 ms, 2,000 partial samples 3.72 ms, commit 8.04 ms;
100,000-segment index setup 3.09 ms, 2,000 local samples 9.03 ms. Host load varies;
these are CPU measurements, not hardware handwriting/display latency guarantees.

Firefox 157 headless/WebGL2 smoke created a scratch Whiteboard, adjusted ink
width, and rendered a synthetic DOM pen stream ramping from 0 to 1 pressure.
Visual inspection confirmed a smooth thin-to-thick curve and round caps; the
queue drained and the unsaved-ink guard activated. Firefox BiDi does not implement
pen pointerMove, so the DOM test temporarily stubbed setPointerCapture for its
synthetic pointer ID; genuine pointer capture and hardware were not exercised.
Drawing pressure sliders changed persisted strength/minimum to 48%/49%; the same
schema-4 localStorage values survived discarding the generated session on reload.
Settings and drawing screenshots are local session artifacts in `/tmp`, not PDFs
or repository fixtures. Automated controller tests cover both document types,
whole/partial erasure/history, secondary-contact isolation and cancellation.
Native Windows/macOS runtime and physical pen testing remain outstanding.

## Archived M1.3 floating drawing workspace (superseded by top toolbar below)

Automated regression exercises the actual floating palette and editor together
in Page/Whiteboard layouts: Pen options appear; Eraser changes the shared
preference/controller mode; adjustments remain open; Escape dismisses on the next
frame; disabled parent UI prevents tool changes; no control click produces ink.
Existing controller, smoothing, pressure, erase/history, mesh cache and spatial
index regression tests stay required. No engine/model/PDF codec changes.

Local validation: fmt, Clippy with warnings denied, 68 tests passed (four manual
benchmarks ignored by default), native build, WASM check and Trunk release build.
Both release partial-erasing benchmarks passed separately. 10,000 strokes:
index setup 8.07 ms, 2,000 local samples 3.65 ms, commit 4.93 ms. 100,000-segment
path: index setup 2.05 ms, 2,000 samples 5.52 ms. Host-dependent CPU measurements
exclude GPU/display/input latency; this UI slice preserves engine caching/indexes.

Performed Firefox 157 / WebGL2 visual smoke with actual release application:
created Page and Whiteboard session PDFs using tab +; dismissed scratch export
dialogs without claiming saved files; inspected white A4 on neutral workspace,
borderless dotted board, palette and both popovers at 1100×760; resized to
480×360 and inspected both themes and scrolling Pen options. Explorer, tabs and
palette occupy separate regions. Drew using mouse pointer events, whole-erased
the stroke and restored it with Ctrl+Z; dirty indicator/export guard followed
history. Screenshots are indexed in `screenshots/README.md`. This verifies the
app against our specification, not pixel parity with Apple's current macOS UI.

Linux native executable was built and launched without startup errors; native
screenshot capture failed (host image tool unavailable for this purpose). Native
interactive drawing, physical pen latency and Windows/macOS runtime remain manual.

To reproduce: run `cargo run --locked`, open a scratch workspace, create both
notes with +, inspect palette/settings and both themes. For browser run
`trunk serve`, open localhost, create both session notes, dismiss export dialogs.
Check small window, popover scrolling/Escape/outside-click, tab switching,
Hand/Space pan, Ctrl/Cmd-wheel/pinch zoom, both erasers and undo/redo. Test Settings
changes against the same smoothing/pressure/mode controls. Ink remains memory-only;
discard only scratch drawings. Proposed future issues/dependencies are recorded
in `CREATIVE_TOOLS.md`; none of their tools are exposed as working controls.

## M1.3 top toolbar, pen types and presets — current acceptance

This supersedes the bottom-floating-toolbar interaction specification. Tools are
in the fixed 40-point panel directly below tabs on both document types.

Automated coverage:
- Preset create/edit/reorder/delete-custom/restore, stable selection/IDs, 32-entry
  limit, name/style/range/unknown-type validation and semantic JSON round trips.
- Preferences 1–4 migrate to schema 5, retaining color/width/smoothing/pressure
  and eraser values; migration produces stable identity. Restart round trips
  retain library order and selection. Invalid/future preferences remain guarded.
- Actual UI clicks create/reorder/delete/restore presets. Toolbar clicks select
  Highlighter in one click without ink on both layouts; creation leaves its editor
  open rather than dismissing itself; disabled controls cannot select tools.
- All three types capture style independently from later preset edits. Synthetic
  force makes Monoline/Highlighter constant-width and Pressure responsive; absent
  force still falls back. Partial fragments keep type/opacity/style, and exact
  undo/redo and JSON round trips hold on Page and Whiteboard.
- Independent triangle-coverage sampling verifies translucent joins/chunk seams
  do not overpaint; idle cached meshes are reused. Existing eraser-index,
  incremental geometry, pressure, zoom, layout and storage/PDF tests remain.

Local checks: fmt, Clippy with warnings denied, 75 tests passed (four manual
benchmarks ignored by default), native build, WASM check and Trunk release build.
Both release partial-erasing benchmarks also passed. 10,000 strokes: index setup
7.46 ms, 2,000 local samples 3.28 ms, commit 4.86 ms. 100,000-segment path: setup
2.64 ms, 2,000 samples 8.16 ms. CPU measurements vary with host load and do not
measure physical pen/GPU/display latency. Existing opaque rendering and spatial
indexes remain intact; translucent strokes reuse the same cached chunk pipeline.

Performed actual Firefox 157/WebGL2 release-app inspection at 1100×760 and
480×360: top toolbar in both layouts/themes, inline color picker, preset settings,
custom creation/rename/reorder and scrolling management controls. New-preset
popover self-dismissal and narrow strip clipping were caught and fixed, then
rechecked. Created Monoline ink and drew a translucent Highlighter over it; the
underlying ink stays readable. Verified custom name/order/selected UUID in the
actual localStorage state before and after browser reload; all survived. Only
scratch session ink was discarded. Screenshots: `screenshots/README.md`.
Linux native executable launched without startup errors. Physical pressure and
interactive native Windows/macOS behavior remain unverified.

Manual reproduction:
1. Run `cargo run --locked`, select a scratch real workspace and create both types.
   Or run `trunk serve`, open localhost and create browser session notes. Dismiss
   scratch export dialogs; ink cannot be saved to PDF until M2.
2. Select Black/Blue/Red, Highlighter and Pressure pen. Inspect color/thickness
   previews and selection. Draw, smooth, pan/zoom, erase in both modes, undo/redo.
3. Use preset +; its editor opens. Rename, change type/color/width/opacity/smoothing
   and pressure options. Create multiple custom presets, reorder, delete custom
   entries and restore defaults. Default presets may be edited/reordered but not
   deleted. Confirm Settings → Drawing uses the same values.
4. Resize to 480×360, scroll preset strip with trackpad/Shift-wheel or Tab through
   buttons, open/scroll popovers, dismiss by Escape/outside click. Check light/dark
   and confirm no bottom toolbar or overlap with Explorer/tabs.
5. Restart/reload only after discarding scratch ink; presets survive independently
   of document bytes. Repeat on compatible physical stylus hardware for pressure.

## M1.4 selection implementation gates

Test shared identity/bounds/placement and backward semantic decoding; click/multi/
full-envelope lasso (concave, crossing, degenerate, pressure and thick ink); uniform
move/resize, cancel and one-command commit; copy/duplicate/paste/delete with fresh
IDs, preserved properties/order and exact undo/redo. Test scope/page bounds, zoom,
DPI and far-origin coordinates; transformed whole/partial erasing; focus/shortcut/
context-control isolation; clipboard payload validation and cross-tab fallback.
Run all existing fmt/Clippy/test/native/WASM/Trunk gates and retain cache/index
performance checks. Actual native/browser observations must be recorded separately
from compilation, including narrow windows, both themes and document types.


### M1.4 performed verification

Formatting, Clippy with warnings denied, all-target tests (95 passed, five ignored
manual benchmarks), native build, wasm32 check and Trunk release build passed.
All five ignored release benchmarks also passed, including indexed lasso with a
4,000-edge region around a 100,000-point path (27.28 ms containment on this host).
Cached mesh reuse during preview and indexed erasing remain covered. These CPU
measurements do not measure physical stylus/display latency or establish a
cross-device performance guarantee.

Actual Firefox 157/WebGL2 release app was exercised at 1100×760 and 480×360:
click/multiple selection, live lasso, move preview, corner resize, duplicate,
copy between Whiteboard and Page, Edit-menu paste, delete/undo/redo, context
controls and light/dark appearance. Browser inspection caught unsupported web
RequestPaste dispatch; web menu Paste now uses the session bundle immediately,
while standard DOM paste events can supply system payloads. Captures are linked
in `screenshots/README.md`. Linux native startup passed; interactive native
selection/system clipboard, Windows/macOS and physical pen remain manual.

Manual reproduction:
1. Run `cargo run --locked` or `trunk serve`; create scratch Page and Whiteboard
   documents. Draw with multiple presets; ink remains memory-only until M2.
2. Choose Selection, click ink, Shift-click to toggle others, click empty canvas
   to clear. Choose Lasso, enclose complete strokes, release; Selection returns
   with a group box. Touching or crossing the lasso boundary excludes a stroke.
3. Drag selected ink; drag a corner to scale proportionally. Repeat after zoom
   and pan, and on page edges. Escape cancels a drag. Undo/redo each gesture.
4. Use Ctrl/Cmd+A/C/V/D, Delete and Ctrl/Cmd+Z/Shift+Z, then equivalent Edit and
   context menus. Copy into another document; check fresh IDs through tests.
   Edit text in a preset popover and confirm canvas shortcuts do not steal input.
5. Erase transformed ink with both erasers; undo. Resize the window to 480×360,
   check both themes and confirm contextual controls stay inside the editor.

Intentional limits: page-local selection, proportional scaling only, conservative
rounded highlighter envelope, 4,096 lasso vertices (overcomplex gestures cancel),
and clipboard caps of 512 objects / 200,000 input points / 4 MiB. No PDF ink saving,
future object creation, nonuniform scaling or rotation is implemented.
