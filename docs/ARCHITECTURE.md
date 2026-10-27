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
  size checks before reading. Completion means export requested, not verified
  permanent storage. The original imported PDF bytes are retained for export;
  unfamiliar PDFs are never reconstructed or silently converted.
- `editor.rs`: common editor initialized from document layout; page geometry and
  whiteboard camera affect presentation and navigation only. World coordinates
  stay f64; only local offsets become screen f32. No type setter/conversion tool.
- `app.rs`: creation dialog, virtual category trees, tabs, clear read-only/error
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
