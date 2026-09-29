# Windows Host And Engine

Windows host adaptation is the active Milestone 1B frontier. The
`Lumiere.Windows.Host` executable owns the platform-host v2 JSON Lines process boundary;
it connects the retained engine for Display and target-token-bound Region capture, one
sRGB Visual Match conversion, and Clipboard, Folder, or Both delivery. Region is advertised
only when the current target supplies a reconstructable native snapshot plus target-local
logical geometry. It does not restore a WinUI product shell.

The retained modules are:

- `Lumiere.Windows.Capture` for WGC target resolution, session state, and frame lifetime.
- `Lumiere.Windows.Graphics` for D3D11/DXGI, HDR-aware input, sRGB Visual Match, PNG,
  clipboard, and folder delivery.
- `Lumiere.Windows.Interop` for the COM/WinRT and diagnostic implementation those
  modules require.
- `Lumiere.Windows.Host` for stdin/stdout protocol handling and structured stderr
  diagnostics.

The capture interface owns target resolution, target-aware HDR probing, target-local
logical-to-pixel Region conversion, first-frame acquisition, one sRGB Visual Match
conversion, requested delivery, cancellation, and native teardown. On an HDR-active target,
the conversion normalizes captured scRGB input against that display's current Windows SDR
white level before tone mapping; if that value cannot be resolved, capture fails instead of
claiming an unverified Visual Match artifact. Issued Region tokens
are short-lived and bind to an opaque Capture-owned target snapshot. A caller supplies a
correlation ID but never owns a raw frame, monitor handle, texture, capture session, or
output cache. Call
`WindowsDisplayCaptureEngine.ConfigureLogging` before creating the engine when the
process needs a structured stderr logger.

The executable conforms to `../../protocol/platform-host/v2.schema.json`. A Debug build
lives at
`src/Lumiere.Windows.Host/bin/x64/Debug/net10.0-windows10.0.26100.0/win-x64/Lumiere.Windows.Host.exe`;
the Electron development launcher builds and selects that artifact before a Release
fallback. `LUMIERE_WINDOWS_HOST_PATH` remains the authoritative development override.

Run `./scripts/verify.ps1` on Windows to restore, Release-build, test, and format-check
the Host and retained engine.
