# Lumiere

Lumiere is a Windows and macOS HDR-aware screenshot tool. A shared Electron/React
shell coordinates native capture hosts: WGC/D3D11/DXGI on Windows and
ScreenCaptureKit on macOS. The first release focuses on fast region/display capture,
honest target-aware HDR state, and compatible sRGB Visual Match output.

Lumiere does not currently claim HDR-preserved export support.

## Start Here

- [Current project state](knowledge/state/CURRENT.md)
- [Product roadmap](knowledge/roadmap.md)
- [Changelog](CHANGELOG.md)
- [Code signing policy](CODE_SIGNING_POLICY.md)
- [Knowledge map](knowledge/README.md)
- [Product contract](knowledge/contracts/product.md)
- [Cross-platform development runbook](knowledge/runbooks/cross-platform-development.md)
- [Windows development runbook](knowledge/runbooks/windows-development.md)
- [macOS development and release runbook](knowledge/runbooks/macos-development.md)
- [On-demand release runbook](knowledge/runbooks/releasing.md)

The repository uses a lightweight Contract → Frontier → Verification workflow.
GitHub Issues own non-trivial tasks and observed checks, contracts own stable
boundaries, `CURRENT.md` owns the frontier, and Git owns history.

## Platform

`Electron` · `React` · `TypeScript` · Windows native host (`.NET 10`, WGC,
D3D11, DXGI, Vortice) · macOS native host (Swift, ScreenCaptureKit)

macOS can build and verify the shared shell. Each native host and all HDR claims still
require runtime and hardware verification on its owning platform.

## Install on Windows

The Windows preview is an unsigned x64 installer for Windows 10 or newer. Download the Setup
executable and `SHA256SUMS` from the same official release, then compare the installer's SHA-256
digest before running it:

```powershell
Get-FileHash .\Lumiere-Setup-<version>-x64.exe -Algorithm SHA256
```

Windows will show an unknown-publisher warning because the preview is not code signed. Continue
only when the digest matches `SHA256SUMS` from the official release. The assisted installer runs
per user, allows a custom destination, and can create desktop and Start menu shortcuts. Uninstall
Lumiere through Windows Settings or its Start menu shortcut. This preview intentionally excludes
the production borderless-capture identity and automatic updates.

## Install on macOS

Lumiere's macOS release is ad-hoc signed and is not notarized by Apple. Download the `arm64` DMG
for an Apple Silicon Mac or the `x64` DMG for an Intel Mac, together with `SHA256SUMS` from the
same official release. Older releases may instead provide one Universal DMG. Place the selected
DMG and checksum manifest in one directory, then verify the disk image before opening it:

```sh
shasum -a 256 -c SHA256SUMS
```

Open the DMG and drag `Lumiere.app` to Applications. On first launch, macOS may block the app
because it is not registered with Apple by a known developer. After attempting to open Lumiere,
open System Settings → Privacy & Security, scroll to Security, choose **Open Anyway**, authenticate,
then confirm **Open**. Only make this exception when the checksum matches the official release.
Apple documents the same manual override and its security implications in
[Open a Mac app from an unknown developer](https://support.apple.com/guide/mac-help/mh40616/mac).

## Repository Layout

```text
apps/       shared Electron desktop shell
protocol/   language-neutral platform-host schemas and fixtures
hosts/      native macOS and Windows ownership trees
knowledge/  contracts, current state, ADRs, roadmap, and runbooks
scripts/    cross-repository structural checks
```
