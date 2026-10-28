# Single-PDF native document format (proposed v1)

M0.5 implements the blank-document subset below. Drawing serialization and full edit round trips remain the M2 gate; the broader examples are still a design specification.

## Implemented M0.5 blank-document subset

Output is PDF 1.7: a Catalog, one Pages/Page tree, a conventional A4 MediaBox
(595.276×841.89 pt), an empty content stream and a named embedded file. The same
finite A4 page is a blank Whiteboard's ordinary-viewer fallback, not a limit on
its XiToc canvas. Output is not claimed to be PDF/A.

`/Root /Names /EmbeddedFiles` contains exactly one `XiToc/document.json` entry
pointing to a Filespec whose `/EF /F` and `/EF /UF` reference the JSON stream.
The stream has MIME `application/vnd.XiToc.document+json`. Paths are not identity
and are not stored in the edit payload, so filesystem relocation requires no PDF
rewrite. Example implemented Page payload:

```json
{
  "schema_version": 1,
  "document_id": "f0be46d4-b81e-4c41-a184-d70f5f46ab91",
  "layout": { "kind": "page", "pages": [{ "width_mm": 210.0, "height_mm": 297.0 }] }
}
```

A Whiteboard has `layout: { "kind": "whiteboard", "viewport": { "center": [0.0,
0.0], "zoom": 1.0 }, "grid": true }`. There is no independently mutable kind or
mode field. UUID and validated initial settings round-trip inside this one PDF.
The broader `mode`/object example below is conceptual, not this implemented schema.

No payload: classify a valid ordinary PDF as Page, read-only; content rendering
and explicit conversion are deferred. A recognized but invalid/unknown schema,
invalid geometry, duplicate edit payload or broken attachment tree is an error.
Never overwrite the source. Recognized metadata alone is not proof that visible
external edits are represented: any nonempty content stream, multiple visible
pages or page annotations is treated conservatively as read-only in M0.5.
Compressed empty streams may also be treated read-only. External edits that strip
or alter payloads are not guaranteed to retain native editability.

Creation on native uses a synced temporary file and atomic no-clobber publication.
It never overwrites a filename collision. No updates to existing PDF content are
offered in this milestone. Browser exports retain the original file bytes and
require explicit download confirmation outside the app.

M2 still must introduce/version stroke/object and camera snapshots, generate
matching visible graphics, validate full semantic round trips, and implement
safe trusted-model updates. The blank-only parser rejects unknown payload fields;
it must not silently discard future drawing data.

## One PDF / two representations

1. **Visible PDF:** Standard pages and vector strokes/text/images, legible in common readers. A4 pages have MediaBox 595.28 x 841.89 pt approximately (210 x 297 mm), with deliberate unit conversions; do not mix CSS pixels, physical points and logical stroke coordinates.
2. **Embedded edit data:** One embedded file, e.g. `XiToc/document.json`, mime type `application/vnd.XiToc.document+json` (application-defined). Register it in the PDF `/Names` -> `/EmbeddedFiles` name tree; optionally reference it from `/AF` with `AFRelationship` according to the actual PDF version/specification chosen. Metadata stores editable strokes, object IDs, document mode, page layout, whiteboard offsets/camera, layers if present, and schema version. A regular viewer may expose this as an attachment; avoid relying on PDF viewers' ability to edit it.

Do not call generated PDFs PDF/A-3 or PDF/A compliant unless verified with a conformance validator. The use of embedded files alone does not establish PDF/A conformance.

## Example edit payload (conceptual)

```json
{
  "schema_version": 1,
  "application": "XiToc",
  "document_id": "example-uuid",
  "mode": "whiteboard",
  "viewport": { "center_x": 0.0, "center_y": 0.0, "zoom": 1.0 },
  "objects": [
    {
      "type": "stroke", "id": "example-stroke-uuid",
      "color_rgba": [20, 70, 140, 255], "width": 2.5,
      "points": [[0.0, 0.0], [11.0, 13.0], [29.0, 14.0]]
    }
  ]
}
```

## Whiteboard export
Calculate finite bounds of objects (+ padding), map them to one or more valid PDF pages (large extents may require tiling or scaling). Keep logical world coordinates and saved camera separate in edit metadata so reopening restores unlimited workspace behavior. An empty whiteboard should still export a valid visible page.

## Updating and external edits
On save, rewrite the PDF from the trusted document model and never maintain sidecars. If an external PDF reader strips or changes the embedded payload, reopen as external/read-only or require explicit conversion. Avoid silently discarding third-party content. Do not pretend all PDF annotations are native editable XiToc objects.

## Required tests
- Encode-decode round trip yields equivalent semantic document (mode, points, colors, width, stable IDs, negative/large positions, view settings).
- PDF begins with correct PDF header, parses with independent parser, and each page contains expected visible drawings.
- Edit payload extractable from exactly the output PDF; no extra files required.
- Bad/corrupted/missing payload is handled with a clear message and does not overwrite original.
- Empty, one-stroke, multi-page A4, and large/negative whiteboard tests.
- Preserve last good file on failed save; repeated save/reopen/modify/save should not grow stale payloads unboundedly.

## M1 in-memory drawing (not persisted)

`drawing::Stroke` prepares stable document/stroke UUIDs, page/canvas ownership,
f64 PDF-point positions and widths, pressure-ready points and RGBA color. These
objects are held in the editor session only. The implemented blank-PDF schema 1
has not changed; it cannot contain ink. No sidecar or alternate note format is
written. Export is blocked for unsaved drawing, and Save explicitly reports that
persistence is unavailable. Discarding/restarting loses the session's ink.

M2 must deliberately version the document payload and decode/migrate old blank
notes, include stroke/page ownership and editor snapshots, emit matching visible
vector paths with round caps/joins, and validate full drawing round trips. Never
feed an ink-bearing snapshot into the blank encoder or claim original blank PDF
bytes represent current edits. Add atomic update failure/recovery tests and a
saved revision marker before enabling Save or dirty-document export.

## M1.1 smoothing metadata (preparation for M2)

In-memory strokes retain editable raw samples and a serde-ready per-stroke
processing tag: `Legacy` or `AdaptiveV1 { strength: 0..100 }`. Strength is captured
at pointer-down; rendering and erasing reconstruct the same deterministic vector
path. JSON round trips preserve f64 samples exactly. Future M2 encoding must
version the PDF payload and serialize the raw vectors/processing metadata with
matching visible graphics. The existing blank-PDF schema remains unchanged and
cannot save handwriting; no sidecar format is created.
