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

Order: Selection with mode dropdown, Hand, Shapes, horizontal pen presets,
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

One Selection button and its small dropdown arrow occupy the left of the TOP
toolbar. The dropdown offers Rectangle Selection (default) and Lasso Selection.
The chosen mode remains active until changed, including after closing a lasso;
returning from Pen/Hand/Eraser recalls it. Both modes click topmost visible ink
with a 4-screen-point tolerance. Normal click replaces membership, except an
already selected object preserves the group for dragging. Shift-click adds
idempotently; Alt/Option-click removes. Alt takes precedence over Shift.
Empty click clears membership on release. Ctrl/Cmd+A selects the active scope.
Text focus and open popovers retain their shortcuts; Space temporarily pans.

Dragging empty canvas at least 3 screen points starts a rectangle. Horizontal
endpoint displacement determines policy independently of vertical direction:
left-to-right contains the complete rounded pressure envelope; right-to-left
crosses any part of it, including boundary contact. Zero horizontal displacement
uses containment; changing direction immediately updates policy. Blue solid
borders mark containment; green dashed borders mark crossing, with 10% fills.
Centralized containment/crossing tokens are #286ED2/#238C50 in Light and
#6EAAFF/#64D291 in Dark. Matching object outlines highlight while dragging. Tests and
geometry use document coordinates independently of camera pan/zoom.

Lasso draws a freeform live outline, closes on release and retains its existing
strict rounded-envelope containment (boundary contact excluded). Both area modes
capture modifiers at pointer-down: none replaces, Shift adds, Alt/Option removes.
Invalid short lasso replaces with an empty selection; an over-complex outline
retains membership and explains the 4096-vertex limit. Handles take priority over
area gestures in both modes (Shift still snaps rotation); Alt bypasses handles
for subtraction. Modified object clicks change membership without moving it;
unmodified object drags select/move the object or existing group.
Escape cancels a live gesture and clears selection. Focus loss or camera changes
cancel previews without altering membership. A later release cannot commit a
cancelled gesture. OS-reserved Alt-drag may be intercepted by window managers;
use the corresponding area gesture with Shift where available or change the OS
binding. Browser canvas guards preserve text-agent and system clipboard behavior.

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
text is not treated as a native note. No unfinished tools appear. Shapes formatting is specified below.

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


### M1.4 horizontal and vertical flipping (#33)

Right-clicking ink opens the existing context menu and activates the remembered
Selection mode if another drawing tool was active. A selected target preserves
its group; an unselected target becomes the sole selection. Right-clicking empty
canvas retains membership and never edits content by itself. Popup/text focus
and read-only documents retain their existing protections. eframe 0.37 prevents
the browser's default context menu on its canvas; other HTML is unaffected.

Both Edit and canvas context menus now share a Transform submenu containing
Rotate left 15°, Rotate right 15°, Flip Horizontally and Flip Vertically. Transform
is disabled without an editable selection. No new toolbar controls or handles.
Horizontal flip reflects about the vertical axis through the selection box center;
vertical flip reflects about its horizontal axis. A group mirrors its relative
object positions around one shared center. Axes are document-aligned regardless
of current rotation, zoom or pan. The whole operation is one undo step and keeps
selection IDs. Bounds/handles refresh immediately. Page ownership stays fixed;
if a centerline would leave the page, reject the operation and explain why.
Stroke color, positive width/scale, smoothing and pressure are preserved. Text
boxes reflect their centers while keeping glyphs readable; see Editable text boxes.


## M1.5 basic Shapes picker and vector editing

Shapes is a 26-point top-toolbar icon immediately after Hand, before pen presets.
It opens a compact four-column/two-row picker with 30-point original vector glyph
buttons: Rectangle, Rounded rectangle, Circle, Ellipse, Triangle, Diamond, Straight
line, Arrow. Normal selected-button treatment marks the remembered shape; hover
and accessibility names expose labels. Tokens: SHAPE_PICKER_ICON=30; existing
Surface/Text/Accent/Selection colors apply. No bottom controls or tab/Explorer changes.
At editor widths below 400, Hand moves into View (Space-drag remains available),
leaving Shapes/Selection/presets/Eraser accessible in the top row.

