# XiToc UI design system — M1.3 drawing workspace

Follow this specification for future UI changes. Keep `src/ui/style.rs` tokens
and this document synchronized. All dimensions are egui logical points. Use one
Rust/egui component tree on desktop and WASM. Original vector icons live in
`src/ui/icons.rs`; no Apple assets, icon fonts or remote images.

## Reference and hierarchy

Primary UX reference: [Apple Freeform for Mac: drawing and sketching](https://support.apple.com/guide/freeform/draw-and-sketch-frfm6e3d132d2/mac),
consulted November 2026. Adopt compact floating tools, nearby contextual options,
and direct canvas manipulation. This is an adaptation, not a pixel replica.
Apple's indexed guide text confirms bottom-of-window drawing tools, contextual
nib/color options, and object/partial erasing. Its full page/images returned
HTTP 429; detailed current macOS visual parity has not been verified.

Preserve XiToc's application menu bar, resizable real-folder Explorer, horizontal
scrolling document tabs, New Document (+) beside tabs, Settings and status bar.
Order inside the editor: tabs → compact document header → canvas. The palette
floats inside the canvas; never anchor it to the entire application window.
Document type is chosen at creation and derived from metadata, never a toggle.
Browser import/export retains truthful session/download language. Ink remains
memory-only until M2, with dirty indicators and existing discard protections.

## Colors and appearance

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| Shell | #F6F7F9 | #1B2028 | Application bar, tabs, status |
| Sidebar | #F0F2F5 | #181D24 | Explorer |
| Surface | #FFFFFF | #242B35 | Palette, active tab, popovers/dialogs |
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

## Floating drawing palette

Bottom center of the editor viewport, 16 above its bottom edge. Surface fill,
1-point Border outline, 12-point corners, 6-point inner padding, 4-point gaps.
Restrained black shadow: alpha 24/255, offset (0,3), blur 12, spread 0.
Tool buttons are 34×34; original monoline glyphs occupy 14–16 points and use
1.25-point strokes. Current three-tool palette is approximately 124×48.
Keep its position fixed; it is not draggable. Never place it over Explorer,
tabs or status. Native minimum window is 480×360; hide Explorer for more room.

Enabled sequence: Hand, Pen, Eraser. Each is a real keyboard-focusable button
with an accessible name and tooltip. Selection uses selection fill, accent icon
and outline; hover/focus use shared semantic styles. Tooltips explain settings
and Space-drag. Clicking Pen or Eraser selects it and toggles its settings.
Hand selects navigation without a settings panel. No dead/fake tool controls.

Future conceptual sequence reserves Selection, Hand, Pen, Monoline, Pencil,
Marker, Eraser, Shapes, Text, Sticky Notes. These are architecture positions,
not empty slots occupying canvas today. See `CREATIVE_TOOLS.md` for dependencies.

## Contextual settings and state ownership

Popovers anchor above their tool with natural viewport fallback/constraining;
280-point preferred width, capped to available editor width minus 24. Content
scrolls when height is limited, leaving palette accessible. Inside adjustments
stay open; outside click, Escape, or clicking the opener dismisses naturally.
They are nonmodal. Foreground hit regions are registered before canvas input;
clicking controls never starts ink or panning. Outside canvas clicks dismiss and
may begin the intended canvas gesture. Existing stroke input stays unchanged.

Pen: color swatch, inline expandable opaque color picker, thickness 0.25–12 PDF
points, smoothing strength 0–100%, pressure enable, sensitivity 0–100%, minimum
width 1–100%. Default blue #2563C8, thickness 2.5, smoothing 50%, pressure enabled
at 100% with minimum 20%. No opacity control until compositing is implemented.
Eraser: Whole-stroke (`Stroke Eraser`) or Partial-stroke (`Partial Eraser`), and
radius 4–48 logical screen points. Its two-ring cursor matches the engine radius.

Color/width/radius edit the existing active-tab canvas values. Smoothing, pressure
and eraser mode edit existing persisted DrawingPreferences, also shown in
Settings → Drawing; no mirrored palette state. Smoothing/pressure affect new
strokes across tabs, never reshape old strokes. Settings color/width/radius are
defaults for newly opened tabs. Shared drawing-control functions serve both UIs.
Mouse and devices without supported force remain constant-width; native Linux
and macOS tablets currently use that fallback. Do not claim hardware detection.

Settings remains Edit → Settings, centered 520-point preferred width with 24
outer clearance, General/Appearance/Drawing/Input Devices sections, scrollable
content, persistent preferences, Done/Escape dismissal. Underlying editor is
disabled while Settings is open. Popovers need no new modal dialogs.

## Document surfaces and controls

Page Notes: white A4 pages, 0.75 gray edge, black shadow alpha 28/255, offset
(0,3), blur 10, spread 1. Neutral Workspace surrounds paper, outer/page gap 24.
Fit available width up to 794; minimum paper width 210 with scrolling. Preserve
the page list, boundaries, per-page labels and page-local ink clipping. Multipage
creation remains #11; no new page-editing feature in M1.3. Palette stays fixed
while paper scrolls; pan/scroll can reveal content behind it.

Whiteboards: Board fill, no A4 bounds/edge/shadow. Optional low-contrast adaptive
dot grid follows the existing camera. Infinite f64 world coordinates remain
unchanged. Both types share the same palette, controller and renderer.

Header retains immutable document type, Undo/Redo and View menu (zoom out/in,
percentage, reset/fit; Whiteboard-only grid). Under 360 editor points, collapse
these actions into one typed document menu. Browser export remains disabled for
dirty ink. External PDFs show read-only/preview-unavailable; no drawing palette.

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

`app.rs` coordinates state/actions. `toolbar.rs` renders header and palette;
`drawing_controls.rs` shares settings widgets; style/icons centralize visuals.
Document model, PDF, storage, geometry, smoothing, indexes and mesh caches remain
independent. No IO, timers or unconditional repaint requests in palette drawing;
closed popovers do not construct their content. Preserve existing incremental
stroke meshes, spatial eraser indexes and large-coordinate transforms.

Verify actual running app at 1100×760 and 480×360, both themes and document types.
Capture workspace, Page, Whiteboard, Pen and Eraser popovers; confirm no overlap
with Explorer/tabs, no accidental ink from controls, drawing/erasing/history,
resize, grid and zoom. Record performed checks in TEST_PLAN, distinguish build
checks from visual verification and physical pen/display latency tests.
