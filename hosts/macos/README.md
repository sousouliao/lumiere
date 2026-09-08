# macOS Host

This directory owns the Swift ScreenCaptureKit adapter at the platform-host seam. It
acquires HDR-aware frames, converts the official output to sRGB Visual Match, and
performs native clipboard and file delivery.

The implementation targets macOS 15 or newer. Development builds use the active
architecture; distribution builds a universal `arm64` + `x86_64` Host and verifies both
slices before packaging. Apple Silicon uses ScreenCaptureKit's local-display
HDR screenshot preset when the target display exposes potential EDR headroom above
1.0. The target is the display under the pointer when the request reaches the host;
main and system-primary displays are recovery fallbacks only. Intel capture remains
SDR and does not claim HDR acquisition.

Build and test the host:

```sh
swift build --package-path hosts/macos
swift test --package-path hosts/macos
```

The executable reads platform-host v4 JSON Lines requests from standard input, writes
protocol responses to standard output, and reserves standard error for structured
diagnostics correlated by request ID. The current implementation supports display and
target-local region capture with clipboard, folder, or both-target delivery from one
converted PNG. Capabilities issue an opaque, short-lived region target token; the Host
rejects stale tokens, changed display topology, and out-of-bounds geometry before using
ScreenCaptureKit to convert target-logical points into the owning display's pixel output.
Every capture returns one result per requested delivery target.