Choose a form, then drag at least 3 screen points. Preview uses its final vector
appearance; release creates one selected shape and returns to the remembered
Selection mode. Clicking without a drag creates nothing. Boxes normalize all
quadrants. Shift makes boxes square/circular; Circle always stays circular.
Shift snaps line/arrow direction to 45° increments. Escape, focus loss, camera or
tool changes cancel without a document/history edit. Page previews retain the last
valid candidate when the pointer would leave its source page.

Defaults: blue border RGB(37,99,200), 2 document points, solid; closed shapes have
fill RGB(90,150,220), opacity 35/255 (~14%). Fill None is semantically distinct
from transparent fill. Rounded corners default to min(12 pt, 20% of shorter side).
Arrowheads default to length 12 pt/width 10 pt, clamped for short segments. Widths
scale with Placement; pattern dimensions are independent logical-point values. These
are initial semantic defaults; Settings → Drawing can configure creation styling.

Click/add/subtract, directional rectangles, lasso, clipboard, move/rotate/flip and
history work for mixed ink/shape selections. Singleton boxes show local corner
handles, which also follow rotation/reflection; drag to resize axes independently,
Shift to preserve aspect ratio. Circle and group resizing remain proportional.
Singleton lines/arrows show their two endpoint handles: drag either to adjust
length/direction, Shift to constrain angle, with the other endpoint fixed. Endpoint
and corner click jitter below 3 screen points never commits a geometry edit.
Each completed creation/edit is one undo step. Shape erasing uses selection
Delete/Cut; ink erasers retain their existing behavior. No embedded text, magnetic attachments, connector routing or shape PDF saving.

Shape body drags preserve the initial pointer offset and current multi-selection.
Only visible local corners/endpoints resize a singleton shape; an invisible
axis-aligned bounding-box corner is not an edit target. Grabbing a visible handle
off-center retains that offset rather than snapping the handle to the pointer.
Movement previews follow the pointer immediately from the original gesture
snapshot, without changing intrinsic size or angle. Release commits one undo
step; a click, Escape cancellation or return to the starting position commits none.


A selected group is one persistent drag session, including fully offscreen
members. Dragging from a visible portion of an oversized shape remains valid.
Viewport edges clip graphics only: they neither clamp Whiteboard movement nor
end the session. A held pointer may leave the canvas into another application
panel and return without dropping the selection. Hover loss alone does not
cancel; explicit release finishes, while contact cancellation or focus loss
restores the original geometry without history. View/layout changes cancel safely
rather than changing the drag reference frame; navigation remains blocked during
Whiteboard drags. Actual Page Note bounds still constrain movement independently
of the viewport. No smoothing, animation or auto-pan is added.


## M1.5 shape styling and formatting

Selection retains the nearby contextual toolbar. Shape selections add Fill (closed
forms), Border or Line, Width, Style, Opacity, Arrowheads (segments), and More.
Fill provides RGB color, 0–100% opacity and an explicit enabled/no-fill choice;
Border/Line provides independent RGB/opacity and enabled/no-border choice. Width
is 0.1–24 logical document points. Solid, dashed and dotted patterns share the
same width infrastructure. Dash/gap and dot dimensions are independent logical-point settings (see the
border correction below). Placement and zoom scale these together;
DPI affects tessellation only. Closed fills composite in canonical object order.

Arrowheads offers None, Start, End and Both ends, also for straight lines.
Filled heads remain solid when the shaft is dashed/dotted and follow directed
endpoints through rotation, reflection, resizing and endpoint editing. More
includes No fill, No border/line and rounded rectangles' Corner radius. Radius is
nonnegative and clamps to half the shorter local side; later shrinking safely
clamps the displayed radius without invalid geometry.

