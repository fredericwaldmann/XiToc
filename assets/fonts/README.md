# Portable text fonts

Unmodified DejaVu Sans and Sans Mono regular, bold, oblique and bold-oblique
faces are bundled for editable canvas text. Droid Sans Fallback supplies CJK
coverage after egui’s existing proportional/emoji fallback. Native and WASM use
these same embedded bytes; no system font discovery or runtime download occurs.
See LICENSE-DejaVu.txt and LICENSE-Droid.txt for redistribution terms.

The font files were copied from the development system’s fonts-dejavu-core and
fonts-droid-fallback packages. Future PDF font embedding/subsetting remains M2
work; bundling editor fonts does not implement PDF text export.

Formula objects use the unmodified `NewCMMath-Regular.otf` (New Computer Modern
Math Regular), providing Computer Modern typography with OpenType MATH metrics.
The font is embedded locally for native and WASM builds, without system font
installation. Downloaded from the [official CTAN package](https://ctan.org/pkg/newcomputermodern)
(font name-table version 4.0; README package version 8.1.1; retrieved 2026-11-13). Original font URL:
https://mirrors.ctan.org/fonts/newcomputermodern/otf/NewCMMath-Regular.otf
SHA-256: `d66ac1cc91c55c24d3636ae2df1238076debdff51841f9893fc5419cc2df3df7`.

This math face is under the GUST Font License, based on LPPL 1.3c or later.
See `assets/licenses/GUST-FONT-LICENSE.txt`, `LPPL-1.3c.txt`, the upstream package's
`NewComputerModern-License.txt` mapping, and `README-NewComputerModern.txt`.
The package's GPL exceptions apply to other faces that XiToc does not bundle.
STIX notices remain for the rendering dependency's bundled resource/attribution;
STIX is no longer the selected math font.
