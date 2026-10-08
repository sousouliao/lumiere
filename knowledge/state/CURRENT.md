# Current Project State

- Updated: 2026-10-08
- Frontier: P6 final acceptance, [#32](https://github.com/sousouliao/lumiere/issues/32), under [#25](https://github.com/sousouliao/lumiere/issues/25).
- Approved decision: [ADR 0021](../decisions/0021-windows-only-tauri-rust-migration.md).
- Branch: `codex/windows-rust-migration`; frozen baseline: `cc0c011af0bc53be696d4f84bda803df24315f6e`.
- Public distribution remains Windows `v0.6.0` from `d218c02`. Migration does not authorize push, tag, workflow dispatch, public release or a new release version.

## Implementation

The source cutover targets Windows x64 only: stable Tauri 2.12.1, retained React
components/tokens, an independent Rust capture library and resident supervised JSONL
Host. The native shell/Host remain resident; WebView2 is created on demand and destroyed
on close. Native tray, transactional shortcuts/recording, v1-v5 settings migration at
`%APPDATA%/Lumiere/settings.json`, quiet capture and failure recovery are implemented.
Former Electron/.NET/macOS implementation, resources, tests and configuration are removed.
No source archive or platform stub remains; Git owns history. Historical ADRs, release
entries and design variants do not imply current platform support.

Windows v5 capabilities/Display and v6 native Region/cancellation retain WGC RGBA16F,
active-target DXGI/SDR-white state, fixed sRGB Visual Match and one PNG for clipboard/folder.
Region selects/crops the same frozen frame, owns native cancellation and a 60-second lease.
An exact half-domain transfer lookup removes repeated per-pixel curve evaluation without
changing the baseline conversion bytes. No HDR-preserved or compositor-fidelity claim.

NSIS owns transactional replacement/rollback and explicit legacy inventory cleanup;
it preserves install location, settings and unknown files and never runs the legacy
recursive uninstaller. Official Tauri updates verify minisign signatures and signed versions,
retire capture/Host before handoff and pin the existing path. Download/install are explicit;
debug updates are disabled. Matching `latest.yml` and signed `latest.json` use the same NSIS.
Private keys stay outside Git; release CI secrets have not been provisioned.

## Verification posture

P0-P5 evidence belongs to [#26](https://github.com/sousouliao/lumiere/issues/26) through
[#31](https://github.com/sousouliao/lumiere/issues/31). P6 repository checks, 5 shared test
files / 37 retained tests, Rust workspace tests, strict Clippy/format, Vite/native Release,
Windows schemas and local installer/updater checks pass. CI configuration now targets
Rust/Tauri Windows; it has not been dispatched. #32 owns exact checks and artifact hashes.

Real HDR 3840x2160 both-delivery bytes and same-frame Region crop pass. The 30-cycle
WebView fixture passes with settled native/React state before snapshots; warm/final shell
handles 461/463 and private bytes 14127104/15339520. Vendor late-close PostMessage warnings
remain without fixture failure. Main/settings geometry was reviewed at 150% DPI; complete
state-by-state pixel parity is not established.

After 4 warm captures and 30 alternating resident Display folder captures per Host,
Release Rust median/p90 1119/1176 ms vs frozen baseline 1409/1426 ms: -20.6%/-17.5%.
Both report HDR. The earlier p90 regression was corrected by the exact transfer lookup.
This establishes the observed Host hot path, not physical Region or UI-trigger latency.
Local installer 3400788 bytes vs baseline 133323003; shell/Host 12640256/1067520 bytes.

Process-tree sampling includes Host/WebView2: installed baseline hidden idle private bytes
225710080; Debug GUI fixture ranges 6209536-307015680 while children start/exit and capture/
update/window states differ. These samples are not an equivalent-state Release memory gate.
All-state/DPI, physical input/compositor, SDR/topology, sleep/reconnect, clean-machine,
installed legacy-client update and equivalent-state total-memory acceptance remain open.
Earlier release waivers do not certify the replacement; #32 remains open.

The user's installed baseline at `D:\lumiere` remains restored/running after the P5 fixture
incident recorded in #31. All 309 frozen hashes were verified again; original registration
and settings remain intact. Fixed fixtures retain process handles, pin `/D` and wait for
installer commit before restoring the three owned HKCU keys.

## Next action

Complete the remaining P6 equivalent-state Release memory and user-journey/hardware
observations on the required environments, record them in #32 and close acceptance only
when the evidence supports it. Do not publish or change the real installed baseline.