Mixed compatible selections show “Mixed values”; the first compatible value seeds
the editor, and changing a property applies just that property to all compatible
shapes. Fill skips segments, radius skips other forms, arrowheads skip closed
shapes, and ink is untouched. A batch is one undo operation; continuous pointer
adjustments preview immediately and coalesce until release. New adjustments remain
separate undo steps. Unselected objects and application defaults do not change.

Right-click → Format exposes these same menus and commands. Interactive menus use
CloseOnClickOutside; color selection expands inline to preserve the parent menu. Existing
selection-group right-click and Transform behavior remains. Widgets retain native
egui keyboard focus, picker interaction and light/dark styling.

The toolbar uses Surface/Border tokens, six-point corners and existing compact
spacing. FORMAT_TOOLBAR_WIDTH=480 is a maximum, clamped to available editor width;
controls wrap in small editors. FORMAT_TOOLBAR_GAP=12 and the existing rotation
clearance place it above the handles when possible, otherwise below the selection,
constrained to the visible editor. Oversized selections may leave no completely
unoccupied space; the editor constraint takes precedence. No drawing tools move
from the top, and Explorer/tabs remain unchanged.

Settings → Drawing → Default shape style configures new objects with these same
widgets, including new-arrow heads and rounded radius. Defaults persist through
the existing local application preferences backend. Unconfigured radius preserves
adaptive min(12 pt, 20% of shorter side) creation behavior. A configured radius
clamps to each new shape's bounds. Object edits never implicitly set defaults.


### Shape border and color corrections

Style → Dashed exposes Dash length and Gap length; Dotted exposes Dot spacing
(center-to-center) and Dot size (diameter). Solid exposes neither. Values are
0.25–256 logical document points, independent of the 0.1–24 pt border thickness.
Defaults are dash 8, gap 4, spacing 5, diameter 2 pt. Each shape captures its own
settings; defaults in Settings use the same controls. Placement/group scaling
scales appearance uniformly, while camera zoom/DPI never rewrites properties.
If spacing is reduced below diameter, diameter clamps; increasing diameter beyond
spacing raises spacing. Thickness never changes any pattern setting.

Closed dashed contours fit round(perimeter / requested period) equal periods
(at least one), scaling all dash/gap lengths by the same factor to retain their
ratio. The closing seam lies at the center of a full gap. Open strokes retain
requested lengths, clipping only the final dash. Dashes crossing a corner remain
one joined polyline. Closed dots use floor(perimeter / spacing) uniformly spaced
centers; if chords at sharp corners/opposite sides would overlap circles, reduce
the count globally while retaining even arclength distribution. No duplicate seam
point. Pattern output is bounded to 8192 marks on extreme geometry; fitting that
limit increases effective spacing rather than allocating unbounded geometry.

Borders have explicit cyclic closure. Joins use a miter limit of two half-widths,
then bounded bevels at sharper turns; they do not leave disconnected caps at
corners. Curves use a stable 0.01-world-point contour tolerance (existing sample
caps remain), so pattern phase/count does not depend on view zoom or pixel density.
Head triangles remain solid, independent of shaft pattern.

Fill and Border/Line show a transparency-aware color swatch, Color picker toggle,
Hex RGB field, and separate opacity slider. Clicking either the swatch or toggle
expands egui's RGB/HSV picker inline in the same menu, avoiding competing root
popups. The preview includes the object's actual alpha; RGB edits preserve that
alpha. Hex accepts #RRGGBB or RRGGBB and commits valid input on Enter/focus loss;
invalid input never changes semantic color. Picking a fill color enables fill;
editing a disabled border retains its disabled state. Mixed compatible selections
reuse the same color commands and shared batch history.


## M1.6 magnetic connection interaction

