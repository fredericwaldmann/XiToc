# XiToc — Product specification

## Goal
A local-first note-taking editor with a VS Code-like explorer of actual directories/PDFs, and two editing modes sharing one Rust engine.

## Main window
- Left collapsible, resizable Explorer tree. Open workspace directory; nested actual directories; `.pdf` files; create/rename/move/delete via confirmation; tabbed currently open notes; file dirty/saved indicator.
- Editor center with tools (select, pen, eraser, hand, text, shapes). Start MVP with select/pen/eraser/hand only.
- Top mode indicator (`A4` or `Whiteboard`), zoom controls, undo/redo, save, shortcut hints. Use clean, original icons/branding, not exact copied assets.

## Modes
- A4: white pages at 210x297 mm; vertical pages; page margin and background templates; logical millimeter/point coordinate scheme; eventual PDF import/print.
- Whiteboard: unbounded logical coordinates, arbitrary pan/zoom, drawing anywhere; when saved, PDF page fits content bounds + padding; viewport and logical coordinate origin stored in the embedded edit metadata.
- The note mode is chosen on creation and stored in its PDF edit metadata. Both have same objects.

## Editing
MVP: pressure-agnostic freehand points, stroke width/color, erase a whole stroke, undo/redo, pan/zoom, zoom-to-content, autosave (debounced) and explicit save. Later: true stylus pressure, shape/text/image objects, partial stroke eraser, PDF background import, selection/lasso, tab management.

## Files
- Canonical files are `.pdf` only; real filesystem tree. No proprietary note sidecar.
- Other PDF readers must display visible note content; application reloads full editable model from embedded data.
- A PDF without edit metadata is *not* automatically a fully editable native note. Show as external PDF/read-only first.
- Native open-workspace operates only within the selected workspace; web uses browser permissioned APIs where possible, and file import/export fallback elsewhere.

## Privacy and scope
No backend, login, collaboration, cloud sync, PDF OCR, handwriting recognition, or mobile support in first release. No automatic upload of note content. Browser-delivered app assets may be fetched from GitHub Pages, but notes are handled locally.
