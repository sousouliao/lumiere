# Windows Engine Development Runbook

Windows host adaptation is active. The repository contains a platform-host v5 executable
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
delivery from the same encoded sRGB Visual Match artifact. Region is advertised when
the target under the pointer has effective-DPI logical geometry and a reconstructable
native target snapshot. `prepareRegion` copies the first complete WGC frame into an
application-owned texture; `commitRegion` crops that frozen frame to an outward-aligned
pixel rectangle inside the native boundary.

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
4. Do not configure production Publisher, sparse identity, or updater metadata without a
   later decision. A future signed lane must open a new ADR and Issue and repeat its
   signing, identity, update, checksum, provenance, and clean-machine verification.

An unsigned stable release represents the verified application posture, not a signed
publisher identity. It does not verify signing or borderless capture.

Build the unsigned Windows installer from the repository root:

```powershell
pnpm package:windows
```

This produces `artifacts/windows/build/Lumiere-Setup-<version>-x64.exe`. Unsigned installers
deliberately omit the production sparse identity and updater configuration, so WGC keeps
the system capture border and automatic updates remain disabled.

The active release workflow always uses this unsigned path for Windows, adds the installer
to the unified checksum manifest, and does not generate `latest.yml`. Dormant SignPath,
sparse-identity, and updater code remains unavailable to the workflow. Do not reconnect it
or add signing variables or secrets until a future ADR and Issue establish a provider and
verification plan.
