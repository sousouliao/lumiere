# 0022: Preserve scRGB in the Native Region Preview

Date: 2026-10-11

Tracking: [GitHub Issue #36](https://github.com/sousouliao/lumiere/issues/36)

Status: Implemented; physical SDR/HDR appearance and shell latency acceptance pending.

## Decision

Windows Region preview presents the retained WGC RGBA16F/scRGB frame through an
independent FP16 flip-model swap chain with `RGB_FULL_G10_NONE_P709` color space.
Each redraw copies the immutable source on the GPU, then draws selection UI into
the independent back buffer. The unmasked background is neither tone-mapped nor
normalized a second time. The overlay remains full-resolution and capture-on-demand.

UI token colors are decoded from sRGB to linear values and scaled to the target's
SDR white level. That scale is updated when the cached presenter attaches a new
target frame. Selection-outside dimming remains a black alpha blend; waiting for
selection does not dim the background.

PNG output continues to crop the same original frame and use the existing fixed
sRGB Visual Match conversion, encoding and clipboard/folder delivery. Native
preview capability does not establish HDR-preserved export.

This supersedes the Windows sRGB-derived preview choice in ADR 0019. The native
ownership and same-frame selection decision remains unchanged; ADR 0021 owns the
current Windows-only platform boundary.

## Rationale And Trade-offs

The prior preview reused the output tone map, compressing bright areas before
displaying the frozen desktop. Native scRGB presentation removes that compression
and eliminates the preview-only shader and intermediate BGRA texture. It requires
no second capture, CPU preview readback, encoding or cross-process image transfer.

The worker retains only valid monitor `GraphicsCaptureItem` descriptors, with
closed-event and geometry invalidation; sessions, pools and frames remain
capture-scoped. Successful device-level WGC support probes are cached, with native
failure invalidation, while target HDR/white-level queries remain live. This avoids
repeated WGC calls that independently grow handles on the observed Windows system.

FP16 swap-chain buffers consume more memory/bandwidth than BGRA buffers. Removing
the conversion does not by itself prove unchanged latency. Performance acceptance
compares preview startup and selection-to-delivery separately on named hardware;
native visibility polling is not physical shortcut-to-visible-frame evidence.
Provider/driver-dependent resource observations and physical appearance remain
separate from GPU readback and fixed-input PNG byte equality.

## Verification Boundary

GPU fixtures verify HDR/negative background values, masking without source
mutation, source reattachment, and unchanged PNG bytes. Existing same-frame crop
and output-baseline checks remain applicable. Measurements, exact commands,
pre-existing resource-test failures and remaining platform acceptance belong to #36.
