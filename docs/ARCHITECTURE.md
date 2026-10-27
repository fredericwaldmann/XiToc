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

## Core state
Document { schema_version, doc_id, mode, objects, pages?, viewport?, settings }. Object variants: Stroke { id, points, color, width }, later Text, Shape, Image. Stable IDs, canvas coordinates as f64 if necessary for large whiteboards. Undo/redo is command-based. Edits update memory first, then serialized snapshot on save.

## Storage lifecycle
User opens a workspace. Explorer lists nested folders and actual PDFs. On edit, flag dirty; save serializes editable JSON + generates visible PDF + embeds metadata into PDF bytes. Native writes atomically where possible, web obtains explicit directory write permission or exports a new `.pdf`. Never silently claim a successful save after browser download when there was no write handle.

## Build constraint
Make native and WASM compile from the first milestone. If a library is native-only, hide it behind target-specific modules, not behind a broken compile path.
