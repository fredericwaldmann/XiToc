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
