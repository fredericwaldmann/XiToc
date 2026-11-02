# XiToc UI design system — M1.3 drawing workspace

Follow this specification for future UI changes. Keep `src/ui/style.rs` tokens
and this document synchronized. All dimensions are egui logical points. Use one
Rust/egui component tree on desktop and WASM. Original vector icons live in
`src/ui/icons.rs`; no Apple assets, icon fonts or remote images.

## Reference and hierarchy

Primary UX reference: [Apple Freeform for Mac: drawing and sketching](https://support.apple.com/guide/freeform/draw-and-sketch-frfm6e3d132d2/mac),
consulted November 2026. Adopt compact drawing tools, nearby contextual options,
and direct canvas manipulation. This is an adaptation, not a pixel replica.
Apple's indexed guide text confirms bottom-of-window drawing tools, contextual
nib/color options, and object/partial erasing. Its full page/images returned
HTTP 429; detailed current macOS visual parity has not been verified.

Preserve XiToc's application menu bar, resizable real-folder Explorer, horizontal
scrolling document tabs, New Document (+) beside tabs, Settings and status bar.
Order inside the editor: tabs → top drawing toolbar → canvas. Drawing tools
MUST stay directly below the tabs. No bottom floating toolbar.
Document type is chosen at creation and derived from metadata, never a toggle.
Browser import/export retains truthful session/download language. Ink remains
memory-only until M2, with dirty indicators and existing discard protections.

## Colors and appearance

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| Shell | #F6F7F9 | #1B2028 | Application bar, tabs, status |
| Sidebar | #F0F2F5 | #181D24 | Explorer |
| Surface | #FFFFFF | #242B35 | Toolbar, active tab, popovers/dialogs |
| Text | #202731 | #E6EAF0 | Primary labels |
| Muted | #637083 | #9CA8B9 | Secondary labels |
| Border | #D9DEE6 | #353E4A | Restrained outlines |
| Accent | #256B68 | #73C7BC | Selected icons, focus, active tab underline |
| Selection | #DDEDE9 | #29443F | Selected tools/rows |
| Hover | #E5E9EF | #2F3845 | Hover feedback |
| Workspace | #DFE3E9 | #10151C | Around Page Notes |
| Board | #F6F8FA | #1B212A | Borderless Whiteboard |
| Dot | #C6CFD9 | #404C5C | Optional grid |

Pages stay white in both themes. Theme and Explorer visibility persist locally;
grid and camera remain session state. Use semantic error colors with readable
messages, never color alone. Avoid gradients, animation and heavy outlines.

## Dimensions and typography

Body/buttons 13, secondary text 11, headings 18, wordmark 16 bold, empty heading
20. Spacing scale: 2, 4, 6, 8, 10, 16, 24. Ordinary controls/tree rows 26;
ordinary icon buttons 26×26 with 14–16 glyphs. App bar 38, tab strip 36 with 32
high tabs, document header 40, status 28. Ordinary corners 4, dialogs 6.
Explorer starts at 248, resizes from 132 to min(360, 45% window).

## Top drawing toolbar and presets

Fixed 40-point top panel directly below the document tabs, with 8×6 padding and
4-point item gaps on Surface. Controls are 26×26 with original 14–16-point icons
and 1.25-point strokes. Preset buttons are 40×26. No floating palette or shadow.
Never overlay Explorer, tabs or canvas with the drawing toolbar.

Order: Selection, Lasso, Hand, horizontal pen presets,
New preset (+), Pen settings, Eraser, Undo/Redo, zoom. No unfinished pen types.
One click selects a preset and Pen; its preview combines a nib icon with its actual
color/opacity and a thickness stripe clamped to 1–8 screen points. Hover and
accessible names expose name, type, document-point thickness and opacity.
Selected presets use Selection fill and outline; keyboard focus uses normal egui
focus styling. Color alone is not the only selection indication. Hand and Eraser
show the same selected-tool treatment.

The five editable/reorderable defaults are Black pen, Blue pen, Red pen
(Monoline, 2.5 pt, 100% opacity), yellow Highlighter (18 pt, 35%), and purple
Pressure pen (4 pt, 100%). Blue is initially selected. New preset copies the
selected style with a new stable ID and opens its editor. Up to 32 presets are
stored locally. Only custom presets can be deleted. Move left/right reorders
any preset while retaining identity and selection. Restore default presets
explicitly replaces custom and edited presets with the five defaults.

The preset strip scrolls horizontally with trackpad/Shift-wheel; Tab navigation
reveals focused buttons. Selecting, reordering or resizing reveals the selected
preset. At editor widths below 800, zoom moves into View; below 500, history also
moves there. The toolbar remains one row. At the native minimum 480×360 window,
Explorer remains usable and at least one preset is accessible in the strip;
hiding Explorer exposes more tools. No wrapped tool rows or bottom fallback.
Whiteboard-only grid stays in zoom/View. Browser export is in View on compact
windows and inline only with at least 1000 editor points. Dirty ink export stays
disabled. The immutable document type remains in the tab/tooltip and status.

## Preset popover and state ownership

Pen settings opens below the top toolbar; preferred width 280, constrained by
viewport edges, content scrolls at limited heights. It provides editable name
(up to 64 characters), implemented pen type, inline expandable Color picker,
thickness 0.25–24 PDF points, opacity 1–100%, smoothing 0–100%, and Pressure Pen's
enable/strength/minimum controls. A blank name displays as “Untitled pen”.
Color expansion stays inside the popover, without replacing it with a second root
popup. Slider rails use Border fill and selection trailing fill in both themes.
New/Delete/Move/Restore controls live in this same editor. Inside adjustments
remain open; outside click and Escape dismiss naturally. No modal dialogs are
needed to manage presets.

Preset values are the single live style state. Toolbar and Settings → Drawing
use the same preset library and shared editor widgets. Settings retains its
existing sections, layout and persistence. Selection/order/edits persist using
the existing preferences backend; no workspace sidecars or ink storage. All tabs
use the selected preset for subsequent strokes. Active gestures capture a style
snapshot at pointer-down, so edits never change existing strokes or history.

Monoline always ignores pressure for thickness, while retaining smoothing.
Pressure Pen captures the configured response and uses normalized device force
when available; absent force and mouse keep constant width. Highlighter ignores
pressure and uses a wide translucent vector ribbon. Separate strokes and true
self-crossings accumulate opacity via normal source-over compositing. Translucent
ink has a flat nib; ordinary opaque ink retains the rounded renderer. No textures
or watercolor types. Opacity is captured into stroke RGBA, not a registry lookup.
Native Linux/macOS pressure hardware currently uses constant-width fallback.

Eraser opens a compact popover with Stroke Eraser / Partial Eraser and active-tab
radius (4–48 logical screen points). Mode remains the shared application preference;
Settings retains the default radius for newly opened tabs. Its cursor ring and
one-gesture/one-undo behavior stay unchanged.

Settings remains Edit → Settings, centered 520-point preferred width with 24
outer clearance, General/Appearance/Drawing/Input Devices sections, scrollable
content, persistent preferences, Done/Escape dismissal. Underlying editor is
disabled while Settings is open. Popovers need no new modal dialogs.

## Document surfaces and controls

Page Notes: white A4 pages, 0.75 gray edge, black shadow alpha 28/255, offset
(0,3), blur 10, spread 1. Neutral Workspace surrounds paper, outer/page gap 24.
Fit available width up to 794; minimum paper width 210 with scrolling. Preserve
the page list, boundaries, per-page labels and page-local ink clipping. Multipage
creation remains #11; no new page-editing feature in M1.3. The top toolbar stays fixed while paper scrolls.

Whiteboards: Board fill, no A4 bounds/edge/shadow. Optional low-contrast adaptive
dot grid follows the existing camera. Infinite f64 world coordinates remain
unchanged. Both types share the same top toolbar, controller and renderer.

External PDFs show read-only/preview-unavailable and expose no enabled drawing
tools. Both document kinds retain existing navigation, erasing and undo behavior.

## Navigation, shortcuts and accessibility

- Pen: crosshair; Eraser: radius ring; Hand: grab/grabbing. Space temporarily pans.
- Space-drag pans either type; optional middle-drag pans Whiteboards.
- Ctrl/Cmd-wheel or pinch zooms; View zoom anchors at viewport center. Page 100%
  means fit-width baseline; Whiteboard 100% means unit camera scale.
- Ctrl/Cmd+Z undo; Ctrl/Cmd+Shift+Z redo. Buttons disable at journal boundaries.
- Ctrl/Cmd+S reports that ink saving requires M2. Never mark current ink saved.
- Tab/Shift+Tab moves focus through controls; Enter/Space activates buttons/tabs;
  Escape dismisses popovers or Settings. Retain existing platform behavior.
- No unmodified tool hotkeys added; do not steal future text-entry keystrokes.

Tabs retain kind icon, filename, full path/type tooltip, dirty dot, close button,
selected surface and 2-point underline; scroll horizontally instead of wrapping.
The + action stays beside the scrollable tab region. Closing never deletes PDFs.
Explorer virtual categories retain actual hierarchy; folder selection determines
creation destination. Truncate long labels with full hover text. Status details
keep full errors and the unsaved-ink caveat accessible on narrow windows.

## Boundaries, performance and verification

`app.rs` coordinates state/actions. `toolbar.rs` renders the top toolbar;
`drawing_controls.rs` shares settings widgets; style/icons centralize visuals.
Document model, PDF, storage, geometry, smoothing, indexes and mesh caches remain
independent. No IO, timers or unconditional repaint requests in toolbar drawing;
closed popovers do not construct their content. Preserve existing incremental
stroke meshes, spatial eraser indexes and large-coordinate transforms.

Verify actual running app at 1100×760 and 480×360, both themes and document types.
Capture workspace, Page, Whiteboard, preset settings and Eraser popovers; confirm no overlap
with Explorer/tabs, no bottom palette, no accidental ink from controls, drawing/erasing/history,
resize, grid and zoom. Record performed checks in TEST_PLAN, distinguish build
checks from visual verification and physical pen/display latency tests.

## M1.4 Selection, Lasso and contextual object controls

Selection and Lasso are adjacent working controls at the left of the TOP toolbar.
Selection clicks pick topmost visible ink (4 logical-screen-point tolerance);
empty clicks clear. Shift-click toggles membership; Alt-click subtracts. Ctrl/Cmd+A
selects all in the active page/canvas scope and activates Selection. Text-input
focus retains its own shortcuts. Lasso draws a live outline and closes at release:
only fully enclosed rounded stroke envelopes are selected, excluding boundary
contact. Shift adds, Alt subtracts; a degenerate or focus-cancelled lasso leaves
selection. Escape cancels any live selection gesture and clears membership,
bounds and outlines, including the previous selection during an additive lasso.
A later pointer release cannot commit the cancelled gesture. Text editors and
open popovers retain their own Escape handling.
A completed valid lasso returns to Selection, exposing handles and direct movement.
Select Lasso again to add/subtract another region.

Selected ink has a subtle accent outline, group bounding rectangle and four
7-point corner handles with 5-point-radius hit targets. A 22-point minimum visual
selection rectangle keeps handles separated for dots and low zoom. Drag selected ink to move;
corner drags always scale proportionally about the opposite corner. A small drag
threshold avoids accidental transformations. One released drag is one undo entry;
Escape cancels the preview and clears selection; focus loss cancels only the
preview. Neither cancellation edits ink/history. Page ownership never changes by dragging.
Pen/Eraser/Hand remain independent tools; Space temporarily pans without selection.

A compact contextual toolbar near a visible selection offers Duplicate, Delete,
Shrink and Grow. It stays constrained inside the editor viewport; the main toolbar
never moves. Matching Copy/Paste/Duplicate/Delete/Select All and Undo/Redo commands
are exposed through Edit and canvas context menus. Ctrl/Cmd+C/V/D, Delete/Backspace,
Ctrl/Cmd+Z and Ctrl/Cmd+Shift+Z follow focus. The in-app clipboard remains available
when system clipboard permissions/integration are unavailable; arbitrary clipboard
text is not treated as a native note. No unfinished shape/style controls appear.

### M1.4 rotation, Cut and keyboard movement

A round rotation handle sits 24 screen points above the group box, connected by
a thin stem. Drag around the selection center; hold Shift for 15-degree snapping.
The preview outline follows the rotating group; after commit bounds are recomputed.
Page rotations stop at the last valid conservative centerline bounds; ownership
never changes. The handle may be reached in the page margin within the editor.
Corner resizing remains proportional for handwriting, including off-axis drags
and Shift; width/pressure appearance scales uniformly. Nonuniform boxes are future
content, not a handwriting stretch mode.

Cut appears alongside Copy/Paste in Edit and canvas context actions, using
Ctrl/Cmd+X. Cut removes content only after a validated session clipboard copy is
available. Arrows move selected objects 1 document point; Shift-arrows move 10.
Text input, popovers and active drags retain input priority. Escape cancels the
preview and clears selection as before. The top drawing toolbar and application
layout remain unchanged.

On web, canvas Ctrl/Cmd+A and Ctrl/Cmd+D belong to XiToc, not browser page
selection/bookmark commands. Native HTML drag initiation on the canvas is blocked
so it cannot swallow a manipulation release. Text fields keep native shortcuts,
and Copy/Cut/Paste events are not suppressed by this guard.
