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

The origin is labeled once. Other numbers are opt-in. X/Y units have equal
physical scale by default. Turn that off to independently fit both ranges to
the frame. The shared resize handles scale the diagram proportionally. Use
the frame width and height in the dialog to change its aspect ratio without
scaling typography. Canvas rotation and the 3D viewing angles are independent.

Custom axis labels are italic text. Standalone Greek TeX names such as
`\omega` and `\theta` render as their Unicode counterparts. Labels retain
the authored text in the model. Grid and ticks follow the mathematical axes.
Dense ticks are thinned to bound rendering. The octant uses an optional XY
reference grid.

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
dependency is required. TikZ/PGFPlots import and export are outside this
implementation. PDF persistence remains M2 work.