Selection shows four circled-plus ports on each selected closed shape, nine
screen points beyond its top/right/bottom/left border anchor. They rotate/reflect
with intrinsic shape coordinates; four-point radius circles distinguish them
from corner resize handles and the plain rotation ring. Hit radius is seven
screen points at every zoom. Source ports may occupy page margins within the
editor clip; actual connector graphics/endpoints still obey page bounds. Tokens:
CONNECTOR_RADIUS=4, CONNECTOR_HIT=7, CONNECTOR_OFFSET=9, CONNECTOR_SNAP=16. Existing
Surface/Accent colors apply in both themes; hovering a port uses a crosshair.
No ports on unselected shapes except target feedback during an active connection.

Press/drag a port to preview a straight connector. Compatible nearby targets
highlight their border and cardinal anchors; the preview snaps to the closest
port. Release commits once; release in empty space creates a free destination.
Returning into the source cancels new insertion; self attachment is forbidden.
Escape, focus loss/contact cancellation, tool/view/revision changes cancel without
editing history. Hover loss alone retains ownership; held gestures may leave the
viewport and return. Page endpoints clamp to actual page bounds, never merely to
visible scroll bounds. Touch uses the primary pointer gesture; no hover is needed.

Selected connector endpoints have plain rings. Drag either to reconnect to another
shape/port or detach into free space, with the initial grab offset preserved.
Click-only/no-op/cancelled interactions do not enter history. Shape bodies win
picking over connector shafts at overlaps; selected connector endpoints deliberately
retain editing priority. Shift/Alt, directional rectangle and strict lasso retain
the shared rules. Moving/rotating/resizing a selected connector without its targets
detaches those ends; selecting targets with it preserves each attachment once.

For keyboard creation, select two closed shapes and press Ctrl/Cmd+Shift+K, or
choose Connect selected shapes in the canvas context menu. Canonical order chooses
source/destination, with the nearest border-port pair. Tab/Enter can activate the
context action. This supplies a keyboard alternative to small ports, without a
new drawing tool or top-toolbar rearrangement. Keyboard/text focus keeps priority.

New connectors inherit current common shape-stroke defaults and a destination
arrowhead. The existing Line/Width/Style/Opacity/Arrowheads/More contextual menus
and right-click Format edit their color, thickness, alpha, solid/dashed/dotted,
custom dash/gap/dot settings and independent heads. Fill/radius are incompatible
and remain hidden. Edits/continuous sliders share the same batch undo policy.
Deleting a connected node retains its wires as free ends at their last locations;
Undo restores the node and attachments together. No routing/label/text/PDF-save
controls appear. Explorer, tabs and the TOP drawing toolbar remain unchanged.

## Advanced connector formatting and route handles

Selected connectors add Routing to the existing contextual formatting controls
and right-click Format menu. Choices are Straight, Orthogonal and Curved, with
separate Automatic source/target anchor checkboxes and Reset Route. Mixed routing
is explicit, and compatible multi-selections change in one transaction. Existing
line/color/opacity/pattern/head controls remain shared. The main drawing toolbar
stays directly below tabs; Explorer and tabs are unchanged.

Explicitly dragged shape ports stay fixed. Snapping onto a shape body away from
its ports enables automatic attachment; Routing can toggle either endpoint later.
Automatic sides use a stable dead band; local ports follow rotation/reflection.
Selected endpoints retain round rings. Route controls are outlined diamonds,
visually distinct from round endpoints and resize corners. Curves show two control
links and diamonds; orthogonal intermediate segments show draggable midpoint
diamonds, with an aligned segment handle available before bends exist. Handles
remain screen-sized using the existing 7-point hit radius.

Dragging a diamond previews immediately, retains attachments and commits once on
release. Escape/focus loss cancels. Curve edits store offsets relative to endpoints;
orthogonal edits move a rail relative to the endpoint midpoint. Node movement
preserves those edits; Reset Route returns the current mode to automatic geometry.
Shape bodies still take selection priority over crossing connectors. Geometry-aware
connector clicks, Shift/Alt membership, directional rectangle and lasso retain
the same interaction rules across all modes.

