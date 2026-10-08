# Windows Capture Host Protocol

The Tauri shell supervises a separate Rust Host over UTF-8 JSON Lines on stdin/stdout.
Each request carries `version`, nonempty caller-generated `id`, `method` and `params`.
Each response echoes version/id and contains exactly one result or error. Concurrent
requests correlate by id; writes are serialized and flushed. Diagnostics use stderr.
Unknown versions, methods, fields or enum values fail validation.

[`v5.schema.json`](v5.schema.json) owns capabilities and Display.
[`v6.schema.json`](v6.schema.json) additionally owns native `captureRegion` and
`cancelRegion`. These self-contained Windows schemas retain the implemented Windows
wire shape. Earlier implementations and schemas belong to Git history.

Capture accepts `delivery` (`clipboard`, `folder`, `both`) and optional absolute
`saveDirectory` for folder/both. Absence uses Pictures/Lumiere; null and clipboard-only
saveDirectory are invalid. The Host resolves the pointer target at capture time.

Native Region captures one frozen frame, presents a full-resolution overlay, selects
in effective-DPI logical units and crops outward-aligned physical pixels from that
same frame. At most one capture is active. Selection expires after 60 seconds.
Cancellation identifies the pending Region request with `requestId` and returns
`released`, including for an already-ended request. The original capture result
remains correlated with its original id. Capture is reserved before asynchronous work
so immediate cancellation cannot overtake it. EOF cancels and joins native work.

A completed result means acquisition/conversion completed, not that every delivery
succeeded. Exactly one result per requested target is required; duplicate targets are
invalid. Folder success includes the final path; clipboard success has no path.
Both targets use one sRGB Visual Match PNG and report failures independently. No
preview path, raw frame or image handle crosses the seam. Artifact delivery does not
prove visual match or HDR preservation.

`fixtures/v5` and `fixtures/v6` illustrate accepted envelopes. Run
`node scripts/verify-rust-protocol.mjs` after building the Debug Host to validate all
fixtures and live correlated responses. Rust transport tests own malformed messages,
request concurrency and cancellation. Real display delivery is an explicit hardware
check through `scripts/verify-rust-display.mjs`.
