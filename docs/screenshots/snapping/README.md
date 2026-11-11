# M1.12 Firefox WASM observations

Actual running local release app, captured 2026-11-11 at 1280×900 using scripted
browser pointer/keyboard events and wheel/pinch zoom. These captures do not certify
physical mouse/stylus latency or native-platform interactions.

- `left-edge-guide.png`: a moved rectangle aligns to another's left bound.
- `equal-spacing.png`: differently sized neighbors produce equal 158-point gaps.
- `align-menu.png`: all six canvas alignments and both distributions are exposed.
- `distributed.png`: five differently sized rectangles, aligned left and distributed
  vertically with equal gaps; undo/redo was also exercised.
- `hidden-grid-controls.png`: grid snapping on with objects/dots off; subsequent
  dragging verified that hiding the grid does not disable its snap mode.
- `connected-group-200.png` / `connected-group-400.png`: a two-rectangle group with
  a magnetic wire retains attachment/selection during fast moves near a reference;
  the 400% group is partly offscreen and its alignment guide stays thin.
- `alt-bypass-400.png`: holding Alt temporarily frees the proposal and hides the
  guide; releasing Alt restores the snap without changing group membership.

See [TEST_PLAN](../../TEST_PLAN.md) for the performed checks and remaining matrix.
No screenshot establishes PDF persistence; all fixture content is memory-only.
