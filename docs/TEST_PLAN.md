# Tests and CI release gates

The general lists include planned acceptance tests, not a statement that all
features exist. Milestone result sections are historical observations; later
results supersede earlier counts/scope. See [CODEBASE_GUIDE.md](CODEBASE_GUIDE.md)
for current tests/CI and [DEVELOPMENT_LOG.md](DEVELOPMENT_LOG.md) for attribution.
Ink PDF round trips, multipage creation and browser directory handles remain future gates.

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

## Unified canvas object architecture follow-up

Automated gates cover unique object creation and duplicate rejection, lookup and
missing IDs, object type/ownership, ordered traversal, undoable stacking in both
directions and redo branching, rotated AABBs/anchors/hits, inverse/composed
transforms, raw style/sample preservation, pressure/dot hits and legacy placement
JSON. Controller regression exercises both erasers on scaled and rotated ink in
Page and Whiteboard with exact undo/redo; existing input/presets/smoothing/cache/
index tests remain required. Rotation is a domain API, with no new UI control.

Manual verification: run `cargo run --locked` or `trunk serve`, create scratch
Page and Whiteboard notes, draw with every preset and varying supported pressure,
select/move/resize, erase using both modes, undo/redo, pan with Space and zoom
with Ctrl/Cmd-wheel. Check existing clipboard/duplicate/delete and overlapping
stroke selection. No new toolbar controls should appear; ink remains memory-only.
Physical pen, native Windows/macOS and interactive browser behavior require
separate runtime verification; compilation and headless UI tests do not certify them.

Performed for this follow-up: formatting, Clippy with warnings denied, 98 tests
passed (five benchmarks ignored by default), Linux native build, wasm32 check and
Trunk release build passed. All five ignored release benchmarks also passed.
On this host, 10,000 strokes: index build 8.86 ms, 2,000 partial samples 5.11 ms,
commit 10.65 ms; 100,000-segment path: index 4.72 ms, 2,000 samples 8.86 ms.
Host load affects these CPU measurements; they are not input/display latency
measurements. No interactive runtime or physical stylus smoke was performed in
this follow-up. Existing headless controller/UI regression tests passed.

## Selection/Lasso acceptance follow-up (#23)

The top-toolbar controls, pressure-envelope picking and full-envelope lasso were
already implemented by PR #28. The follow-up changes canvas Escape to cancel a
live gesture and clear membership/bounds/outlines together. It adds four tests in
`selection_tests.rs` using actual ordered egui pointer/key frames:

- Idle and active additive-lasso Escape on both layouts; late pointer release
  cannot select/commit; original vectors, revision, undo sequence and cached mesh
  Arc remain unchanged. The active-lasso assertion failed before the fix.
- Single/multiple selection at 0.25×/1×/4× with view offsets; a point inside a
  diagonal AABB but outside its ink is rejected, and screen-point tolerance stays
  consistent with visible ink width.
- Outside-paper click and lasso-start rejection, contrasted with the same logical
  positions on an unbounded Whiteboard; ownership and history stay unchanged.
- Cancel lasso by changing tools, draw a dot, select it, erase in both modes,
  undo and switch to Hand without creating accidental ink.

Performed: fmt check, Clippy (`--locked --all-targets -- -D warnings`), full tests
(102 passed, five ignored), Linux native build, wasm32 check and Trunk release
build. All five ignored release benchmarks passed. On this host, indexed lasso
containment on 100,000 points/4,000 polygon edges took 48.94 ms; 10,000-stroke
partial erasing took 8.00 ms for 2,000 samples and 13.99 ms commit. Benchmarks ran
alongside other work; timings are host-dependent CPU observations, not hardware
input/display latency or a cross-device guarantee. Drawing hot paths are unchanged.

Actual Firefox 157/WebGL2 release app, controlled through WebDriver BiDi: created
scratch Page and Whiteboard PDFs, cancelled export dialogs, drew two strokes,
selected by click, Shift-added on Whiteboard, watched live lasso, completed lasso
and pressed Escape during another live lasso. Inspected before/after captures:
all outlines disappeared, pointer release did not restore selection, both vectors
remained. Ctrl+A/Escape, dark appearance, tabs/Explorer and 480×360 layouts also
worked. No bottom drawing toolbar appeared. Session captures are under
`/tmp/xitoc-selection-*.png`; these are local test artifacts, not saved note data
or committed fixtures. Headless browser mouse events do not verify physical pen,
interactive native input or Windows/macOS runtime.

