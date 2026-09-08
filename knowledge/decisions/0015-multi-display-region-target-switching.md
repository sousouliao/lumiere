# 0015: Stable Native Targets For Multi-Display Region Switching

Date: 2026-09-08

## Decision

Region capture follows the pointer between displays by reusing one prewarmed Overlay
window and one native frozen session at a time. Electron main observes the global pointer
display during an active Region operation. When it changes, main serially hides the old
Overlay, releases its frozen session and preview, prepares the new display, and activates
the same window with new bounds, preview, and generation.

The platform-host seam moves to version 4. `getCapabilities` may issue an opaque,
short-lived target token with the native target's logical size. `prepareRegion` consumes
that token exactly once. Hosts retain the corresponding native target snapshot and reject
stale tokens or changed topology; Electron never interprets a token as `Display.id`,
`CGDirectDisplayID`, `HMONITOR`, or a cross-platform coordinate. A small bounded token set
prevents unrelated capability refreshes from invalidating an in-flight Region command.

## Context

The previous implementation prepared one frozen frame for the display under the pointer,
then set one Overlay window's bounds once. Moving the pointer outside that window produced
no renderer pointer event, and only moving the window would still show and crop the old
display's frozen frame.

Electron does not expose a cursor-crossed-display event, and its public API does not promise
that `Display.id` equals either platform's native display handle. Electron cursor positions
are also DIP values rather than a portable native monitor coordinate. The source analysis is
recorded in
[`../research/multi-display-region-overlay.md`](../research/multi-display-region-overlay.md).

## Consequences

- Overlay bounds, preview, target-local geometry, and native frozen frame switch as one
  target bundle; the UI cannot silently crop the previous display.
- A single switch task coalesces rapid pointer movement toward the latest display. New
  generations reject late renderer ready and submit messages. Selection is disabled while
  a target switch is in flight.
- Target resolution retries at most three times with a short delay; a stable unavailable
  target ends the operation instead of spinning in Electron main. Overlay readiness starts
  before target resolution so renderer recovery remains parallel with native work.
- The short-lived watcher exists only during Region selection. Display topology changes,
  cancellation, lease timeout, app teardown, preview failure, and renderer failure still
  converge on the existing cleanup path.
- Switching includes native acquisition and preview latency. If real platform observation
  finds the gap unacceptable, a later decision may introduce one pre-frozen session and one
  Overlay per display; that is not required for this fix.
- Version 3 remains frozen compatibility history. The bundled Shell and Hosts move together
  to version 4; no dual Region implementation is retained.

## Rejected Alternatives

- Move only the BrowserWindow: preview and final crop would still belong to the old display.
- Pass Electron `Display.id` to native Hosts: no public API guarantees equivalence with native
  display handles.
- Pass Electron cursor DIP coordinates to native Hosts: mixed-DPI Windows and macOS coordinate
  origins require platform-specific conversion and create a weaker identity contract.
- Cover the virtual desktop with one giant window: this expands mixed-DPI, negative-origin,
  macOS Space, multi-target HDR, and frame-stitching scope beyond the requested behavior.
