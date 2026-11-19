# 0023: Native FP16 Region Selection Adjustment And Inline Annotation

Date: 2026-11-19

Status: Accepted for implementation.

## Decision

Region selection adjustment and Phase-1 inline annotations (`Rect`, `Spotlight`, `Arrow`,
`Freehand`, in that toolbar order) run natively inside `crates/capture-windows` on the
existing Win32 `Overlay` and DXGI FP16/Direct2D `Presenter`, rather than adding a
transparent WebView2 layer or a post-capture React editor.

After dragging a valid selection (or single-clicking to lock a top-level window), the
overlay transitions into an interactive `Ready` state with 8-way resize handles,
selection translation, and a token-driven floating Direct2D toolbar. Monitor switching
is locked once a window or selection is active. Double-clicking an empty region inside
the selection, pressing `Enter`, or clicking the primary confirm action commits capture
and delivery.

Export uses a two-stage pipeline:
1. Crop the immutable captured `RGBA16F` texture and run the unchanged fixed
   `rgba16f_to_rgba8` sRGB Visual Match conversion.
2. When no annotations are present, encode that result directly to PNG with identical
   bytes to unannotated capture. When annotations exist, composite them onto an
   offscreen Direct2D sRGB target using the same vector/spotlight renderer before PNG
   encoding.

`Spotlight` annotations compute `Selection \ Union(Spotlights)` via Direct2D path
geometry so lit cutouts retain 100% original brightness without darkening overlap seams,
and render beneath vector strokes (`Rect`, `Arrow`, `Freehand`). Linear FP16 preview
dimming alpha is calibrated to match the physical luminance of the exported sRGB veil.

## Rationale

- Keeping selection and annotation inside the resident Host's native Direct2D layer
  preserves ADR 0019/0022's low startup latency, avoids a 60–150 MB transparent
  WebView2 overlay process, and prevents cross-window DPI/coordinate drift.
- Running `rgba16f_to_rgba8` before compositing annotations preserves the fixed sRGB
  Visual Match contract (`claims.md`) for underlying desktop pixels while keeping
  annotation colors accurate without HDR highlight roll-off distortion.

## Consequences

- `REGION_LEASE_SECONDS` increases from `60` to `300` seconds (synchronized with the
  shell Host request timeout) so users have adequate time to adjust and annotate before
  automatic cancellation.
- Existing tests that previously committed a region on single `WM_LBUTTONUP` now commit
  via `VK_RETURN`, double-click, or toolbar confirmation.