The connector formatting content budget is 640 points within the existing
480-point toolbar viewport; wrapping reserves additional vertical clearance so
Routing and shared style controls do not cover rotation handles.

Popover placement includes route-control positions outside the curve envelope,
so an off-curve Bézier diamond remains clickable when viewport space permits.

## Standalone rich text: Text | Paragraph

The T tool stays directly below document tabs in the TOP drawing toolbar. Click
or width-drag creates and focuses text. Escape finishes editing and preserves
object selection; double-click re-enters the same object. Rotated objects keep the
existing labeled unrotated editing overlay. Empty drafts are discarded.

Contextual/right-click formatting contains exactly **Text | Paragraph**. There is
no Box section, background, border, corner radius, padding or container opacity.
Standalone text has invisible wrapping/resizing bounds; only editing/selection
outlines and eight resize handles expose those bounds. Decorative shape styling
belongs to future Shape + Text, separate from this tool.

Text shows family, directly editable numeric size with −/+, a size slider,
bold/italic/underline, and the shared RGB/HSV/hex text color with text opacity.
Selected characters retain their exact range and direction while using controls.
Property patches preserve unrelated mixed formatting. With a caret only, changes
set future typing style and leave existing characters unchanged. Mixed indicators
remain visible. Paragraph exposes Left/Center/Right/Justify, line spacing and
paragraph spacing for the caret paragraph or all intersecting selected paragraphs;
these controls never patch character styles. A separate layout choice inside
Paragraph selects Reflow or Scale for the individual text object, without creating
a third tab. Settings → Text still sets defaults captured by new objects.

A text session owns its retained Unicode character selection; egui's widget cursor
is an adapter. Formatting focus never discards or collapses that range. During
popups/font browsing, editing input is suspended and an inactive selection highlight
uses the same laid-out glyph geometry and the Selection token at 65% opacity
(`INACTIVE_TEXT_SELECTION_OPACITY`). Closing controls restores editor focus and
the same selection, allowing font → size → bold → color without reselecting.
The highlight is transient; it is never canonical text content or object selection.

The searchable Font browser highlights the current family. Up/Down preview only
the retained text range; Enter/mouse choice commits one operation, while Escape
cancels without history changes. Preview arrows do not move the caret or objects.
Visible font names use their own fonts with lazy cached loading. Native lists
installed supported families; web lists bundled families and states the limitation.
For text-only object selections, contextual −/+ scale every stored font run by
reciprocal 0.9/1.1 factors instead of scaling object geometry. Other selections
retain geometric scaling. Mixed-size Text controls show `Mixed` and omit the slider
until the selection has one shared size; the mixed entry changes content only after
the user enters a valid size. Opening or switching Text/Paragraph is read-only.

Justify expands breakable word gaps on eligible wrapped rows to reach both margins.
It does not stretch letters, single words or paragraph-final rows. Explicit newlines
end paragraphs. Mixed fonts/sizes remain editable, with matching caret/selection
geometry after width changes and zoom. Scripts without inter-word gaps retain
natural spacing; no extra CJK inter-character justification is claimed. Full RTL/
bidirectional layout remains an egui text-engine limitation.

All eight local corner/edge grips retain captured-source drag behavior, rotation,
zoom independence and one release transaction. Reflow changes wrapping/container
bounds while preserving font sizes; Scale corners preserve aspect ratio and mixed
size proportions, while edge grips always reflow. Changing a default/mode does not
itself resize existing content. No bottom drawing toolbar is introduced.

Native/browser IME and clipboard continue through egui; XiToc session copies retain
rich runs, while external text paste is plain text. Both layouts retain dirty/discard
protections and page overflow scrolling. No Shape + Text or PDF persistence is added.

## M1.9 Shape + Text

