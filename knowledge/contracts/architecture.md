# Architecture Contract

## Platform Baseline

- Shared shell: Electron, React, TypeScript, and Chromium; Windows and macOS only.
- Windows host: `.NET 10`, WGC, D3D11, DXGI, Vortice, `x64` / `win-x64`.
- macOS host: Swift, ScreenCaptureKit, and native Apple color/GPU frameworks; the
  distribution supports macOS 15 or newer with separate `arm64` and `x64` applications.
  Apple Silicon owns HDR acquisition; Intel remains an SDR-only path.
- MVP output: one shared semantic profile, RGBA8/sRGB Visual Match, delivered through
  platform-native clipboard and file adapters.

Do not introduce Electron desktop capture, renderer Canvas, `NativeImage`, GDI,
cloud upload, or telemetry as the official capture/conversion foundation.

## Module Boundaries

| Module | Responsibility |
|---|---|
| `apps/desktop` | Electron lifecycle, shared React UI, secure preload, platform-host orchestration |
| `protocol/platform-host` | Language-neutral process protocol, compatibility rules, schema, and fixtures |
| `hosts/macos` | Swift ScreenCaptureKit adapter, HDR-aware acquisition, sRGB Visual Match conversion, and native delivery |
| `hosts/windows/src/Lumiere.Windows.Host` | .NET executable, protocol validation, capability queries, and capture-engine orchestration |
| `hosts/windows/src/Lumiere.Windows.Capture` | WGC target resolution, frame-pool lifecycle, capture state |
| `hosts/windows/src/Lumiere.Windows.Graphics` | D3D11/DXGI device state, HDR-aware readback, sRGB Visual Match, native delivery |
| `hosts/windows/src/Lumiere.Windows.Interop` | Required COM/WinRT adapters, diagnostics, and native-resource wrappers |

Both native Hosts communicate with Electron through the platform-host JSON Lines
process interface. Electron selects and supervises the owning platform's executable;
the Windows Host composes the Capture, Graphics, and Interop libraries.

Platform APIs stay in their owning module. The shell consumes the platform-host
interface but must not own WGC, DXGI, D3D11, ScreenCaptureKit, Metal, ColorSync,
COM/WinRT, or native-resource lifetime details. IPC may carry commands, typed state,
diagnostics identifiers, and artifact paths; it must not carry raw HDR frames.

## Electron Security Invariants

- Load packaged local content only.
- Keep renderer sandboxing and context isolation enabled and Node integration disabled.
- Expose one named preload method per command; never expose `ipcRenderer` directly.
- Validate IPC sender and payload before crossing the platform-host seam.
- Deny unexpected navigation and window creation.

## HDR Invariants

- Preserve each platform's native high-dynamic-range acquisition semantics until the
  shared sRGB Visual Match conversion is complete.
- Assess HDR against the active target, not a global or first-monitor assumption.
- Keep sRGB Visual Match conversion behind each native host but governed by one shared
  semantic contract and fixed regression fixtures.
- Separate artifact delivery success, visual-match evidence, and HDR preservation.
- Keep platform capability and verification independent; one adapter cannot certify another.
- Require the claims contract before changing public HDR language.

## Resource And Diagnostics Invariants

- Dispose COM, DXGI, D3D11, WGC, ScreenCaptureKit, Metal, and related native resources deterministically.
- Use structured platform logging; do not use ad-hoc console output for native failures.
- Prefer typed results and explicit state transitions for expected platform failures.
- Automated tests may cover configuration, state, projection, protocol, and lifecycle
  seams; real capture/HDR presentation remains a platform hardware concern.