Manual reproduction: run `cargo run --locked` or `trunk serve`, create scratch
notes of both kinds, draw multiple strokes, click Selection and Shift-click another
stroke. Click empty canvas to clear. Select Lasso, enclose strokes, release, then
start another lasso and press Escape before releasing. Both the lasso and existing
selection must disappear without changing ink. Repeat after zoom/pan, with both
themes and a narrow window; switch to Pen/Hand/both erasers and exercise undo/redo.
Text editing and open popovers must retain their own shortcuts. Ink remains
memory-only; do not discard drawings you need to keep.

## M1.4 object manipulation and clipboard

`manipulation_tests.rs` runs through the shared Canvas controller and real egui
frames. Coverage includes:

- Group rotation at 0.25×/1×/4×, raw points/style/pressure/IDs/order/ownership,
  unchanged committed revision during preview, one commit per release and exact
  undo/redo. Vertex positions are compared with an independent view/placement
  mapping; base mesh Arcs and idle preview Arcs are retained.
- Shift snapping, including pressing/releasing Shift without moving; Escape/late
  release cancellation; full-turn angle unwrapping near 1e12 coordinates and
  view-change cancellation.
- Page rotation rejection, 1/10-point keyboard nudges, ownership, proportional
  off-axis corner drags with/without Shift and a 1-point longest ink extent floor.
  Handwriting intentionally has no independent X/Y stretch mode.
- Cut/Paste across all Page/Whiteboard source/target combinations, fresh IDs,
  complete captured style/placement, group offsets, native Cut-event/shortcut
  deduplication, session fallback, delete and exact undo/redo. Oversized Cut keeps
  objects, history, selection and the preceding clipboard untouched.
- Text focus, modified navigation keys and active gestures retain input priority.
  The complete rotation handle hit target stays outside contextual controls; this
  regression failed with the earlier toolbar clearance and passed after the fix.

Performed checks: formatting, Clippy with warnings denied, 111 tests passed
(six ignored benchmarks), Linux native release build, wasm32 check and Trunk
release build. All six ignored release benchmarks passed. The rotation fixture
(2,000 strokes/40,000 samples) measured 44.01 ms first preview, 12.15 ms subsequent
changed frame and 5.45 ms idle preview on this host during concurrent builds.
Existing erasing/smoothing/lasso benchmarks passed too. These CPU observations do
not certify physical-device/display latency. Ordinary drawing bypasses rotation
preview transforms; live moves/resizes retain the original layer-transform path.
The Linux release executable stayed alive for an eight-second timed startup with
no emitted errors; that is not interactive native manipulation verification.

Manual reproduction:
1. Create scratch Page/Whiteboard notes. Draw several strokes, including supported
   pressure input and translucent presets; select a group by Shift-click or Lasso.
2. Drag ink, then a corner. Check relative placement, uniform nib/pressure scaling,
   off-axis drags/Shift and the minimum extent. Undo/redo each completed gesture.
3. Drag the round handle above the group through a full turn; Shift snaps to 15°.
   Change Shift while stationary. Escape cancels and clears; release must not
   commit. Repeat near page edges, after pan/zoom and far from Whiteboard origin.
4. Use Ctrl/Cmd+D/C/X/V, paste into the other document type, Delete/Backspace,
   arrows and Shift-arrows. Check fresh selected copies, undo/redo, exact styles
   and preserved page ownership. Use Edit/right-click rotation and clipboard actions.
5. Repeat with light/dark and narrow windows; edit preset names and ensure Cut/
   arrows stay in text input. Toolbar remains below tabs; ink is memory-only.

Physical stylus, interactive native manipulation/system clipboard durability,
Windows/macOS runtime and low-end-device latency need their corresponding devices.

Actual Firefox 157/WebGL2 release-app checks: scratch Whiteboard drawing, group
rotation commit, rotation undo/redo, Ctrl+D offset duplicate/undo, Cut leaving an
empty board and Paste restoring the group; cross-document paste into a Page Note,
Shift-snapped rotation there, 1/10-point keyboard nudges, dark theme and 480×360
layout. Inspected before/after captures under `/tmp/xitoc-manip-*.png` (session
artifacts, not committed notes/fixtures). Initial browser testing reproduced native
page Select All starting a browser drag and swallowing release. The canvas A/D
key-default and dragstart guards fixed the observed event stream. A fresh load of
the final packaged HTML passed DOM assertions: canvas Ctrl+A, Cmd+D and native
DragStart are prevented, but a separate text field's Ctrl+A is not prevented.
Clipboard key defaults remain available to eframe's native Copy/Cut/Paste events.
This browser verification is headless mouse/keyboard input, not physical pen or
interactive native clipboard certification.

## Unified Selection (#34, 2026-11-04)

