# Windows Engine Development Runbook

Windows host adaptation is active. The repository contains a platform-host v4 executable
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
3. Publish a clearly labeled unsigned preview GitHub Release using the same assisted
   NSIS artifact form intended for production signing. Its release page must document
   functionality, installation, uninstall, and expected unsigned-publisher warnings.
4. Apply to SignPath Foundation only after that public released/documented-project
   prerequisite is satisfied.
5. After approval, configure the assigned Publisher, GitHub trusted build system,
   SignPath project, roles, signing policy, artifact configurations, repository
   variables, and API-token secret.
6. Publish a matching stable tag through the signed workflow, then verify signing,
   identity registration, borderless/fallback behavior, full-installer update,
   uninstall, checksums, provenance, and a clean-machine journey.

The unsigned preview is an application prerequisite. It is not a production release
and does not verify signing or borderless capture.

Build an unsigned local preview installer from the repository root:

```powershell
pnpm package:windows
```

This produces `artifacts/windows/build/Lumiere-Setup-<version>-x64.exe`. Preview installers
deliberately omit the production sparse identity and updater configuration, so WGC keeps
the system capture border.

Production releases run `.github/workflows/release.yml` from a finalized release commit on
`main`; the unified workflow creates the matching `v<package-version>` tag only after all
selected platform artifacts succeed. Configure the SignPath organization, project, signing policy,
application artifact configuration, installer artifact configuration, certificate
Publisher, and Publisher display name as repository variables; store only the SignPath API
token as a secret. The application artifact configuration must preserve the uploaded
`windows-host/` and `windows-identity/` paths while signing Lumiere-owned PE files and
`Lumiere.Identity.msix`. The installer configuration signs the final Setup executable.

The workflow generates `latest.yml` only after final signing, attests the signed Setup, and
adds the installer to the unified checksum manifest and GitHub Release. Installer registration of
the sparse identity is non-fatal; a failed registration or denied borderless consent falls
back to the normal WGC system border.
