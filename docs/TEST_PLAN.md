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

## M0 manual smoke check
- Native: `cargo run --locked`; browser: `trunk serve`, open the localhost URL.
- Resize Explorer by dragging its divider; shrink and expand the window.
- Switch A4/Whiteboard by click and keyboard focus (Tab, Space/Enter).
- A4 shows a fitted portrait white page at 210:297 aspect ratio on gray;
  Whiteboard shows a full-area grid without a bounded page.
- Confirm the Explorer has no invented file entries and tools are disabled.
- Restart/reload: preview resets; no save success, file access or persistence.
- Inspect browser console for startup errors. Test WebGL2-disabled startup
  shows an error instead of leaving the loading message indefinitely.
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
