# Lumiere

Lumiere is a precise, HDR-aware screenshot tool for Windows. Region and display capture
deliver fixed sRGB Visual Match PNGs tuned for everyday clipboard and folder use.
Native shell and capture services stay resident; the interface opens on demand and
closes cleanly.

HDR-preserved export is not yet a supported public path.

## Start Here

- [Current project state](knowledge/state/CURRENT.md)
- [Product roadmap](knowledge/roadmap.md)
- [Changelog](CHANGELOG.md)
- [Code signing policy](CODE_SIGNING_POLICY.md)
- [Knowledge map](knowledge/README.md)
- [Product contract](knowledge/contracts/product.md)
- [Desktop development runbook](knowledge/runbooks/cross-platform-development.md)
- [Windows development runbook](knowledge/runbooks/windows-development.md)
- [On-demand release runbook](knowledge/runbooks/releasing.md)

The repository uses a lightweight Contract → Frontier → Verification workflow.
GitHub Issues own non-trivial tasks and observed checks, contracts own stable
boundaries, `CURRENT.md` owns the frontier, and Git owns history.

## Platform

`Tauri 2` · `React` · `TypeScript` · `Rust` · `WebView2` · `WGC/D3D11/DXGI`

## Install on Windows

The Windows release is an unsigned x64 installer for Windows 10 or newer. Download the Setup
executable and `SHA256SUMS` from the same official release, then compare the installer's SHA-256
digest before running it:

```powershell
Get-FileHash .\Lumiere-Setup-<version>-x64.exe -Algorithm SHA256
```

Because early releases are not yet code-signed, Windows SmartScreen may display an unknown-publisher warning. Continue
only when the calculated digest matches `SHA256SUMS` from the official release. The assisted installer runs
per user, allows a custom destination, and can create desktop and Start menu shortcuts. Uninstall
Lumiere through Windows Settings or its Start menu shortcut. Windows releases intentionally exclude
the production borderless-capture identity, so Windows Graphics Capture keeps its system capture border.
Lumiere checks for updates periodically; download and silent restart installation are
explicit actions in System settings. Ordinary quit does not install. Existing v0.5.0 users need
one manual upgrade to obtain this capability. Public v0.5.0 has no in-app updater.
Unsigned updates can still be blocked by Windows application-control policies.
From v0.7.0, the app verifies updater signatures and signed versions independently of
Windows publisher identity. Upgrading from v0.6.0 preserves settings and the existing
installation location; user-owned files are retained while old application files are removed.

System settings includes an optional **Launch at login** switch, off by default.
It starts Lumiere quietly in the system tray for the current Windows user without administrator
privileges. If Windows disables startup, use the **Open Windows Settings…**
button to review Startup Apps.

## Repository Layout

```text
apps/       Tauri shell and retained React UI
protocol/   language-neutral platform-host schemas and fixtures
hosts/      separate resident Windows Rust Host
crates/     typed capture contract and independent Windows capture library
tools/      transactional Windows installer helper
knowledge/  contracts, current state, ADRs, roadmap, and runbooks
scripts/    cross-repository structural checks
```
