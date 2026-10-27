# XiToc UI design system (M0.6)

Follow this document for all interface work. `src/ui/style.rs` is the source of
Rust tokens; update both together. Use egui logical points, never physical pixels.
The interface supports desktop and browser with one component tree.

## Principles and hierarchy

Keep the application compact, quiet and useful. Content receives the largest
area; chrome uses flat surfaces, restrained separators and a single teal accent.
Do not copy other products' assets. Avoid decorative cards, oversized buttons,
heavy borders, gradients or animation. Every enabled control must perform its
advertised action. Future tools are disabled with milestone tooltips.

Order: application bar → document tabs → document toolbar → editor, with a
resizable Explorer on the left and a status bar across the bottom. Keep filesystem
IO, document mutations and PDF encoding out of UI components. Never introduce a
document-type switch: layout comes from immutable document metadata.

## Colors

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| Shell | #F6F7F9 | #1B2028 | Application bar, tabs, status |
| Sidebar | #F0F2F5 | #181D24 | Explorer |
| Surface | #FFFFFF | #242B35 | Toolbar, active tab, menus/dialogs |
| Text | #202731 | #E6EAF0 | Primary labels |
| Muted | #637083 | #9CA8B9 | Secondary labels |
| Border | #D9DEE6 | #353E4A | Restrained control outlines |
| Accent | #256B68 | #73C7BC | Active type icon, focus, tab underline |
| Selection | #DDEDE9 | #29443F | Selected row/button |
| Hover | #E5E9EF | #2F3845 | Hover feedback |
| Workspace | #DFE3E9 | #10151C | Around Page Notes |
| Board | #F6F8FA | #1B212A | Borderless Whiteboard |
| Dot | #C6CFD9 | #404C5C | Whiteboard grid |

Appearance menu selects Light or Dark. Light is the initial default; theme and
Explorer visibility are session preferences, not PDF data. Pages remain white in
both themes. Use egui's semantic error color for errors, never color alone to
explain a failure. Avoid hard-coded colors outside tokens except white paper,
its neutral edge and shadow.

## Typography and spacing

Use the bundled egui proportional font. Body/buttons: 13 pt; secondary labels:
11 pt; headings: 18 pt; wordmark: 16 pt bold; empty-editor heading: 20 pt. Avoid
all-caps except the small Explorer heading. Use strong text for category labels,
not every control. Truncate long file/tab/status labels and reveal full text on
hover; errors also have a dedicated readable status window.

Spacing scale: 2, 4, 6, 8, 10, 16, 24 pt. Standard horizontal gap 8, vertical gap 6,
button padding 8×4, tree indentation 16. Controls and tree file rows are 26 pt high;
icon buttons 26×26, glyph box 14–16. Application bar 38, tab strip 36 (tab 32),
document toolbar 40, status bar 28. Control corner radius 4, dialog radius 6.
Explorer starts at 248 pt, resizes from 132 to at most 360 and 45% of the window.

## Icons, interaction and navigation

Use the original monoline vector icons in `src/ui/icons.rs`; no emoji or external
icon font dependency. Page: outlined sheet with lines. Whiteboard: wide outlined
canvas with dots. Folder: folder outline. Plus/minus, reset, grid, sidebar and close use
consistent strokes. Pair document icons with readable kind labels or tooltips;
color alone never identifies a type. Icon buttons have accessible names and
hover descriptions, with keyboard focus inherited from real egui buttons.

Explorer category chevrons and labels expand/collapse. Folder names select the
real creation destination; their chevrons expand/collapse. Files have hover and
selected backgrounds. Context menus expose existing creation actions; do not
advertise rename/move/delete before M3. Categories are virtual views of real paths.
Tabs show type icon, filename, active surface and 2 pt accent underline, close
button and full path/type tooltip. They scroll horizontally and reveal newly
activated tabs. Tab focus uses an accent outline; Enter/Space activates a focused
tab. Closing never deletes a file. Do not show a dirty indicator until content
editing and genuine dirty-state tracking exist.

Toolbar reserves space for Pen, Eraser, Undo/Redo (M1), Select, Text and Shapes
(M6). These are disabled and explain availability on hover. Working Whiteboard
view controls are zoom out/in, Reset view and optional dot grid. Page Notes show
A4 / fit-width information. No save control claims an unsupported save operation.
Browser Export PDF remains an explicit export request, not a verified disk save.

## Document surfaces

Page Notes: white A4 rectangles with 0.75 pt gray edge and subtle black shadow
(alpha 28/255, offset 0×3, blur 10, spread 1). Center pages in a neutral workspace,
24 pt outer margin and page gap. Width fits available space up to 794 pt; below
210 pt page width, horizontal scrolling preserves readable geometry. Retain the
page list and label pages beneath the paper; do not add multipage editing yet.

Whiteboards: fill the editor with the board color, no paper edge or page shadow.
Optional low-contrast dots follow the existing camera, spaced adaptively to avoid
excessive drawing. Grid visibility and camera changes are session-only. Drag or
space-drag pans; Ctrl/Cmd-wheel or pinch zooms around the pointer. Toolbar zoom
anchors at the viewport center. These are navigation controls, not a drawing engine.
External PDFs display a truthful read-only/preview-unavailable explanation.

## Responsive behavior and component boundaries

The Explorer can be hidden with the top sidebar button. Narrow Explorer headers
use “Files” and a Workspace menu; long trees scroll. Tabs scroll rather than wrap.
Toolbar contents move into Tools/View menus; below 360 pt editor width, one typed
document menu contains the available view/export actions and disabled tools.
Inline view controls require 600 pt; inline future tools require 820 pt. Status
context is omitted on narrow windows; the status text truncates and the details
button always exposes the complete message. Dialog inputs follow available width.
No fixed physical-pixel sizing; egui handles display scaling and canvas resizing.

`app.rs` owns state and executes actions. `ui/chrome.rs`, `explorer.rs`, `tabs.rs`,
`toolbar.rs` and `dialog.rs` render their respective areas and return intents;
`style.rs` and `icons.rs` are reusable foundations. `editor.rs` renders metadata-
derived document surfaces and maintains transient navigation. Domain/PDF/storage
remain independent. Add small components as needed, not a framework or parallel
editor implementations. Verify narrow/wide windows, both themes and scaling when
extending this layout; see `TEST_PLAN.md`.
