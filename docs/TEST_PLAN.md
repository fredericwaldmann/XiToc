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
