# LaTeX typography and configuration (M1.13)

Formula objects retain exact user source, stable IDs, color, placement and editable
bounds. PDF saving remains unavailable until M2. No sidecars or TeX installation
are used. All math processing runs locally in Rust on native and WASM.

## Rendering

The bundled STIX Two Math preset uses the engine's OpenType MATH constants,
italic corrections and stretchy glyph variants/assemblies. Variables are math
italic; named operators such as `\sin`, `\cos` and `\lim` are upright. Greek,
fractions, roots, scripts, integrals, large operators, matrices and scalable
`\left`/`\right` delimiters share the same baseline/layout engine.

Settings → LaTeX offers the supported STIX preset, default size (6–144 pt), default
color and high-quality rendering (on). Size/color initialize future formulas;
existing objects keep their captured size/color. An unavailable stored preset
falls back to bundled STIX. Ordinary system/UI fonts are not offered as math fonts.

Original glyph outlines become self-contained SVG paths. The display rasterizer
renders these at physical resolution: canvas zoom × object scale × device pixel
ratio, rounded up to a power-of-two density bucket. High quality uses 2× linear
supersampling. Linear texture filtering handles fractional movement/rotation.
Each object/preview caches its layout and a texture; moving without changing its
resolution bucket does not parse, lay out or rasterize again. Size/zoom/DPI changes
regenerate the texture when its bucket changes. Resolution is bounded at 4096
pixels per side (or the GPU limit, if smaller) and approximately four megapixels per formula; extreme enlargement
of very large formulas can reach that limit. SVG remains resolution independent.

## Global and document settings

Edit → Settings → LaTeX edits a global draft. Apply validates before replacing the
active preferences; Reset to Defaults resets the draft and still requires Apply.
Normal preferences storage persists applied global settings across restarts when
available. Invalid input and unapplied drafts remain in the dialog.

The top toolbar's Document Settings → LaTeX opens the active document's draft
(on compact layouts, inside View). Validate checks it; Apply also validates every
existing formula and commits one shared undo operation. Cancellation preserves
the previous document configuration. Page Notes and Whiteboards have independent
settings. Document fields participate in the existing serde model, but are
memory-only in this milestone: the blank PDF encoder explicitly rejects nondefault
LaTeX configuration, and existing dirty/export/discard protections remain active.

Global configuration changes preflight all open documents before applying. They
remeasure formulas in each document's history, retaining source/placement/color;
undo in a document restores its earlier effective configuration and geometry.
This does not undo the application preference for other/new documents. A subsequent
global Apply with a changed configuration updates open documents again.

## Supported math-only preambles/postambles

This is not a full LaTeX document compiler. Configuration fragments support the
same math syntax as formulas, plus bounded custom command definitions:

```latex
% Global Preamble
\newcommand{\vect}[1]{\mathbf{#1}}
\newcommand{\R}{\mathbb{R}}
```

A document can override an existing custom command explicitly:

```latex
% Document Preamble
\renewcommand{\vect}[1]{\mathit{#1}}
\left[
```

```latex
% Document Postamble
\right]
```

Formula source can then be `\vect{x}\in\R`. Braced macro names, zero to nine
mandatory braced arguments and `#1`…`#9` substitution are supported. Optional
arguments, builtin redefinition, packages, `\documentclass`, document environments,
file access, shell commands and unsupported math commands return errors. Unsupported
commands in unused definitions also return errors. Empty defaults add no commands.

Definitions are resolved in global-preamble, document-preamble,
document-postamble, global-postamble order. Remaining math fragments wrap the
individual formula in this order:

1. Global preamble.
2. Document preamble.
3. Formula source.
4. Document postamble.
5. Global postamble.

Fragments are separated by newlines, protecting control-word/comment boundaries.
No `$`, `\begin{document}` or package commands are inserted into math mode. Stored
backslashes, line breaks and original formula source remain unchanged. `%` comments
in configuration are honored. User-authored formula source is never regenerated
from expanded source or rendered glyphs.

Limits: each configuration field 4096 bytes, 64 custom commands, expansion nesting
16, expanded formula 16 KiB/128 tokens, parser nesting 32. Recursive/oversized macros
fail with errors. Validation uses a sample `x` and existing formula sources before
application; configuration requiring a particular structural body may fail the
sample validation. The last valid preview remains visible while a formula draft
is invalid; Finish is disabled until corrected.
