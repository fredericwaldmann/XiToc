# M0.6 browser review

Real Firefox/WebGL screenshots of the Trunk release build served under `/XiToc/`
on 2026-10-27. These are blank documents created through the browser UI, not mocks.

- `m06-page-light.png`: Page Note and Explorer, 800×1000 browser window.
- `m06-whiteboard-light.png`: Whiteboard and both document tabs, 1100×800.
- `m06-whiteboard-dark.png`: Dark theme, compact toolbar menus, 800×600.
- `m06-narrow.png`: Narrow browser with document controls in an overflow menu.

Observed: creation dialogs open both kinds; canceled export does not claim disk
save; typed tabs switch/close; session Explorer reopens a closed document; zoom,
Reset view, grid toggle and theme selection work. Page is white with neutral
surroundings; Whiteboard has dots and no page edge. Browser resize keeps panels
separate. Tabs intentionally scroll horizontally and long status text truncates;
full text is available through the status details button/tooltip.

Automated egui layout regression additionally checks 320/480/800/1200 logical
widths, both themes and 1×/1.5×/2× scaling. This is not physical high-DPI display
verification. Native graphical startup, native folder-picker interaction and
Windows/macOS visual checks have not been performed for M0.6. Existing native
filesystem creation/reopening is covered by Rust integration-style tests. Actual
browser download completion is not asserted by this review.

# M1 handwriting review

Actual Firefox/WebGL captures from the M1 Trunk release build, served at `/XiToc/`
on 2026-10-27 (1100×800 browser window; canvas 1100×714):

- `m1-page-ink.png`: blue freehand strokes and dots on A4, dirty tab/status and
  disabled dirty-document PDF export.
- `m1-whiteboard-ink.png`: borderless Whiteboard with a changed opaque pen color
  and thicker strokes, using the same input/rendering engine.
- `m1-discard-warning.png`: explicit warning before discarding in-memory ink.

Observed in the running app: both creation flows; several pen strokes on each
kind; pen width and inline color adjustment; whole-stroke erasing and keyboard
undo/redo; A4 zoom followed by reset; Whiteboard pan by 100×40 logical points,
zoom followed by reset; dirty status and export blocking; dirty-tab warning and
Escape preserving the drawing; Explorer/tab navigation retaining separate ink.
The browser dirty flag was true and dispatching a cancelable beforeunload event
requested a warning. This tests the handler, not every browser's native prompt
policy or forced shutdown behavior.

No native graphical/OS-dialog testing or physical stylus/high-DPI hardware testing
was performed. Native Linux compilation, simulated scaling tests and GitHub CI
cover build compatibility separately; they do not prove hardware latency or
platform startup. M1 drawings are deliberately not saved/reloaded from PDF.

## Archived M1.3 bottom-palette captures (superseded)

Firefox 157 / WebGL2 running the Trunk release build on Linux, November 2026.
These are app screenshots, not mockups. The browser uses explicit import/export
fallback, so Explorer contains session PDFs rather than a permissioned disk tree.

- [Main workspace, dark, with ink](m1.3/main-dark.png)
- [A4 Page Note](m1.3/page.png)
- [Whiteboard](m1.3/whiteboard.png)
- [Pen settings](m1.3/pen.png)
- [Eraser settings](m1.3/eraser.png)
- [480×360 light](m1.3/small.png) / [dark](m1.3/small-dark.png)
- [480×360 scrollable Pen settings](m1.3/small-pen.png)

Native Linux executable launched successfully; native screenshot capture was
unavailable in this environment. Windows/macOS and physical pen checks remain
manual. No comparison screenshot from Apple's application is included.

## M1.3 top toolbar and presets — current design

Actual Firefox 157 / WebGL2 Trunk-release application screenshots, Linux,
November 2, 2026. These supersede the bottom-toolbar captures above.

- [A4 Page Note, dark](m1.3-presets/page.png)
- [Whiteboard with Monoline ink and translucent highlighting, light](m1.3-presets/whiteboard.png)
- [Preset settings, dark](m1.3-presets/settings-dark.png)
- [New custom preset opens its editor, light](m1.3-presets/new-preset.png)
- [Reorder/delete/restore controls, dark](m1.3-presets/management-dark.png)
- [Inline color picker](m1.3-presets/color-picker.png)
- [480×360 top toolbar](m1.3-presets/small.png)
- [480×360 scrollable preset popover](m1.3-presets/small-popover.png)

The toolbar remains below tabs in every capture. Only the contextual popover
overlays content. Browser Explorer shows session imports/exports; native real
folder behavior remains unchanged. The native Linux executable also launched
without startup errors; physical stylus and Windows/macOS runtime remain manual.

## M1.4 universal selection and lasso

Actual Firefox 157 / WebGL2 release application, Linux, November 2, 2026.

- [Single selected stroke](m1.4/single.png)
- [Live lasso](m1.4/lasso.png) and [completed multi-selection](m1.4/group.png)
- [Movement preview](m1.4/preview.png) and [proportional resize](m1.4/resized.png)
- [Cross-document paste into A4 Page Note](m1.4/page.png)
- [Duplicate](m1.4/duplicate.png) and [delete followed by undo](m1.4/undo.png)
- [480×360 Page Note selection](m1.4/small.png)

Verified top toolbar, Explorer and tabs remain separate. Selection, live lasso,
movement, resizing, duplicate, delete, undo/redo, Edit/context menus and cross-tab
copy/paste were exercised in the browser. Both light and dark appearances were
inspected. Native Linux launched without startup errors; interactive native
selection and system clipboard behavior still require manual verification.

## Unified Selection (#34)

`unified-selection/` records the release WASM app in Firefox at 1100×760:
`dropdown.png` shows the one top Selection button and mode menu;
`containment.png` and `crossing.png` show live solid-blue/dashed-green rectangles
around the same two strokes; `lasso.png` shows completed strict lasso selection
with the mode still active and existing transform handles. Automated BiDi pointer
input drove these captures. See TEST_PLAN for verification limits.


## Object flipping (#33)

`object-flipping/` records Firefox release WASM at 1100×760. `before.png` is the
selected asymmetric pair; `transform.png` shows the nested menu and preserved group;
`horizontal.png` reflects both positions/shapes, `undo.png` restores them, and
`vertical.png` shows vertical reflection after horizontal/redo. BiDi mouse/keyboard
input drove the app. TEST_PLAN records exact scope and platform limitations.


## Basic shapes (#24 creation slice)

`basic-shapes/` captures the release WASM app in Firefox using real BiDi pointer
and keyboard input: four-column picker, all eight forms, arrow endpoint edit,
rectangle corner edit, group duplication, dark appearance, narrow 480×360 picker
and Page Note geometry. Captured styles are the semantic defaults; no formatting
UI or text is implemented. See TEST_PLAN for performed checks and device limits.

M1.11 actual Firefox grouping/locking/child-text observations are indexed in
[grouping/README.md](grouping/README.md); see TEST_PLAN for the tested subset.

M1.12 actual Firefox snapping, equal spacing, alignment, distribution, hidden grid
and connected-group zoom observations: [snapping/README.md](snapping/README.md).