Automated gates: fmt, Clippy `-D warnings`, 117 tests pass; seven manual benchmarks
ignored by the normal suite. Native release build, WASM check and Trunk release
build pass. New tests cover both layouts, all four rectangle diagonals, full versus
partial envelope selection, Shift/Alt area membership, zoom/pan, repeated Shift
clicks, both-mode movement/undo/redo, direction changes/Escape, rotated objects,
zero-width containment and exact crossing rejection for empty bounds overlap.
The toolbar event test opens the actual dropdown, selects Lasso and recalls it.
Existing lasso, resize/rotation, clipboard, focus and drawing regressions pass.
`cargo test --release indexed_rectangle_large_selection_performance -- --ignored --nocapture`
passes: 2,000 objects / 100 warmed samples in 27.56 ms (geometry only, host dependent).
A timed Linux native release launch remained alive without errors until timeout.

Manual review procedure:
1. Create a Page Note and a Whiteboard; draw separated, overlapping and diagonal ink.
2. Select using the single top Selection button. Open its arrow and verify Rectangle
   and Lasso choices; switch to Pen and back to verify mode recall.
3. Click ink, Shift-click twice (membership stays), Alt/Option-click to subtract,
   then click empty space to clear. Drag selected ink to move its group.
4. Drag empty canvas left-to-right in both vertical directions: solid blue and only
   complete envelopes. Reverse horizontal direction: dashed green and crossing ink.
   Reverse direction during a drag; verify immediate policy/preview changes.
5. Repeat under zoom/pan and with Shift/Alt captured at pointer-down. In Lasso mode,
   enclose complete ink, add/remove regions, then repeat without reselecting the mode.
6. Resize/rotate (Shift snaps), duplicate/copy/paste/delete and undo/redo. Escape
   cancels gestures; text and popup focus retain shortcuts. Verify both themes and
   narrow layout. A window manager reserving Alt-drag may require rebinding that OS
   gesture; the app cannot receive events intercepted before its input pipeline.

Interactive Windows/macOS, physical stylus and native gesture performance still
require those devices. Build/controller checks do not claim those observations.

Firefox WebDriver BiDi smoke on the rebuilt WASM app: fresh-load document creation,
single top Selection control and Rectangle/Lasso dropdown, drawing two strokes,
solid blue containment excluding the partial stroke, dashed green crossing
including both, and lasso selecting one while retaining Lasso/transform handles.
Screenshots: [dropdown](screenshots/unified-selection/dropdown.png),
[containment](screenshots/unified-selection/containment.png),
[crossing](screenshots/unified-selection/crossing.png),
[lasso](screenshots/unified-selection/lasso.png). Desktop/browser modifier paths
are covered by the shared ordered-event tests; this smoke did not interactively
exercise every modifier, both themes or Windows/macOS window-manager bindings.


## Object flipping (#33, 2026-11-04)

Performed checks: `cargo fmt --all -- --check`, Clippy with `-D warnings`,
124 passing tests (seven ignored manual benchmarks), native release build,
WASM check and Trunk release build. The Linux native release launch stayed alive
without errors until its deliberate eight-second timeout.

New coverage in `flip_tests.rs`, `object.rs` and `ui/selection_controls.rs`:
horizontal/vertical single/group flips on both layouts at 0.25×/1×/4× view scale;
rotated/scaled smoothed pressure ink; unchanged IDs/raw samples/styles/widths,
bounds and unselected Arc/cache reuse; exact one-step undo/redo and double-flip
geometry; far-origin placement composition/inversion over every reflection parity;
legacy/defaulted semantic snapshots; clipboard and subsequent move/resize/rotate,
partial erasing; atomic Page rejection; right-click group/unselected/empty behavior
from Rectangle/Lasso/Pen/Eraser/Hand and actual Transform submenu command routing.
Existing drawing, selection, rotation/resize, clipboard and focus tests pass.

Actual Firefox packaged-WASM smoke created a Whiteboard, drew asymmetric strokes,
selected both, right-clicked one without losing the group, opened Transform and
clicked Horizontal flip. Undo restored the original drawing; Redo restored its
reflection; Vertical flip then reflected the group vertically. A DOM assertion
confirmed eframe prevents default `contextmenu` on the canvas (the handler is
canvas-scoped). Drawing-region pixel crops match exactly for before versus Undo
and horizontal flip versus Redo. The five flip/menu tests also pass in release mode.
Review screenshots: [before](screenshots/object-flipping/before.png),
[Transform](screenshots/object-flipping/transform.png),
[horizontal](screenshots/object-flipping/horizontal.png),
[undo](screenshots/object-flipping/undo.png),
[vertical after horizontal](screenshots/object-flipping/vertical.png).

