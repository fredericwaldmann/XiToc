# XiToc UI design system (M0.6 foundation, M1 tools)

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

Appearance menu and Settings select Light or Dark. Light is the initial default;
theme and Explorer visibility are persisted application preferences, separate from PDF data. Pages remain white in
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
tab. Closing never deletes a file. M1 prefixes dirty tab titles with a filled dot
and exposes “Unsaved drawing (memory only)” in the tooltip. The bottom status
always warns when any tab has unsaved ink. Discard dialogs default to keeping
work; Escape cancels. Never label memory-only ink as saved.

The M1 toolbar provides Pen, Eraser and Hand selection, an Ink settings menu,
Undo/Redo and View zoom controls. Active tools use the selection background;
Undo/Redo are disabled when their document journal has no corresponding action.
Ink contains an inline expandable opaque color picker (Edit color), pen width (0.25–12 document points) and
eraser radius (4–48 logical screen points). The default pen is blue (#2563C8),
2.5 pt wide. M1 renders constant-width ink; pressure hardware comes later.
The screen-space eraser ring must match the swept hit-test radius. Use logical
points, not physical display pixels, for its dimensions. View supports both
layouts; Page 100% fits the available width. Hand/Space-drag pans without ink.
Selection, Text and Shapes remain disabled in the Ink menu with M6 tooltips.
Browser Export PDF is disabled while that tab contains unsaved ink, explaining
that drawing export is M2. Blank/original PDF export remains an explicit request,
never a verified disk save. Ctrl/Cmd+S explains the persistence limitation.

## Document surfaces

Page Notes: white A4 rectangles with 0.75 pt gray edge and subtle black shadow
(alpha 28/255, offset 0×3, blur 10, spread 1). Center pages in a neutral workspace,
24 pt outer margin and page gap. Width fits available space up to 794 pt; below
210 pt page width, horizontal scrolling preserves readable geometry. Retain the
page list and label pages beneath the paper; do not add multipage editing yet.
M1 zoom scales that fit-width baseline; scrollbars expose enlarged pages. Ink is
stored page-locally in PDF points, clipped to paper, and never in the margins.

Whiteboards: fill the editor with the board color, no paper edge or page shadow.
Optional low-contrast dots follow the existing camera, spaced adaptively to avoid
excessive drawing. Grid visibility and camera changes are session-only. Pen draws by default;
Hand, space-drag or middle-drag pans; Ctrl/Cmd-wheel or pinch zooms around the
pointer. Toolbar zoom anchors at the viewport center. M1 ink uses the same
controller and renderer as A4, without page bounds.
External PDFs display a truthful read-only/preview-unavailable explanation.

## Responsive behavior and component boundaries

The Explorer can be hidden with the top sidebar button. Narrow Explorer headers
use “Files” and a Workspace menu; long trees scroll. Tabs scroll rather than wrap.
Toolbar contents move into Tools/View menus; below 360 pt editor width, one typed
document menu contains drawing, view and permitted export actions.
Inline drawing controls require 760 pt; inline view controls require 1100 pt.
Smaller editors use Tools/View menus without changing toolbar height. Status
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

## Settings (M1.1, #6)

Edit → Settings opens a centered modal with General, Appearance, Drawing and
Input Devices sections. Section selectors wrap at narrow widths; the content
scrolls independently so Done remains accessible. `SETTINGS_WIDTH` is 520 logical
points; the modal leaves 24 points of outer clearance. Reuse the standard surface,
heading, separators, spacing and focus styles. Done and Escape close the dialog.
The editor beneath is disabled while the dialog is open.

General includes editor behavior (Show Explorer). Appearance reuses Light/Dark.
Drawing reuses the Ink color picker and width/radius controls for defaults applied
to newly created/opened tabs. Input Devices includes middle-button Whiteboard
panning. Hand/Space navigation remains available. Changes apply automatically;
preferences persist locally with a short write debounce. Appearance menu and
Explorer button edit the same preferences. Persistence warnings appear in Settings.

Partial-erasing and pressure controls remain explanatory text until their
behaviors exist. Do not expose working-looking controls for
them. Preferences never modify existing stroke objects or PDF contents.

### Stroke Smoothing (#12)

Settings → Drawing contains a Stroke Smoothing group and a 0–100% Strength slider.
`SMOOTHING_SLIDER_WIDTH` is 200 logical points, constrained by available width.
Use the Border token for its visible rail and the standard selection trailing fill.
Show the scale: 0% Off, 25% Light, 50% Medium, 75% Strong, 100% Maximum. Default
50% balances jitter reduction and detail. State that it applies to new strokes
in every open tab. Persist it with other preferences. Existing ink is never
reshaped by this control. Retain the shared spacing, typography and colors.
