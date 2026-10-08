# Architecture Contract

## Platform Baseline

Lumiere currently targets Windows x64. Stable Tauri 2 owns the native shell and
on-demand WebView2; React/TypeScript owns the retained desktop UI. Rust owns the
separate capture Host and independent WGC/D3D11/DXGI capture library. No other platform
implementation, compatibility stub, or source archive is kept in the working tree.
[ADR 0021](../decisions/0021-windows-only-tauri-rust-migration.md) owns this decision.

## Module Boundaries

| Module | Responsibility |
|---|---|
| `apps/desktop/src/renderer` | React components, generated tokens, typed product state |
| `apps/desktop/src-tauri` | Native lifecycle, tray, shortcuts, settings, named IPC, Host supervision, signed updater |
| `crates/capture-contract` | Typed v5/v6 JSONL commands, validation and results |
| `crates/capture-windows` | WGC target/frame lifetime, DXGI HDR state, D3D conversion, native Region, PNG and delivery |
| `hosts/windows/rust` | Separate executable, concurrent request dispatch, cancellation, structured stderr |
| `protocol/platform-host` | Windows wire schemas and executable examples |
| `tools/windows-installer` | Transactional install ownership, explicit legacy inventory cleanup and rollback |

The native shell and Host remain resident. The WebView is created on demand and
destroyed on close. The shell supervises one adjacent Host executable over UTF-8
JSON Lines. Raw frames, textures, native handles and capture ownership never cross
that process seam. WGC/DXGI/D3D11 and COM/WinRT lifetime details stay in capture-windows.
Settings remain at `%APPDATA%/Lumiere/settings.json`; upgrades preserve that data.

## WebView Security

- Load packaged local content only; deny unexpected navigation and new windows.
- Use CSP and the minimum Tauri capability ACL; expose named validated commands.
- Do not give renderer code generic shell, filesystem or process access.
- Keep capture/conversion in the native library, never renderer Canvas or browser capture.

## Output And Lifetime

- Retain native RGBA16F acquisition until fixed sRGB Visual Match conversion completes.
- Probe HDR and SDR white level on the active target; missing required HDR input fails.
- Region selects and crops the same frozen frame, with a 60-second native selection lease.
- Clipboard and folder consume one encoded PNG; failures stay target-local.
- Keep artifact success, visual match and HDR preservation separate under the claims contract.
- Dispose native resources deterministically; log structured diagnostics to stderr.
- Tests establish the seam and lifecycle; physical capture/compositor fidelity requires hardware evidence.

## Distribution

NSIS installs per user and preserves the existing location, settings and unknown files.
Only explicitly owned files may be replaced or removed. Never invoke the former recursive
uninstaller. Interrupted or failed replacement rolls back from a sibling transaction backup.
The official Tauri updater verifies minisign signatures and signed versions before readiness;
installation waits for capture quiescence and confirmed Host exit. Both updater metadata
formats reference the same installer bytes. No automatic install on ordinary quit.
