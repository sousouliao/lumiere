# Current Project State

- Updated: 2026-10-08
- Frontier: P5 installer and signed updates, [#31](https://github.com/sousouliao/lumiere/issues/31), under [#25](https://github.com/sousouliao/lumiere/issues/25).
- Approved decision: [ADR 0021](../decisions/0021-windows-only-tauri-rust-migration.md).
- Branch: `codex/windows-rust-migration`; frozen baseline: `cc0c011af0bc53be696d4f84bda803df24315f6e`.
- Public distribution remains Windows `v0.6.0` from `d218c02`; historical macOS is `v0.5.0`.
  No push, tag, public release or new release-version decision is authorized by this migration.

## Current implementation

The development shell now uses pinned stable Tauri 2.12.1 and the existing React
components/tokens. Native tray, global shortcuts, settings v1-v5 at the existing
`%APPDATA%/Lumiere/settings.json` location, capture activity/recovery and failure
notifications live in Rust. Shortcut registration/persistence is transactional;
recording temporarily releases native bindings. The native shell and supervised
Host stay resident; the WebView is created on demand and destroyed on close.
The updater is explicitly disabled in this P4 development shell until P5 lands.
Installed/public users still run the former Electron/.NET distribution.

The independent Rust Host implements Windows v5 capabilities/Display and v6 native
Region/cancellation. Its owned MTA worker retains D3D resources; WGC freezes RGBA16F
on the cursor's monitor. DXGI/SDR-white conversion produces the existing fixed
sRGB Visual Match PNG for clipboard/folder delivery. Region uses the baseline GPU
shader and native D2D/DirectWrite selection, then crops the retained frame. A hidden
native HWND/presenter is reused and frozen bindings detach after selection.
No HDR-preserved export or compositor fidelity claim is introduced.

The former shell/Hosts remain reference source until P6. The macOS permission and
WebView selection renderer surfaces have already been removed. P6 must remove all
remaining Electron, .NET and macOS implementation/resources/tests/configuration and
active support claims; Git owns their history. No frozen source archive or platform
stub will remain. New installer migration must preserve location/settings/unknown
user files and avoid the legacy recursive uninstaller.

## Verification posture

[#26](https://github.com/sousouliao/lumiere/issues/26) owns P0 checks and the frozen
133323003-byte installer / 309-file inventory. [#27](https://github.com/sousouliao/lumiere/issues/27)
owns cancellable JSONL transport; [#28](https://github.com/sousouliao/lumiere/issues/28)
owns byte-identical .NET conversion fixtures and real HDR 3840x2160 both-delivery;
[#29](https://github.com/sousouliao/lumiere/issues/29) owns same-frame crop and 30
native Region/cancel/EOF cycles. Their Cargo/schema checks pass.

[#30](https://github.com/sousouliao/lumiere/issues/30) owns P4: `pnpm check`, 32
shared test files / 197 tests, Rust workspace tests, strict Clippy/format, Vite and
native Release build pass. Explicit Windows fixtures verify Host disconnect and
replacement, quiet capture without WebView, real renderer settings IPC, shortcut
rollback/recording, caption maximize/restore/close and 30 WebView open/close cycles.
Warm/final shell handles 478/483, private bytes 14770176/16543744. These counters
exclude Host/WebView2 subprocesses and do not establish total-memory acceptance.
Vendor late-close PostMessage warnings appear without fixture failure.

A running installed baseline's `app.asar` exactly matches the frozen inventory hash.
Its real main-window screenshot now supplements the isolated Electron GPU-failure
record. React layout/source and current main/settings snapshots were reviewed; one
physical client-height pixel preserves baseline viewport rounding at 150% DPI.
All-state pixel parity, other DPI ratios, compositor/physical input, SDR/topology,
sleep/reconnect, clean-machine behavior and installed update acceptance remain open.
Earlier [#22](https://github.com/sousouliao/lumiere/issues/22), [#23](https://github.com/sousouliao/lumiere/issues/23)
and [#24](https://github.com/sousouliao/lumiere/issues/24) evidence/waivers do not
certify the replacement. Baseline +10% hot median/p90 and smaller total memory/artifact
gates still require independent final comparison.

## Next action

Implement P5's safe upgrade transaction, NSIS payload, signed Tauri updater and
matching legacy/new metadata; verify rollback and installed upgrades. Then perform
P6 cleanup, CI/contracts and the remaining final acceptance without publication.