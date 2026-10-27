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
  Actions/desktop machines; local archive fixture tests do not prove them.
