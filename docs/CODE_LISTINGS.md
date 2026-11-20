# File-based Code Listings

Use **Insert → Code Listing…** for a UTF-8 file on a Whiteboard or Page Note.
The listing displays an embedded snapshot. Its original bytes and filename
are used by **Save Original File As…** or **Download Original File**. On
desktop, **Open Original File** uses the existing guarded snapshot opener.
It may recommend saving first for extensions or content the opener disallows.
Use **Replace Source File…** to import a new revision. There is no source
editing, automatic source synchronization or execution.

Double-click to inspect and select source for plain-text copy. Display and
source copy normalize CRLF and CR to LF and omit a UTF-8 BOM. Export preserves
the BOM, line endings and all other bytes. Use File Attachment for binary data
or other encoding. With wrap off, the inspector can scroll horizontally.

## Style reuse

Start with Scientific Light, Scientific Dark, or Classic LaTeX Listings.
Use **Code Listing Styles** in global or document LaTeX settings, select a
base appearance and choose **Duplicate as New Style** with a name. Edit the
appearance or apply a directive and check the preview before applying the
settings. Names are case-sensitive; document definitions shadow global
definitions, which shadow built-ins. Individual overrides take precedence
over all three. Deleting a named definition reveals an inherited definition
or the listing’s fallback. Cross-document clipboard freezes the effective
appearance to preserve the copy’s visuals.

## Supported configuration subset

Enter one directive at a time in the dedicated style section. A named
definition creates or updates the style. A setup command modifies the
currently selected style, optionally using `style=Name` as a base.

```tex
\lstdefinestyle{Scientific}{
  basicstyle=\ttfamily\small,
  keywordstyle=\color{blue},
  commentstyle=\color{gray}\itshape,
  stringstyle=\color{red},
  numbers=left, numberstyle=\tiny, frame=single,
  breaklines=true, tabsize=4
}
```

- `style`, `language` and `basicstyle` select inheritance, language and size.
  Size commands are `\tiny`, `\scriptsize`, `\footnotesize`, `\small`,
  `\normalsize`, and `\large`. Source is always monospace; choose between
  the bundled DejaVu Sans Mono and interface monospace in appearance settings.
- `keywordstyle`, `commentstyle`, `stringstyle`, `identifierstyle`,
  `backgroundcolor`, and `rulecolor` accept named black, white, blue, red,
  gray, and green or a six-digit hex color in braces. Use `\color{...}`
  for a token style, with optional `\itshape` after the comment color.
- `numbers` accepts `none`, `left`, or `right`. Customize with
  `numberstyle` (size commands), `numbersep`, `firstnumber`, and `stepnumber`.
- `frame` accepts `none`, `single`, `lines` / `tb`, or `leftline` /
  `l`. `framerule`, `framesep`, `xleftmargin`, `xrightmargin`, and
  `numbersep` accept point values with optional `pt` suffix.
- `breaklines`, `breakatwhitespace`, `showspaces`, `showstringspaces`, and
  `showtabs` take `true` or `false`. `tabsize` takes an integer 1–16.
- `caption` and `title` take literal text in braces and share one caption slot.

Unsupported commands and invalid values produce an error and keep the input
available to fix. The subset is inspired by [CTAN’s listings package](https://ctan.org/pkg/listings).
It is parsed independently of formula preambles and postambles. Package loading,
file inclusion, arbitrary macros, and external TeX execution are outside scope.

## Languages and rendering

Languages include Python, Rust, C, C++, JavaScript, TypeScript, JSON, HTML, CSS,
Bash, SQL, LaTeX, Markdown, Java, YAML, XML and TOML. Unknown extensions suggest
Plain Text. The listing’s explicit language override wins over an optional
style language, followed by file extension. The offline grammar backend is
two-face/Syntect with pure-Rust fancy-regex. The canvas lays out visible rows
and uses plain styling beyond its highlighting budget. PDF persistence is
future M2 work.