Manual review:
1. Create Page/Whiteboard notes, draw asymmetric strokes and select one or several.
2. Right-click a selected stroke (group stays), an unselected stroke (only it is
   selected), then empty canvas (no content edits). Transform is disabled when
   nothing editable is selected. Repeat from Pen/Eraser/Hand.
3. Choose Transform → Flip Horizontally / Flip Vertically. Check mirroring around
   the collective box center and unchanged pressure/color/width/smoothing.
4. Rotate and resize ink first; repeat flips under zoom/pan. Flip twice on one
   axis to restore geometry. Undo/redo each flip and verify selection/identity.
5. Move/resize/rotate, copy/paste/duplicate/delete and erase after flipping.
   Try Page edges: a flip that moves a centerline outside paper must fail intact.
   Check light/dark and narrow windows; the top tools/tabs/Explorer stay in place.

Interactive native gesture behavior, Windows/macOS, physical stylus and exhaustive
browser theme/device-latency checks still require their devices. The native
startup check is not an interactive native manipulation certification.


## Basic shapes (creation slice of #24, 2026-11-04)

The actual repository issue is #24 “[M1.5] Shapes and Styling”, with no comments;
no separate “Basic Shape Creation” issue exists. Its styling/embedded-text criteria
remain open. This slice implements the user-requested eight-form geometry and
shared editor integration, not those excluded features.

Performed checks: formatting, Clippy `-D warnings`, 132 active tests passing
(seven manual benchmarks ignored in the ordinary suite), native release build,
WASM check, Trunk release and a timed Linux launch with no startup errors before
its deliberate eight-second timeout. All seven ignored release benchmarks pass
when explicitly run. Under concurrent build/benchmark load: 2,000-object rectangle
100 previews 95.46 ms; 2,000-stroke rotation unchanged preview 2.67 ms / changed
6.32 ms; 10,000-stroke warm sparse eraser 200 samples 10.25 µs. These CPU records
are host-dependent observations, not frame/device guarantees or controlled
before/after comparisons. Incremental common-store appends preserve the ink fast
path; shape meshes reuse Arcs on idle frames.

Automated shape fixtures (`shape.rs`, `shape_tests.rs`) cover all eight forms in
all four drag quadrants, both layouts and 0.25×/1×/4× view scales; live uncommitted
previews, creation/select/undo/redo; Shift proportions and line angles; click/zero
size/Escape/focus/view/tool cancellation and preview-cache cleanup; semantic JSON
validation/normalization; fill/no-fill, rounded/ellipse/triangle geometry and
shaft/head hits; mixed topmost and directional/lasso membership; shared movement,
resize/rotate/flip, duplicate/clipboard/Cut/Delete and exact mixed order/history;
local box/endpoint edits after rotation/reflection under zoom and idle mesh reuse.
Mouse comparisons account for the actual f32 screen coordinates entering egui.
Existing ink, pressure, eraser, clipboard, reflection and focus regressions pass.

Actual Firefox packaged-WASM pointer smoke:
- Created all eight forms from the real four-column top picker on a Whiteboard.
- Edited an arrow endpoint and independently resized a rectangle corner.
- Selected all, duplicated the group, then Undo restored the original eight.
- Inspected dark appearance and 480×360 layout with Explorer/tabs and the top
  picker visible; Hand remains reachable through View/Space-drag at narrow widths.
- Created a Page Note and inserted a rectangle and freestanding arrow on paper.
- Captures: [picker](screenshots/basic-shapes/picker.png),
  [eight forms](screenshots/basic-shapes/eight.png),
  [endpoint](screenshots/basic-shapes/endpoint.png),
  [corner resize](screenshots/basic-shapes/resized.png),
  [duplicate](screenshots/basic-shapes/duplicate.png),
  [dark](screenshots/basic-shapes/dark.png),
  [narrow picker](screenshots/basic-shapes/narrow-picker.png),
  [Page Note](screenshots/basic-shapes/page.png).

Manual review:
1. Create scratch notes of both kinds. Choose each Shapes glyph and drag in every
   direction. Hold Shift for square/circle or 45° segments. Escape cancels;
   clicks/degenerate drags must not create accidental objects.
2. Click shapes/ink, Shift-add and Alt-remove. Test left-to-right containment,
   right-to-left crossing and lasso, including no-fill interiors and diagonal
   lines/arrowheads. Confirm mixed topmost order and Ctrl/Cmd+A.
3. Move, rotate, flip and resize selected groups. For one box, drag its local
   corners (Shift preserves aspect); Circle stays circular. For a line/arrow,
   drag either endpoint (Shift snaps angle), including after rotation/reflection.
