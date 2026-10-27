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
