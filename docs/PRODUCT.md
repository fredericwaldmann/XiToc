# XiToc — Product specification

## Goal
A local-first note-taking editor with a VS Code-like explorer of actual directories/PDFs, and two document types sharing one Rust editor engine.

## Main window
- Left collapsible, resizable Explorer tree. Open workspace directory; nested actual directories; `.pdf` files; create/rename/move/delete via confirmation; tabbed currently open notes; file dirty/saved indicator.
- Editor center with tools (select, pen, eraser, hand, text, shapes). Start MVP with select/pen/eraser/hand only.
- Subtle document-type indicator (`Page Note` or `Whiteboard`) near the title; never an editor type switch. Zoom controls, undo/redo, save and shortcut hints arrive with their corresponding capabilities. Use clean, original icons/branding, not exact copied assets.

## Document types and creation
- **Page Note:** A4 pages at 210×297 mm, stacked vertically with page scrolling.
  Logical millimeter/point coordinates and a page list prepare for multiple pages.
- **Whiteboard:** borderless canvas with unbounded logical coordinates, pan/zoom
  and the same future drawing tools. A finite PDF page is only its export view.
- Type is chosen once via **New Document → New Page Note / New Whiteboard** or
  the Explorer context menu. Ask for a filename and use the selected real folder.
  Moving or renaming the PDF never changes its type. No conversion is offered.
- Explorer has expandable **Pages** and **Whiteboards** virtual categories.
  Each retains the actual workspace folder hierarchy (including empty folders for
  creation); each PDF appears only in its own category. No mandatory category
  directories, copies or path remapping. Folder selection supplies the creation
  destination. Refresh reflects external filesystem moves.
- M0.5 writes blank native PDFs with type/identity/settings embedded in that same
  PDF. On restart, select the workspace again and reopen; on web, reimport the
  exported PDF. No sidecar state is required to identify the document type.
- M0.5 whiteboard gestures: drag or space-drag to pan; Ctrl/Cmd-wheel or pinch to
  zoom around the pointer; toolbar +/− and Reset view are keyboard-focusable.
  Camera changes are session-only until the later save/edit lifecycle.
- Browser M0.5 uses import/export fallback and a flat session list in each virtual
  category. It does not expose real disk directories or claim a completed disk
  save after initiating a download. Create & export requests a PDF download;
  confirm the browser download or use Export PDF to retry.

## Editing
MVP: pressure-agnostic freehand points, stroke width/color, erase a whole stroke, undo/redo, pan/zoom, zoom-to-content, autosave (debounced) and explicit save. Later: true stylus pressure, shape/text/image objects, partial stroke eraser, PDF background import, selection/lasso, tab management.

## Files
- Canonical files are `.pdf` only; real filesystem tree. No proprietary note sidecar.
- Other PDF readers must display visible note content; application reloads full editable model from embedded data.
- A PDF without edit metadata is *not* automatically a fully editable native note. Show as external PDF/read-only first.
- Native open-workspace operates only within the selected workspace; web uses browser permissioned APIs where possible, and file import/export fallback elsewhere.

## Privacy and scope
No backend, login, collaboration, cloud sync, PDF OCR, handwriting recognition, or mobile support in first release. No automatic upload of note content. Browser-delivered app assets may be fetched from GitHub Pages, but notes are handled locally.

## M0.6 interface foundation

The application area, resizable/hideable Explorer, horizontally scrolling typed
tabs, compact document toolbar and bottom status line share a light/dark design
system (`UI_DESIGN.md`). Appearance switches themes for the current session.
Page Notes use white, subtly shadowed A4 pages in a neutral workspace; Whiteboards
use a borderless surface with an optional session-only dot grid. Narrow toolbars
collapse into menus. Status details expose complete errors and export caveats.
Pen/eraser/undo/redo and later selection/text/shapes remain explicitly disabled;
only implemented navigation, creation, opening and export actions are enabled.
Tabs switch/close existing documents without conversion or fabricated dirty state.
