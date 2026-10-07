# Windows Engine Development Runbook

Windows host adaptation is active. The repository contains a platform-host executable
using v5 for capabilities/Display and v6 for native Region and request-id cancellation,
with a capability handshake plus the three retained native libraries. Windows is required
for .NET restore, Release build, tests, formatting, WGC/D3D11/DXGI runtime behavior,
clipboard behavior, and HDR hardware checks.

## Prerequisites

- .NET 10 SDK
- Windows SDK `10.0.26100.x` or a documented compatible version
- x64 Windows

## Repository Gate

Run the shared gates first, then the Windows-owned entry point from repository root:

```sh
pnpm install --frozen-lockfile
pnpm check
pnpm test:shared
pnpm build
```

```powershell
pwsh ./hosts/windows/scripts/verify.ps1
```

The PowerShell script restores and Release-builds `hosts/windows/Lumiere.Windows.sln`, runs
the Host, Capture, Graphics, and Interop test projects, and verifies formatting. This
Windows-owned suite runs only in Windows CI; it is not part of the shared `pnpm test`
alias and does not execute macOS tests.

The repository-root `pnpm dev` command builds the current Windows Debug Host unless
`LUMIERE_WINDOWS_HOST_PATH` is set, then Electron selects that artifact ahead of a Release
fallback. The current Host executes Display capture with Clipboard, Folder, or Both
delivery from the same encoded sRGB Visual Match artifact. Native Region uses one v6
`captureRegion` request: the first WGC frame is retained, converted and presented at
full resolution by the native overlay, then cropped/delivered from that same frame.
Cancellation identifies the pending request. Effective-DPI logical selection maps to
outward-aligned backing pixels. The explicit Electron diagnostic override retains
`prepareRegion`/`commitRegion`; it is not the default product path.

The Windows Shell sends Region through the v6 native Host path by default. For targeted
comparison with the previous Electron preview overlay, set
`LUMIERE_WINDOWS_REGION_OVERLAY=electron` before starting the desktop app. This is a
diagnostic fallback, not the native runtime verification path.

## Truth Boundary

A passing Host handshake does not prove native capture, HDR Visual Match, or hardware
support. Record runtime capture, artifact delivery, source HDR state, and visual-match
observations separately.

If restore reports a partial NuGet cache, rerun restore with `--force` against the
Windows solution before changing source.

## Packaging and release

The Windows distribution lane advances in this order:

1. Land the packaging implementation and public MIT license, privacy policy,
   code-signing policy, and accurate repository/download documentation.
2. Run repository, Windows Host, installer, and packaged-runtime verification as a
   separate phase.
3. Publish the assisted unsigned NSIS installer in stable or prerelease GitHub Releases.
   The release page must document functionality, installation, uninstall, checksum
   verification, and expected unknown-publisher or SmartScreen warnings.
4. Do not configure production Publisher or sparse identity without a
   later decision. A future signed lane must open a new ADR and Issue and repeat its
   signing, identity, update, checksum, provenance, and clean-machine verification.

An unsigned stable release represents the verified application posture, not a signed
publisher identity. It does not verify signing or borderless capture.

Build the unsigned Windows installer from the repository root:

```powershell
pnpm package:windows
```

This produces `artifacts/windows/build/Lumiere-Setup-<version>-x64.exe` and matching
`latest.yml`. Unsigned installers omit production sparse identity, so WGC keeps its
system capture border. ADR 0020 enables explicit in-app updates independently of signing.
Packaged app-update.yml points at the official repository; SHA-512 is checked on download.
Publisher signature verification is explicitly disabled for unsigned installers.

The active release workflow always uses this unsigned path for Windows, adds the installer
to the unified checksum manifest, and uploads matching `latest.yml`. Dormant SignPath and
sparse-identity code remain unavailable to the workflow. Do not reconnect them
or add signing variables or secrets until a future ADR and Issue establish a provider and
verification plan.

## In-app update verification

Check after 30 seconds and every six hours without opening a window or downloading.
Use System settings to download, observe progress and restart to update. Ordinary quit
must not install; capture in progress disables restart. The Host must exit before NSIS
starts with `/S` and `--force-run`. Development builds do not update themselves.

Test two installed local versions using a test-only app-update.yml pointing at a loopback
HTTP fixture; do not ship that fixture or enable an environment override in production.
Observe a silent upgrade, actual new-version launch and settings retention. A wrong hash
must reject installation; an unavailable server must allow retry. Record source commit,
artifact hashes, commands, OS/GPU/display, install directory and observed OS prompts.
Restore the user's backed-up settings and installation after testing. Public-source
observation is a separate publication gate. Local uninstall/reinstall is not a clean-machine test.