Double-clicking a supported closed shape enters the shared rich-text editor;
single click and drag remain object selection/manipulation. The same top
contextual Text | Paragraph controls format retained character selections and
paragraphs. Shape styling remains in its existing shape controls, clearly
separate from text color and paragraph alignment. Paragraph offers horizontal
alignment; a shape-text-only vertical alignment control offers Top, Middle, and
Bottom, with centered/middle defaults. Because it positions the complete
container layout, those controls are enabled only at a caret, for empty text,
or when all text is selected; a partial text selection remains selected and
shows guidance that the whole text must be selected or the selection cleared.
Text uses a conservative geometry-aware
interior region; overflow is clipped with a visible marker while all content
remains editable. The existing text editing overlay stays unrotated while the
shape's rendered text follows rotation and remains readable when flipped.

## M1.10 Sticky Notes

Sticky Note appears in the TOP drawing toolbar when the responsive toolbar can
fit its control. Click the canvas to create a 220×180 light-yellow note and
begin editing immediately. Notes reuse the TextSession and keep Text | Paragraph
formatting separate from their Note appearance menu. That menu offers the
Yellow/Pink/Blue/Green/Purple/Orange/White palette, custom color, subtle shadow,
and automatic height. Drawing Settings stores the default color for new notes;
existing notes keep their fill. New notes choose a readable typing color for a
dark custom background, and Note warns when existing rich text has low contrast.
Manual resizing through any of eight rotated local grips disables auto-height;
the user can re-enable it. Overflow is marked without removing text. Notes retain
normal shape selection, rotation, flipping, duplication and connectors. No
floating toolbar, separate text object, or PDF persistence is introduced.

## M1.11 organization interactions

Ctrl/Cmd+G groups two or more selected immediate siblings on the same surface;
Ctrl/Cmd+Shift+G ungroups selected containers and selects their immediate unlocked
children. These commands appear in Edit and canvas context menus, disabled when
inapplicable. Existing right-click selection preserves a selected multi-selection.
Lock and Arrange → Bring to Front / Bring Forward / Send Backward / Send to Back
use the same menus; Transform retains rotation and both flips. No toolbar moves.

Single-click selects a group through actual child geometry. Double-click enters
one group level without starting text editing. Within that scope another
double-click edits a text/shape/note host or enters a nested group. The canvas
shows “Editing group … Esc to exit” using existing palette/body typography and
spacing. Escape closes text editing first; subsequent Escape exits one group
level and selects its container. No transforms occur on entry/exit. Select All,
Shift/Alt clicks, directional rectangle and lasso use that active sibling scope.
Crossing matches any descendant; containment/lasso require all descendant leaves.

Locked objects stay visible and ordinary selection passes through them. Right-click
on their geometry exposes Unlock for the outermost locked ancestor, even when
unlocked objects are selected elsewhere. Unlock never clears independent child
locks. Parent transforms/deletion are blocked if a descendant is independently
locked; enter the group and unlock that child first. Ungroup retains child locks.
Ink erasing respects the active group context and effective locks; pen drawing is
unaffected. Duplicate preserves locks. Lock flags remain memory-only pending M2.

Groups form contiguous stacking blocks. Group creation places the block at its
highest selected sibling; Arrange changes only immediate siblings in the current
context. Multi-selection retains relative order. Ungroup leaves nested containers
and effective visible order intact. Rendering and picking now both use canonical
stacking order for all variants, including connectors crossing text/shapes.
Canvas-only browser default suppression includes G and Shift+G; text input keeps
native shortcuts. Existing top toolbar, themes, tabs and Explorer are unchanged.

## M1.12 snapping and canvas alignment

