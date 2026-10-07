# Current Project State

- Updated: 2026-10-08
- Frontier: Windows-only Tauri/Rust migration, [#25](https://github.com/sousouliao/lumiere/issues/25).
- Approved decision: [ADR 0021](../decisions/0021-windows-only-tauri-rust-migration.md).
- Clean implementation baseline: `cc0c011af0bc53be696d4f84bda803df24315f6e`.
- Public distribution remains Windows `v0.6.0` from `d218c02`, unsigned x64 NSIS;
  historical macOS distribution is `v0.5.0`. This work does not authorize publication.

## Current implementation

The baseline still runs Electron/React and the .NET Windows Host. Capabilities and
Display use v5; native Region uses v6 and request-ID cancellation. Native selection
crops the retained full-resolution frame. Explicit Windows updates follow ADR 0020.
No Rust or Tauri runtime cutover is claimed yet.

The accepted target removes Electron, .NET and all macOS-specific implementation
from the final product. It keeps an independent Rust capture library, thin native
Host and language-neutral protocol. The native shell/Host stay resident; the WebView
is created on demand. In-place upgrades must preserve settings/user files and remove
legacy application files. Git owns the previous macOS implementation; no frozen
source archive or speculative platform stubs will remain.

## Verification posture

P0 repository baseline: `pnpm check`, `pnpm test:shared` (34 files, 204 tests),
`& ./hosts/windows/scripts/verify.ps1` (Host 41, Capture 93, Graphics 50, Interop 35;
Release build and format) and `pnpm package:windows` pass on this Windows machine.
PowerShell 5.1 runs the unchanged native script because `pwsh` is not installed.
The rebuilt installer and full program inventory are frozen under ignored baseline
artifacts; [#26](https://github.com/sousouliao/lumiere/issues/26) owns hashes and commands.

An isolated baseline renderer fixture again failed with Electron GPU-process exits
and `ERR_FAILED`, including one software-rendering retry. Fresh UI screenshots are
not verified. The pinned renderer source/design remains reproducible; real UI
comparison is still required before final cutover.

Earlier native observations on RTX 5080 / 3840x2160 HDR / 150% included packaged
same-frame Region delivery and bounded repeated captures. [#22](https://github.com/sousouliao/lumiere/issues/22)
and [#23](https://github.com/sousouliao/lumiere/issues/23) retain missing topology,
physical-input/compositor, sleep/reconnect and non-development-machine acceptance.
The v0.6.0 release-only waiver does not certify the rewritten implementation.
[#24](https://github.com/sousouliao/lumiere/issues/24) owns old installed update evidence;
the new installer must independently verify both generations of upgrades.

## Next action

Land the baseline/ADR/inventory slice, then implement P1's Rust workspace, strictly
typed Windows protocol, cancellable request loop and transport tests. Keep the old
application only as a migration reference until replacement verification passes.
