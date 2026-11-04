# XiToc — Product specification

For implemented behavior and source references, see [CODEBASE_GUIDE.md](CODEBASE_GUIDE.md).
The main-window/files requirements include planned work; milestone sections record
staged delivery, with later sections superseding older scope.

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
  and the same implemented drawing tools. A finite PDF page is only its export view.
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
Implemented in memory: editable freehand vectors, pen presets, captured smoothing
and optional pressure, whole/partial erasers, undo/redo, pan/zoom, selection/lasso,
move/proportional resize/rotation, clipboard (including Cut), arrow nudges and
tabs. Arrange remains an internal API without controls. Shape/text/image/formula/connector objects, zoom-to-content,
PDF background rendering, note autosave/recovery and explicit ink save remain
planned. Application preference persistence does not save handwriting.

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
system (`UI_DESIGN.md`). Appearance switches themes using persistent application preferences.
Page Notes use white, subtly shadowed A4 pages in a neutral workspace; Whiteboards
use a borderless surface with an optional session-only dot grid. Narrow toolbars
collapse into menus. Status details expose complete errors and export caveats.
M0.6 left drawing tools disabled until their milestones; M1 enables the tools
listed below. Tabs switch/close existing documents without conversion; M1 adds
genuine dirty-state feedback for memory-only ink.

## M1 handwriting behavior

Both document types share freehand vector drawing, captured pen settings and
whole/partial erasing. The current top toolbar exposes presets/Pen settings,
Selection/Lasso, Eraser, Hand, Undo/Redo and View. Hold Space to pan without
changing tools; Whiteboards also support middle-button panning. Ctrl/Cmd-wheel or
pinch zooms; Page Notes scroll and clip ink to A4. Navigation/history belong to
each open tab; the preset library is shared application state. Ctrl/Cmd+Z undoes,
Ctrl/Cmd+Shift+Z redoes. M1.4 implements Selection/Lasso; text/shapes remain planned.

**Drawing changes are memory-only until M2.** A dirty tab and bottom status expose
this limitation. Close/workspace replacement/native quit ask before discard;
browser reload/leave requests the browser's standard unsaved-change warning.
Forced shutdown and browsers that suppress prompts cannot be prevented. Original
PDFs remain unchanged; exporting a dirty drawing is disabled. Ctrl/Cmd+S explains
that saving ink is unavailable. Blank creation/import/export retain M0.5 behavior.

## Application Settings (M1.1)

Edit → Settings provides General (Explorer visibility/editor behavior), Appearance
(Light/Dark), Drawing (shared preset library and eraser defaults/mode) and Input Devices
(middle-button Whiteboard panning). Done or Escape closes it. Preferences apply
automatically and persist on the device, independently of workspace PDFs.
Drawing defaults initialize new editors; the top preset editor and Settings share
the same library for subsequent strokes. Toolbar eraser radius is per-tab.
The Appearance menu and Explorer button use the same persisted preferences.
Pressure controls are available under Drawing; see the pressure input section below.

## Handwriting smoothing (M1.1)

Settings → Drawing and the toolbar preset editor provide a persistent per-preset
0–100% strength slider (default 50%). Off preserves raw input, Light/Medium/Strong/Maximum
progressively reduce small irregularities while retaining sharp turns. It affects
new strokes in all open Page Notes and Whiteboards. Existing strokes retain their
captured strength, raw editable samples, identity/style and undo/redo history.
The preview follows the pointer and uses the same geometry after release.
Handwriting remains memory-only until M2; this does not add PDF ink saving.

## Two eraser modes (M1.1, #13)

The Eraser toolbar button opens a popover for Stroke Eraser or Partial Eraser
and a per-tab screen-point radius control. The ring shows its effective eraser radius; hit testing
also accounts for pen width. The selected mode persists as an application
preference and is also available in Settings → Drawing.

Stroke Eraser removes the entire touched stroke. Partial Eraser removes only
intersecting portions of its visible vector centerline; surviving pieces retain
color, width and shape as editable polylines. One sweep across any number of
strokes is one undo operation; undo restores the exact original objects and
stacking order, and redo restores the same fragment IDs/geometry. Both document
types, navigation and smoothing remain supported. Ink is still memory-only until
M2; no PDF serialization or note sidecars are added.

## Pressure-sensitive handwriting (M1.1, #15)

Pressure Pen strokes vary thickness using normalized device force when available.
Drawing settings persist enable/strength/minimum width and affect future strokes,
while existing ink retains its captured response. Mouse input stays constant-width.
Both layouts, smoothing, both erasers and undo/redo retain editable pressure data.
Browser pen PointerEvents and Windows winit pen-force input have a supported code
path; native Linux/macOS tablets currently use constant-width fallback. Physical
hardware is unverified. See PRESSURE_INPUT.md. Saving ink remains the M2 milestone.

## M1.3 top drawing toolbar and presets

Drawing tools remain directly below document tabs; no bottom palette. One-click
horizontal preset buttons show color/thickness and selection. Five defaults:
Black/Blue/Red Monoline, Highlighter, Pressure pen. Users can create a custom copy,
edit type/name/color/width/opacity/smoothing/pressure, reorder, delete custom
presets and restore defaults in Pen settings. Settings → Drawing edits the same
library. Presets persist locally, separately from PDF contents. Legacy drawing
preferences migrate without losing configured values.

Monoline is constant-width; Pressure Pen uses device force with constant-width
fallback; Highlighter is a wide semi-transparent vector stroke. Existing smoothing,
both erasers, history and both layouts remain. Selection is visibly disabled until
its object-selection milestone. No realistic pencils/watercolor or PDF saving is
introduced. Explorer, tabs, menu bar and Settings dialog remain.

## M1.4 universal selection and lasso

The top toolbar provides one Selection button with Rectangle/Lasso dropdown.
Click topmost ink; Shift adds, Alt subtracts, and Ctrl/Cmd+A selects the scope.
Rectangle is default: left-to-right contains the whole envelope, right-to-left
crosses it. Lasso requires the whole rounded pressure envelope to be enclosed.
The chosen mode remains active after closing. Selected ink moves by dragging and resizes with
proportional corner handles. Duplicate/copy/paste/delete, undo/redo, Edit/context
menus and nearby Duplicate/Delete/Smaller/Larger controls share the same commands.
Transforms preserve raw samples and captured pen properties. Page ownership and
clipping remain explicit; no shape/text/formula/image creation or PDF saving.
