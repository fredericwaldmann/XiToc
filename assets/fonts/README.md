# Portable text fonts

Unmodified DejaVu Sans and Sans Mono regular, bold, oblique and bold-oblique
faces are bundled for editable canvas text. Droid Sans Fallback supplies CJK
coverage after egui’s existing proportional/emoji fallback. Native and WASM use
these same embedded bytes; no system font discovery or runtime download occurs.
See LICENSE-DejaVu.txt and LICENSE-Droid.txt for redistribution terms.

The font files were copied from the development system’s fonts-dejavu-core and
fonts-droid-fallback packages. Future PDF font embedding/subsetting remains M2
work; bundling editor fonts does not implement PDF text export.

Formula objects use the unmodified `latinmodern-math.otf` (Latin Modern Math
1.959), providing OpenType MATH metrics and original glyph outlines. The font is
embedded locally for native and WASM builds, without system font installation.
Downloaded from the [official CTAN package](https://ctan.org/pkg/lm-math) on
2026-11-13: https://mirrors.ctan.org/fonts/lm-math/opentype/latinmodern-math.otf
SHA-256: `6075562b771f8b82f0c179e363389684f2dd09de30038269e2628e504bd7be0f`.

Copyright 2012–2014 B. Jackowski, P. Strzelczyk and P. Pianowski (on behalf of
TeX Users Groups). Distributed under the GUST Font License, based on LPPL 1.3c
or later. See `assets/licenses/GUST-FONT-LICENSE.txt`, `LPPL-1.3c.txt` and the
upstream attribution in `README-LatinModernMath.txt`. The previous New Computer
Modern font and its package-specific notices have been removed. STIX notices
remain for the rendering dependency's bundled resource/attribution; STIX is not
selected for formulas.
