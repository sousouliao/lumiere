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

P1 adds the pinned Rust workspace, typed Windows v5/v6 contract and independent
JSONL Host. It reserves captures before dispatch, processes controls concurrently,
and waits for native release on cancellation, lease expiry, EOF and broken output.
P2 implements cursor-targeted WGC Display capture, retained RGBA16F textures,
target-aware DXGI/SDR-white conversion and shared PNG clipboard/folder delivery.
An owned worker retains the MTA and D3D device. P3 adds native GPU Region selection
with the baseline shader, D2D/DirectWrite affordances, same-frame crop and v6
cancellation. A hidden HWND/presenter is reused; frozen source bindings are detached
after selection. Both v5/v6 capabilities advertise the implemented modes.

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

P1 verification: `cargo test --workspace` (4 protocol + 5 transport tests),
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`,
`cargo build -p lumiere-windows-host` and `node scripts/verify-rust-protocol.mjs`
(5 real-child responses against existing schemas) pass. Script ESLint/Prettier pass.
[#27](https://github.com/sousouliao/lumiere/issues/27) owns this slice.

P2 verification: Cargo tests (4 protocol + 4 conversion/delivery + 5 transport),
format and strict Clippy pass. The frozen .NET conversion fixture matches byte for
byte at four scales. Real Release Host/schema capture succeeds. Explicit native
both-delivery test passes on this machine: HDR 3840x2160, exact PNG clipboard/file
equality, DIBV5 availability, partial folder failure; observed 1.32–1.38 seconds.
These samples do not establish performance distribution or compositor visual parity.
[#28](https://github.com/sousouliao/lumiere/issues/28) owns commands and limitations.

P3 verification: Cargo tests (4 protocol + 6 conversion/geometry/delivery + 5
transport), strict Clippy, format, layout and real-child schema checks pass.
Explicit Release fixtures verify the retained HDR frame's exact 200x140 crop and
30 native selection/Esc/right-click/remote-cancel cycles plus active-window EOF.
Warm/final handles 667/659, USER 30/30, GDI 10/10; counters include driver retirement.
Display both-delivery regression also passes. Hardware fixtures remain opt-in and
synthetic input does not certify physical-input/compositor or multi-monitor behavior.
[#29](https://github.com/sousouliao/lumiere/issues/29) owns evidence and decisions.

Implement P4 Tauri shell and renderer bridge, preserving current Windows UI and
behavior while creating/destroying the WebView on demand. Keep the former shell as
a reference until replacement verification; SDR/topology/UI/upgrade/performance
acceptance remains open.