4. Duplicate/copy/paste/Cut/Delete and undo/redo. Check ID/style/order preservation,
   Page ownership/edges, pan/zoom/DPI and text-focus shortcut isolation.
5. Inspect both themes, narrow View/Hand and picker keyboard focus. Closing dirty
   shapes must use the same discard guards as ink. Do not expect PDF ink/shape
   saving, embedded text, formatting controls or magnetic attachments.

Interactive native manipulation, Windows/macOS/physical stylus, exhaustive
browser keyboard/high-DPI devices and low-end latency still require those devices.
Ink erasers intentionally remain ink-only; Delete/Cut removes semantic shapes.


## Shapes dragging regression follow-up (2026-11-05)

Three regressions were reproduced as failing tests before the fix: a rotated
singleton body hit an invisible legacy group resize corner; off-center endpoint
and corner grabs snapped to the pointer; changed movement previews replaced base
shape mesh Arcs. The fix shares the ink graphics-layer movement preview, excludes
hidden singleton resize targets and applies handle displacement to its original
position. Semantic document snapshots remain unchanged until release.

`shape_drag_tests.rs` covers all eight forms over 512 body-drag combinations:
Page/Whiteboard, 50/100/200/400% zoom, pan, rotation and single/multiple selection.
Pointer sequences include repeated positions, reversals, idle frames, exact layer
tracking, preserved styles/dimensions/IDs, one-entry undo/redo, mixed ink/shapes,
clicks, Escape, focus/tool handoff and net-zero movement. Handle-offset regressions
cover box and endpoint editing at all four zoom levels.

Passed: formatting check, Clippy all-targets with warnings denied, all-targets
unit tests (138 passed, eight ignored), native release build, WASM cargo check and
Trunk release build. Explicit release preview benchmarks both passed: existing
2,000-stroke rotation (initial 27.509 ms, idle 3.103 ms, changed 5.099 ms) and
1,000-ellipse movement (40 changed frames in 350.200 ms, all base meshes reused).
These are CPU egui harness timings, not display latency or frame-rate guarantees.

Actual Firefox packaged-WASM pointer smoke created and moved all eight forms,
repeated/reversed drag positions, cancelled a drag and exercised undo/redo.
Each shape's repeated-position screenshot crop was pixel-identical. This browser
smoke used 100% zoom; the full zoom/pan matrix above is automated controller
coverage. Native release launched without startup errors during an eight-second
smoke check (timeout intentionally ended it). Interactive native dragging,
Windows/macOS, hardware stylus and device-specific latency remain unverified.

Manual: create each form, Select its body away from visible handles, then drag
from an off-center point. Repeat near a rotated shape's bounding-box corners,
with multiple shapes and ink selected, and at 50/100/200/400% after panning.
Check no initial jump, stable size/angle, accurate reversals and immediate tracking.
Drag visible corners/endpoints off-center to check resizing retains the grab
offset. Click without moving, cancel with Escape, then complete a move and
Undo/Redo; only the completed move should enter history.


## Persistent group dragging / viewport regression (2026-11-05)

Supersedes the first Shapes drag follow-up's incomplete viewport coverage. Before
production edits, two new tests failed: the preview's global clip shrank after a
50-point translation, and PointerGone discarded a held two-object drag. These
were visual clipping/reference-frame and lifecycle defects, not accumulated
movement-delta errors. The corrected shared painter replaces its clip with the
inverse viewport; the session retains frozen IDs/sources/view/revision and ignores
hover loss while preserving explicit release/cancellation semantics.

Five new tests cover global clip invariance, hover-loss continuation, actual mesh
submission for a mostly offscreen oversized shape, group cancellation/view changes,
and fast/coalesced pointer sequences over 50/100/200/400/800% zoom with and without
pan. The fast test includes seven selected objects (including an initially
invisible member), viewport-sized/oversized geometry, 50,000-point jumps taking
all members offscreen, stationary frames, return, and hover loss coalesced with
release. It verifies exact shared displacement, frozen snapshots, unchanged
rotation/scale/IDs/membership, one transaction and exact undo/redo. Existing
single/mixed/rotated shape, area selection, ink, eraser and history tests remain.

Passed: cargo fmt, Clippy all-targets with warnings denied, all-target tests
(143 passed, eight ignored), native release build, WASM cargo check and Trunk
release. Both explicit release preview benchmarks pass: 1,000 ellipses / 40
changed frames 343.947 ms total with all mesh Arcs reused; 2,000-stroke rotation
initial/idle/changed frames 28.422/3.138/7.708 ms. CPU harness timings are not GPU
latency or universal frame-rate promises.

