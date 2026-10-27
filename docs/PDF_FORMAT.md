# Single-PDF native document format (proposed v1)

This is a design specification for Codex to implement and test, not an already validated implementation.

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
