# Scientific Code Listings

Choose **Insert → Code Listing…** to import UTF-8 source in a Whiteboard or Page
Note. A single click selects the object. Double-click the body or choose
**Configure Code Listing…** in the object menu to open **Code Listing**. Imported
source is read-only. The text area edits only the listing's appearance.

The dialog previews valid changes on the canvas. **Apply** commits the path,
configuration, line selection and title as one undo step. **Cancel**, Escape or
closing the dialog discards those changes. Saving a named preset and setting the
import default are application preference actions and take effect when clicked.

## Presets and configuration

The built-ins are Scientific (the initial default), Scientific Dark, Classic
LaTeX and Minimal. Choosing a preset loads its configuration. Edit that text
and choose **Save Custom Style** to name a reusable preset. **Set as Default**
uses the currently selected saved preset for new listings. Application
preferences retain custom presets and the import default across restarts.
Every listing has a snapshot of its appearance. Editing a listing or changing
the import default leaves existing objects as they are.

Enter one directive, either `\lstset{…}` or `\lstdefinestyle{Name}{…}`.
The optional `style=Name` starts from a preset. Listings parse this subset
separately from mathematical LaTeX. Invalid text stays in the dialog for
correction. Arbitrary commands, input files, packages and TeX execution are
outside the subset.

```tex
\lstset{
    basicstyle=\ttfamily\small,
    keywordstyle=\color{blue},
    commentstyle=\color{gray}\itshape,
    stringstyle=\color{red},
    numbers=left,
    numberstyle=\tiny,
    frame=single,
    breaklines=true,
    tabsize=4
}
```

- `basicstyle` and `numberstyle` accept `\tiny`, `\scriptsize`,
  `\footnotesize`, `\small`, `\normalsize`, `\large`, `\Large`,
  `\LARGE`, `\huge` and `\Huge`. Source uses monospace typography.
- `keywordstyle`, `commentstyle`, `stringstyle`, `identifierstyle`,
  `rulecolor` and `backgroundcolor` accept `\color{…}` with black,
  white, blue, red, gray, green or a six-digit hex color. Comment color
  also accepts trailing `\itshape`.
- `numbers` takes `none`, `left` or `right`. `stepnumber` controls
  the numbering interval against original line indices. `numbersep` controls
  the gap. Number offsets are unsupported so visible numbers identify source.
- `frame` takes `none`, `single`, `lines` / `tb`, or `leftline` / `l`.
  `framerule`, `framesep`, `xleftmargin`, `xrightmargin` and `numbersep`
  accept point values with an optional `pt` suffix.
- `breaklines`, `breakatwhitespace`, `showspaces`, `showstringspaces`
  and `showtabs` take booleans. `tabsize` takes an integer from 1 through 16.
- `title` and `caption` share one literal title slot. The dedicated
  line-range field owns source selection. Language is detected from the filename.

## Line selection, title and size

Leave **Source Lines to Display** blank for all lines. Enter one-based
inclusive selections such as `1-20, 25-32, 35, 37`. Entries are sorted and
overlapping, adjacent or repeated ranges are combined. Out-of-range,
nonpositive or reversed selections are reported before Apply. Rendered
numbers still identify the original lines, so line 25 follows line 20 in
that example. After a linked file shrinks, any requested lines that still
exist are displayed and the expression is retained for later restoration.

The filename is the default title. A nonblank title field overrides a
configuration title or caption, which in turn overrides the filename.
Turning **Show Title** off removes the title row. Renaming a title does
not change the source filename.

Height follows the selected content and its wrapped visual rows. Resize to
change the preferred width while retaining font size. Unwrapped lines
expand the frame to their full width. Listings have no internal scrolling.
A Page Note marks an oversized listing and reserves workspace around its
paper so ordinary page scroll can reach the overflow. The listing still
belongs to its original page.

## Source links and offline use

On native, **Browse** or an absolute UTF-8 path chooses a replacement.
Apply reads a directly entered path in the background. Clearing the field
detaches the link and retains the snapshot. The app polls approved paths
at intervals on a worker and reads changed regular files. Atomic
replacement, disappearance and restoration are checked at the original
path. Missing, unreadable or invalid sources leave the last valid snapshot
visible. Source bytes, including original line endings, are retained
separately from display decoding.

Automatic refresh preserves geometry placement, style, title and ranges and
does not add undo steps. A later undo or redo restores user-authored state.
The active approved link then reconciles with its latest valid snapshot.
Explicit path changes can be undone. Pending dialog changes are separate
from external refresh. Files remain linked by path, so rename to a different
path requires Browse or editing the path.

Web uses permissioned file selection and the embedded copy. Browse to
refresh or replace it. Automatic filesystem synchronization is unavailable
in the browser. Two-face/Syntect highlights by extension with Plain Text
fallback. Long files stay fully visible even beyond the tokenization budget.

## PDF readiness

Listings are currently in-memory edits. M2 will need to embed the original
source and editable configuration and paint every selected line in the
conventional PDF representation. A portable persisted path should be treated
as a reconnect hint and require user approval before it is read. The embedded
snapshot must be sufficient for viewing an unopened or unavailable link.