The TOP toolbar exposes Snap at editor widths ≥650 points; narrower layouts put
Snap inside View. Menus use the existing CloseOnClickOutside behavior. Checkboxes:
Snap to Objects, Show Alignment Guides, Snap to Grid and Show Grid. Snapping
Settings exposes sensitivity (Low 4 / Normal 8 / High 12 logical screen points)
and grid spacing (1–4096 document points). Defaults: objects/guides/grid display
on, grid snapping off, 32-point grid. Settings → Snapping edits separately persisted
defaults for newly opened editors; the toolbar changes the active session only.
The grid is the existing subtle dot style on both document types. Hidden dots
still permit snapping; low-zoom dot thinning does not change the configured grid.

Edit and right-click → Align Objects exposes Left, Center Horizontally, Right,
Top, Middle Vertically, Bottom and horizontal/vertical distribution. At least two
eligible objects are required for alignment and three for distribution. These
are canvas commands, distinct from rich-text Paragraph alignment. Collective
eligible bounds are the alignment reference; distribution keeps the outer objects
fixed and equalizes actual gaps without resizing. Insufficient space is reported
without moving anything. Locked objects/groups with locked descendants cannot
move and never unlock implicitly; locked siblings may be snap references.

Normal selection treats a group as one unit; entering a group restricts candidates
and commands to that sibling scope. Derived connector paths do not determine
rectangular alignment bounds, including when grouped. External magnetic endpoints
stay attached during group movement. One drag is still one history command.

Hold Alt during an existing move/resize to bypass snapping temporarily; Alt-click
still subtracts membership. Screen tolerance divides by zoom, release tolerance is
1.5× acquisition tolerance, and pointer displacement always uses the original
press. No animation or drag-origin reset. Resize follows existing anchors/aspect
policies; rotated objects snap the active world-space grip, while proportional
resizing accepts only achievable corrections without amplified rail jumps.

Guides use the existing theme Accent and centralized SNAP_GUIDE_WIDTH=1 logical
screen point. At most one relevant guide per axis appears, with small brackets
and document-point gap numbers for equal spacing. They are painter overlays with
no input allocation or canonical objects, and disappear on release/cancellation.
Explorer, horizontal tabs, drawing toolbar placement and light/dark styles remain.

## M1.13 formula editing

Formula (∑) lives with the drawing tools in the TOP toolbar. Choose it and click the
canvas to open the source editor for a new formula. The multiline field updates its
math preview as source changes. Finish or Ctrl/Cmd+Enter commits; Cancel or Escape
discards a new formula or restores an existing formula's prior committed source.
Double-click a selected formula to edit it. The source editor owns keyboard focus,
so caret movement and text copy/paste remain normal editor operations. Invalid or
incomplete source stays visible with an error, disables Finish, and leaves the last
valid preview visible with an invalid marker. A normal single click continues to
select/move the formula; its contextual Formula menu offers Edit LaTeX, Reset Size
and the shared color picker. Formula backgrounds are transparent.

The standalone object preserves exact source and scales uniformly with resize
handles. Flip keeps mathematical glyphs readable while reflecting their position;
rotation remains available through the existing Transform menu. Grouping, locks,
z-order, alignment and snapping use the formula's measured bounds. Inline math in
rich text and formula PDF persistence are not part of this milestone.

### M1.13 LaTeX settings

Settings adds a LaTeX tab with a supported Computer Modern (New Computer Modern Math Regular) preset selector, creation
size/color defaults, high-quality checkbox, Global Preamble/Postamble code editors,
Reset to Defaults and Apply. This tab uses explicit Apply, unlike automatic
preferences elsewhere; its subtitle states that distinction. Validation errors
retain drafts. Code editors preserve backslashes/newlines and normal copy/paste.
All content uses existing theme/spacing/dialog-width tokens and vertical scrolling.

Document Settings → LaTeX sits with view/document controls in the TOP toolbar and
moves inside View on compact layouts. Both layouts share a per-document modal with
Global configuration: Enabled, preamble/postamble code fields, Validate, Apply and
Cancel/Escape. Applying updates one document and one undo entry. The UI identifies
the math-only supported subset and memory-only document lifecycle. No bottom tools,
placeholder fonts, package loader or PDF-save controls are introduced.