Actual packaged-WASM Firefox gestures selected five shapes, moved slowly then
jumped hundreds of pixels per event, left the editor into the sidebar/top panel,
returned, released and undid/redid. A six-object selection including a 720×470
logical-point rectangle was tested at exactly 400% zoom (the enlarged rectangle
exceeds the viewport in both axes, with most area offscreen). Repeat-position
canvas crops matched pixel-for-pixel at 100% and 400%; undo matched the original
canvas, redo matched all five object-body crops, and explicit mouse pointercancel
restored the pre-drag canvas exactly. The browser cancellation test bypassed its
asset cache to load the updated inline bridge. See screenshots/shape-dragging.

Native release started without errors. Attempts to capture/control its desktop
window failed: the application uses the available Wayland desktop, while X root
capture returned X get_image error 8 and X window lookup found no XiToc window.
Native interactive dragging, physical input latency and Windows/macOS remain
unverified. Use the following manual protocol on a real desktop and browser:

1. Create five forms and select all; drag one selected body slowly, then make
   fast large jumps and reversals. Confirm fixed relative spacing and no scaling.
2. Pan, repeat at 50/100/200/400%, and create a rectangle larger than the viewport.
   At 400%, start from its visible portion, move until only a small part remains,
   release, then drag again from that visible part. Keep invisible objects selected.
3. While held, move the pointer into the sidebar/top panel and back; confirm the
   session continues. Release outside the editor and Undo/Redo the whole move.
4. Cancel a new move with Escape, genuine focus loss or browser pointercancel;
   check no geometry/history change. Click without moving: no history entry.
5. On Page Notes repeat after scrolling/zooming. Actual page edges constrain
   geometry; the visible scroll viewport must only clip graphics. Navigation or
   layout changes during a gesture safely cancel rather than rebase it.

## Shape styling and formatting (M1.5 slice of #24, 2026-11-06)

Automated coverage: 155 active tests passing, eight manual release benchmarks
ignored in the ordinary suite. New shape styling fixtures cover fill color/alpha/
no-fill, border color/alpha/width/enablement, solid/dashed/dotted rendering,
start/end/both/no heads through rotation/scale/reflection/endpoint editing,
clamped radius and later shrinking, semantic/default compatibility and validation,
clipboard suitability, untouched ink/unselected Arcs, IDs/geometry/order,
compatible batches, exact undo/redo, mixed initial values and redo truncation.
Controller fixtures verify captured creation defaults on both layouts at
50/100/200/400% zoom; cached renderer fixtures verify alpha and invisible
no-fill/no-border output at those scales. Existing dragging, offscreen clipping,
hover loss, group movement, local handles, selection/lasso, transforms, clipboard,
ink and eraser regressions pass.

Two formatting gesture regressions have focused coverage: mixed-to-common values
must preserve control identity/position, and a changed final slider sample on
release must merge with the preceding held-pointer samples. The latter was
reproduced failing before cleanup moved to end_frame. Displaying 0/1/35/64/127/
128/254/255 byte alpha must never modify it merely by rounding the displayed
percentage; a separate UI test guards against such implicit edits.

Performed checks: cargo fmt --all -- --check, cargo clippy --all-targets -- -D
warnings, cargo test --all-targets, native cargo build --release and cargo check
--target wasm32-unknown-unknown. Native release remained alive without errors for
a deliberate eight-second startup timeout. This is startup, not native interactive
input certification. Final Trunk release packaging is recorded below.

Actual packaged-WASM Firefox smoke created shapes, changed fill opacity through
a continuous pointer drag, used Undo/Redo, chose dashed borders, opened the canvas
Format submenu, created a freestanding arrow and selected Both ends. Inspected
the selection toolbar and opacity controls in Light/Dark at 1100×760 and the
wrapped toolbar at 480×360 with a partly offscreen selected rectangle. The main
drawing tools remain directly below tabs; Explorer and tabs retain their layout.
Visual evidence: [fill Light](screenshots/shape-styling/fill-light.png),
[fill Dark](screenshots/shape-styling/fill-dark.png),
[narrow](screenshots/shape-styling/narrow.png),
[Format](screenshots/shape-styling/format-menu.png),
[both heads](screenshots/shape-styling/arrowheads.png).

Manual verification:
1. Create all eight forms on Page and Whiteboard notes. Select closed forms and
   change Fill color/opacity, disable Fill, then restore it. Place shapes over
   handwriting to check source-over transparency and independent object styles.
2. Change Border/Line color, Width, Opacity and Solid/Dashed/Dotted; disable and
   restore borders. Repeat under pan/zoom and device scaling. Select mixed ink,
   boxes and segments: fill/radius/heads must skip incompatible content.
3. Select several differently styled rectangles. Drag a slider continuously,
   release at a new value, Undo once and verify all original mixed values. Redo
   restores the batch. Confirm opening any menu alone does not edit content.
