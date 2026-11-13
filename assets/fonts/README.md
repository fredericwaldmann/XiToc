# Portable text fonts

Unmodified DejaVu Sans and Sans Mono regular, bold, oblique and bold-oblique
faces are bundled for editable canvas text. Droid Sans Fallback supplies CJK
coverage after egui’s existing proportional/emoji fallback. Native and WASM use
these same embedded bytes; no system font discovery or runtime download occurs.
See LICENSE-DejaVu.txt and LICENSE-Droid.txt for redistribution terms.

The font files were copied from the development system’s fonts-dejavu-core and
fonts-droid-fallback packages. Future PDF font embedding/subsetting remains M2
work; bundling editor fonts does not implement PDF text export.

Formula objects use STIX Two Math embedded by the pinned `latex-rust` dependency,
independently of these UI/text fonts. Its original outlines and OpenType MATH
metrics feed the shared renderer. Redistribution license and upstream notices are
in `assets/licenses/STIXTwoMath-OFL.txt` and `assets/licenses/latex-rust-NOTICE.txt`.
