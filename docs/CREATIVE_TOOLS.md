# Creative tools — proposed GitHub issue briefs and dependencies

M1.3 integrates Monoline, Pressure Pen, Highlighter presets, Stroke/Partial Eraser
and Hand in the top toolbar. Selection remains disabled. The following are
proposed issue titles ready for separate GitHub tasks, not implemented features.
Existing #11 covers adding pages; #10 covers split editors. Do not expose future
tools as working controls. Each slice needs desktop/WASM, both layouts, history,
large-coordinate geometry and PDF semantic/viewer round-trip coverage.

| Proposed GitHub issue title | Dependencies | Architecture and intended interaction |
| --- | --- | --- |
| Version the editable object model before M2 PDF persistence | M1 stroke model | Tagged Object enum; stable IDs, ordered objects, immutable page/canvas ownership, local f64 bounds and affine transform. Keep raw samples, smoothing algorithm/strength, explicit partial polylines and pressure response. Define validation, limits, unknown-variant errors and blank-schema migration. |
| Object selection, lasso and movement | Object model | Selection arrow hit-tests topmost visible object; Shift adds/removes. Lasso drag intersects visible geometry. Selection IDs and handles are transient editor state; one drag commits one undoable transform, Escape cancels, Space pans. Reuse spatial broad phase; never flatten ink. |
| Resize, rotate and arrange selected objects | Selection + transforms | Screen-sized handles, opposite-anchor resizing, rotation handle with modifier snapping. One command per gesture; Bring Forward/Send Back change explicit object order. Preserve transforms in model/PDF; do not rebuild raw pen samples during pointer movement. |
| Shape insertion with lines and arrows | Object model + selection | Shapes popover chooses rectangle/ellipse/line/arrow and style; drag previews, release inserts one semantic object, Escape cancels. Store geometry, fill/stroke, endpoint arrowheads and transform; selection edits later. |
| Text boxes and sticky notes | Object model + selection + font strategy | Text tool click inserts and focuses inline editing; sticky note inserts a colored semantic text container. Escape exits editing before tool changes. Store text, layout box, font/style and sticky background. Define portable font embedding/fallback and PDF text metrics before export; no OS-specific text in shared types. |
| Local images as editable objects | Object model + selection + resize | Explicit local picker/drop, bounded decode and image-size limits; drag/place then resize. Store image bytes/resources inside the PDF, preserve crop/transform. No linked external files, uploads or remote image service. |
| Attached connectors and arrows | Shapes + selection + stable IDs | Drag from object anchor to target; transient routing preview. Store endpoint object IDs/anchors or free world points and routing/style. Movement updates attachments; deletion policy detaches to last world point. Undo restores IDs and relations atomically. |
| Textured Pencil and advanced brushes | Versioned stroke style + PDF compositing | Monoline, Pressure Pen and translucent Highlighter are implemented in memory. Pencil still needs deterministic texture and editable/PDF representation; watercolor remains out of scope. Match live opacity, erasing and PDF graphics before persisted brush support. |

## Before implementing PDF ink persistence

The object-model contract issue is an M2 prerequisite. Decide an ordered object
container and transform convention now; M2 may initially support only the Stroke
variant, rejecting unsupported future variants explicitly. Do not implement all
future variants to ship ink. Keep canonical layout immutable, logical PDF-point
coordinates, stable IDs, bounded validation and versioned migration. Selection,
hover, handles and active tool never belong in canonical content. Camera snapshot
is separate from geometry; page ownership must not be inferred from position.

M2 must synchronize embedded `XiToc/document.json` with visible PDF graphics,
including pressure envelopes, exact partial fragments, object order and future
transparency/transforms/resources. Validate supported variants before saving;
unsupported content is an explicit error, never dropped. Preserve original bytes
and last valid revision on failure. Native writes must be atomic; browser reports
success only after actual write/export semantics are met. No schema or codec
changes are part of M1.3. Future object issues depend on matching serializer and
viewer round trips before persisted features are enabled.