4. Set None/Start/End/Both ends on segments, then move/resize/rotate/flip and edit
   endpoints. Adjust rounded radius, shrink its box, and check smooth clamping.
5. Repeat through right-click → Format, from Selection/Lasso and other tools.
   Confirm right-clicking a selected member preserves the group. Copy/paste,
   duplicate, delete, Undo/Redo and both erasers retain existing behavior.
6. Settings → Drawing → Default shape style: configure common style, radius and
   arrowheads; create more shapes and verify old objects are unchanged. Restart
   and verify preferences, without expecting any handwritten/shape PDF saving.
7. Repeat recent drag regression protocols above, especially partly offscreen/
   oversized objects and fast mixed-group movement. Test 480×360, both themes,
   keyboard focus and high-DPI devices on native and browser.

Windows/macOS runtime, physical stylus/high-DPI monitors, exhaustive interactive
native gestures and low-end device latency remain unverified. No magnetic
relationships, embedded text or PDF serialization are implemented.

Final `trunk build --release` completed successfully after the complete native/
WASM/check suite. The packaged application remains local-only for note content.

After the final build, a fresh Firefox session repeated a continuous fill-opacity
drag followed by one Ctrl+Z and Redo. RGB comparisons of the unobstructed shape
interior matched exactly before versus Undo and adjusted versus Redo; the adjusted
image differed from the original. This verifies the release/coalescing and
no-implicit-alpha-rounding fixes in the actual packaged application.

## Border rendering and color-picker correction (2026-11-06, PR #39)

Passed: formatting check, Clippy all-targets with warnings denied, 165 active tests
(eight manual benchmarks ignored), native Linux release build, wasm32 compilation
and Trunk release build. New fixtures verify cyclic rectangle edge/corner mesh
coverage at 50/100/200/400% with rotation, rounded/circle/ellipse continuity,
bounded acute joins, cross-corner joined dash lengths, full closing seam gaps,
equal closed dash periods, even nonoverlapping dots, density/degenerate bounds,
independent custom pattern lengths/thickness, legacy defaults, transform and
clipboard/duplicate/paste preservation, undo/redo, conditional UI controls and
interactive inline picker/hex changes without alpha/border mutation. Existing
smooth/offscreen/multi-object drag, selection/lasso/directional selection,
transform/clipboard, ink/preset/eraser and continuous-slider regressions pass.

Actual Firefox WASM reproduction: clicking the old Fill swatch dismissed the
parent menu and left no usable picker. The corrected inline picker remained open
while pointer changes to hue and saturation/value changed the selected fill.
Hex RGB #35A34D recolored that shape; a second rectangle's pixels stayed unchanged,
and the separate 24% fill opacity and blue border were retained. A six-object
border recolor to #A52B64 changed all outlines. One Undo restored all original body
pixels exactly; Redo restored all recolored pixels exactly (RGB crop excludes
menu/toolbar hover feedback). Dashed Style controls accepted dash 24/gap 8, while
Width still displayed the original 3 pt. Solid and Dotted conditionals are also
automated through real menu widgets.

The visual scene was seeded through the existing semantic clipboard event route
with six objects, then manipulated using actual pointer/key controls: solid and
dashed rectangles, dashed rounded rectangle, dotted circle, dashed triangle and
rotated dotted diamond. This is not certification of an OS system clipboard.
Final packaged-build screenshots inspect all six at 100%, and pan/zoom checks at
50/100/200/400%; zoomed solid corners remain fully connected. Additional 400%
dash/rounded/dot inspections are recorded with the captures below. The top drawing
toolbar, Explorer and tabs retain their existing placement.

Manual review:
1. Create the six forms above; disable fill to inspect their borders. Check every
   corner and seam while resizing/rotating/flipping and at 50/100/200/400% zoom.
2. Style → Dashed: set dash 12, gap 6, then 24/8. Check equal periods around the
   whole outline (closed fitting scales lengths uniformly). Change thickness and
   confirm requested dash/gap remain unchanged. Repeat for open lines/arrows.
3. Dotted: adjust spacing and diameter, including tight curves/skinny shapes.
   Check even distribution, no seam cluster and no geometric dot overlap. Solid
   must not show dash/dot controls; head triangles remain solid.
4. Fill/Border → Color picker: use hue/SV/RGB controls and custom hex RGB. Preview
   updates immediately, selection remains intact, and opacity stays independent.
   Repeat with mixed multi-selections and via right-click → Format and Settings.
5. Undo/Redo edits, including a continuous slider's final release sample; copy,
   duplicate and paste, transform objects, then reopen the formatting menus to
   verify custom values. Repeat on Page Notes; no PDF object saving is expected.

