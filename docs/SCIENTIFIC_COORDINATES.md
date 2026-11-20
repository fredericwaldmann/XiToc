# Scientific coordinate systems

Use **Insert → Coordinate System** and click the canvas to place one of these
presets.

| Preset | Supported data |
| --- | --- |
| 1D Positive | A positive number line, points and intervals |
| 1D Signed | A negative and positive number line, points and intervals |
| 2D First Quadrant | Positive X/Y axes and multiple curves |
| 2D Four Quadrants | Signed X/Y axes and multiple curves |
| 3D Positive Octant | Orthographic X/Y/Z axes and surface wireframes |

Double-click the body or choose **Configure Coordinate System…**. One dialog
contains a live preview, preset, axis ranges, arrowheads, colors, widths,
visibility, labels, grid, ticks, and the relevant data controls. Apply commits
one undoable edit. Cancel or Escape discards the draft. Preset changes keep
curves, surfaces and 1D data in their respective lists. Switching between
signed and positive axes adjusts the lower bound to fit the chosen preset.

The **Show Numeric Labels** and **Show 0 Label** checkboxes are independent.
Numeric labels default off and the zero label defaults on, so the default is
one zero at the origin and no other numbers. The zero label follows coordinate
axis tick placement (the TikZ source records equivalent `xtick={0}` and
`xticklabels={$0$}` semantics); it is not a floating node and does not require
ordinary tick marks. In 2D, zero is labeled once on X. X/Y units have equal
physical scale by default. Turn that off to independently fit both ranges to
the frame. The shared resize handles scale the diagram proportionally. Use the
frame width and height in the dialog to change its aspect ratio without
scaling typography. Canvas rotation and the 3D viewing angles are independent.

Axis labels, zero and numeric tick labels use the same bundled Latin Modern
Math rendering path as XiToc equations. Labels retain authored math source,
including Greek characters, scripts and superscripts. Grid and ticks follow
the mathematical axes. Dense ticks are thinned to bound rendering. The octant
uses an optional XY reference grid.

## Expressions and domains

Enter an expression such as `x^2`, an assignment such as `y=sin(x)`, or a
signature such as `f(x)=2*x+1`. In 3D the independent variables are `x` and
`y` and the output is `z`. Up to eight plots per dimension have individual
visibility, RGB color, width, solid/dashed/dotted style and optional domain.
Custom domains are intersected with the displayed ranges.

- Use `+`, `-`, `*`, `/`, `^` and parentheses. Multiplication is explicit.
- Numeric literals may use scientific notation. Constants are `pi` (or `π`)
  and `e`. Trigonometric functions take radians.
- Functions include `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`,
  `cosh`, `tanh`, `sqrt`, `abs`, `exp`, `ln`, `log` (natural logarithm),
  `log10`, `floor` and `ceil`.
- Input and parser complexity are bounded. Invalid syntax keeps the draft
  intact and reports an error. The grammar evaluates math without executing
  arbitrary code.

2D curves are adaptively sampled and clipped before projection. Undefined
values and segments that remain unresolved near a discontinuity are split.
Finite numerical sampling can miss features narrower than its sample budget.
3D evaluates bounded families of X and Y wires and clips them to the positive
octant, including the Z range. Wireframes do not implement hidden-surface
removal or lighting.

The renderer caches source samples separately from the projected vector
geometry. Viewpoint and style changes can reuse the samples. Object panning
and rotation reuse both caches. No external TeX installation or new package
dependency is required. PDF persistence remains M2 work.

## Configuration dialog and TikZ source

The resizable **Coordinate System** dialog opens at a compact 1000×360 logical
size with a 760×320 minimum. It remains user-resizable, and egui remembers its
size for the session. General settings and TikZ source scroll within the left
pane, while the preview stays visible on the right. **General**,
**Functions**, and **TikZ Editor** tabs and their vertical scrolling stay on the
left. The right column remains a live preview rendered through the same vector
renderer as the canvas. Apply writes configuration and source in one journal
operation; Cancel and Escape discard the draft.

TikZ Editor is an editable multiline monospace field. Its source is retained on
the coordinate object and copied with that object. XiToc does not invoke TeX.
The parser accepts a single `tikzpicture`, bounded to 32 KiB and 512 commands.
It supports `\draw` horizontal and vertical axes, axis labels as simple math
text, known colors or `#RRGGBB`, `line width`, arrow options `->`, `open`, `-`,
grid commands with `step`/`opacity`, plot commands in the form
`plot (\x, {expression})` with a `domain`, and solid/dashed/dotted style.
Legacy `\node` origin labels remain readable for older source, but new source
never creates an origin node: `% xitoc xtick={0}`,
`% xitoc xticklabels={$0$}` and `% xitoc yticklabels={}` represent the single
zero label through the native axis-tick model. `% xitoc numbers=true|false`
controls nonzero labels. A small `% xitoc key=value` extension records
viewpoint, ranges, tick/grid styling, frame-independent settings, 1D endpoint
metadata and 3D surface definitions that ordinary 2D TikZ commands cannot
represent. Surface expressions use the same bounded native math grammar.

Editing a supported source command reparses into the structured `Config`, so
the preview, Functions/General controls, and committed canvas object use the
same values. General or Functions changes regenerate managed source while
retaining user comments. Tab switches alone do not regenerate or overwrite
source. Unknown commands, unsupported colors, incomplete environments,
invalid values and excessive input are rejected with line-level feedback; the
text remains editable, Apply is disabled, and the preview keeps the last valid
configuration. Correct the source or Cancel. Arbitrary macros, `\input`,
external resources, scripts, shell escape, PGFPlots and full TikZ compatibility
are not supported. 3D surfaces use the XiToc extension because the preview uses
its bounded orthographic wireframe renderer rather than TeX. Tick comments are
part of XiToc's safe TikZ subset and do not claim PGFPlots compilation support.
