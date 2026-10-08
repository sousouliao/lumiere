# 0019: Native Full-Resolution Region Overlay

Date: 2026-09-14

Tracking: [GitHub Issue #19](https://github.com/sousouliao/lumiere/issues/19)

Status: Accepted for implementation; platform verification pending.

## Decision

Region selection moves into each native Host. At shortcut dispatch, Electron sends one
`captureRegion` request with delivery preferences. The Host resolves the pointer display,
acquires one frozen native frame, derives a full-pixel sRGB Visual Match display surface,
then shows an interactive native overlay. The selection remains in target-local logical
coordinates; the Host crops the retained source frame only after selection. A separate
request-id cancellation command can end selection while `captureRegion` is pending.

The overlay stays hidden and reusable while the Host is resident, but screen content is
captured only on demand. The overlay must not become interactive before its matching
frozen image is drawable. On pointer display change, the Host hides the old overlay,
releases that session, and acquires a new frozen frame for the latest target.

macOS uses ScreenCaptureKit, the existing Core Image Visual Match transform, and an
AppKit window on the main event loop. Windows retains the WGC frame texture and uses
D3D11/Direct2D for the full-pixel display surface and selection UI. Both Hosts keep
their existing output conversion and delivery semantics.

## Rationale

ADR 0014 optimized the encoded preview by reducing it to logical resolution. That
improved its transfer cost, but made fine text visibly softer and left PNG encoding,
file transport, Chromium decoding, and cross-process window activation ahead of user
interaction. The 2026-09 diagnosis measured about 90 ms of macOS warm preview PNG
encoding alone on a 5120×2880 source. Windows source inspection found a full-frame
RGBA16F readback and a CPU color-conversion loop before preview encoding.

The capture and display APIs are already platform-owned. Native overlays keep the
full-resolution image and its lifecycle inside the owning Host and avoid native-handle
transport through Electron. The cost is two platform-specific interaction surfaces;
their behavior is governed by the same logical selection and cancellation contract.

## Consequences

- The new protocol version replaces the Region prepare/commit/cancel session and
  preview-file contract. Shell and both Hosts must upgrade together; mixed versions
  must fail clearly.
- The Region overlay and its graphics resources are reused, while each frozen frame
  and its derived surface are released on completion, cancellation, switch, timeout,
  topology change, or process exit.
- Pixel size, color, and interaction are verified on each OS. A texture allocation or
  `orderFront` return is not proof of a visible interactive overlay.
- The warm shortcut-callback-to-interactive p90 target is 150 ms on named hardware,
  measured separately on macOS and Windows. Missing this target requires stage-level
  optimization, not reduced preview resolution, PNG in the hot path, or continuous
  background capture.

This decision supersedes ADR 0014 for Region preview once the native path is integrated
and verified. Existing released versions continue to use ADR 0014's implementation.