Interactive native Windows/macOS/high-DPI hardware and physical input latency
remain unverified; shared controller/mesh tests and native/WASM builds do not
certify those devices. No magnetic connectors or PDF persistence were added.

Captures: [six shapes](screenshots/border-correction/scene.png),
[working picker](screenshots/border-correction/color-picker.png),
[24/8 custom pattern](screenshots/border-correction/custom-dashes.png),
[unchanged thickness](screenshots/border-correction/thickness.png),
[50%](screenshots/border-correction/zoom50.png),
[400% solid corners](screenshots/border-correction/zoom400.png).
[400% dashed rectangle](screenshots/border-correction/dashed400.png) and
[400% rounded rectangle](screenshots/border-correction/rounded400.png).
Opaque pixel-run inspection of the rounded rectangle's straight top edge measured
49–50 px dashes (one-pixel raster rounding), consistent with uniform perimeter fit.
[400% dotted circle](screenshots/border-correction/dotted400.png) shows uniform
centers and round, separated dots after panning it fully into the editor.

## M1.6 straight magnetic connectors (2026-11-07)

Automated coverage includes all six closed shape kinds, four local ports,
rotation/reflection, far coordinates, one/both selected nodes moving, resizing,
multiple incident connections, endpoint reconnect/detach, safe deletion and exact
graph undo/redo. Clipboard tests check fresh IDs, internal reference remapping,
external reference detachment and cross-document Page placement. Validation rejects
self/missing/cross-surface references and unknown routing variants. A hub test
checks 80 incident edges and reuse of unrelated object snapshots.

Real egui controller tests exercise Page Notes and Whiteboards at 50/100/200/400%:
port creation, endpoint editing, live node-drag preview, cancellation, no-op clicks,
shape-over-connector hit priority, modifier selection, rectangle/lasso selection,
keyboard creation, page-margin ports, offscreen pointer movement and focus loss.
Coincident endpoint formatting preserves canonical arrowheads despite suppressing
their undefined direction in the render adapter.

Checks passed: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`
(177 passed, eight pre-existing ignored timing tests), `cargo build --release`,
`cargo check --target wasm32-unknown-unknown`, and `trunk build --release`.

Manual review:
1. Create three rectangles in either document kind. Select A; drag its right plus
   port to B, then connect B to C. Check live preview and target highlight.
2. Move, resize, rotate and flip B; both wires must track the correct local ports.
   Repeat with multi-selection and with nodes outside the viewport at 200/400%.
3. Select a wire away from shape bodies. Change color, opacity, width, dash/gap,
   dot settings and arrowheads using the existing contextual controls.
4. Drag a selected endpoint onto another closed shape, then onto empty space.
   Escape cancels. Shift-click adds and Alt-click removes selected objects.
5. Delete B: surviving wires retain their last positions as free endpoints.
   Undo restores B and its original relationships; redo repeats the detachment.
6. Copy A/B and their connector together: the pasted connector must reference the
   new shapes. Copy a wire alone: both endpoints become free.
7. Select two closed shapes and use the context command or Ctrl/Cmd+Shift+K.

These are straight connectors only. Orthogonal routing from issue #25 remains
out of scope. PDF persistence is still deferred to M2; no connector saving is
claimed. Native builds and shared tests do not certify native interactive input
or physical stylus latency.

The packaged WASM application was actually inspected in Firefox using pointer
and keyboard input. Three rectangles were created from the top shape picker,
connected A→B→C, then B was moved, resized and rotated. Both connections followed
its transformed local ports. Deleting B preserved the two free endpoint positions;
undo restored the node and relationships without adding the dependent wires to
the selection. Custom hex color and dashed styling were applied to one connector. An endpoint
was reconnected from C to A; Ctrl+Z and Ctrl+Shift+Z restored both attachment
states.
Rapid alternating pointer movements at 200% and 400% retained attachment while
the other nodes were offscreen. This is a functional browser check, not a
measurement of physical input latency.

Inspected captures: [ports](screenshots/magnetic-connectors/ports.png),
[live snapping](screenshots/magnetic-connectors/snap.png),
[rotation](screenshots/magnetic-connectors/rotated.png),
[deleted node](screenshots/magnetic-connectors/deleted.png),
[undo](screenshots/magnetic-connectors/restored.png),
[connector styling](screenshots/magnetic-connectors/styled.png),
[200%](screenshots/magnetic-connectors/zoom200.png),
[400%](screenshots/magnetic-connectors/zoom400.png),
[reconnected endpoint after redo](screenshots/magnetic-connectors/reconnected-redo.png).
