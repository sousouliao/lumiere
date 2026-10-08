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
Rust/Tauri Windows. All local CI-equivalent commands pass with Node 22.23.3 and pinned
Rust; workflow actionlint and first-command failure probes pass. Remote Actions has not
verified this branch. #32 owns exact commands, corrections and final artifact hashes.

Real HDR 3840x2160 both-delivery bytes and same-frame Region crop pass. The 30-cycle
WebView fixture passes with settled native/React state before snapshots; warm/final shell
handles 461/463 and private bytes 14127104/15339520. Vendor late-close PostMessage warnings
remain without fixture failure. Main/settings geometry was reviewed at 150% DPI; complete
state-by-state pixel parity is not established.

After 4 warm captures and 30 alternating resident Display folder captures per Host,
Release Rust median/p90 1119/1176 ms vs frozen baseline 1409/1426 ms: -20.6%/-17.5%.
Both report HDR. The earlier p90 regression was corrected by the exact transfer lookup.
This establishes the observed Host hot path, not physical Region or UI-trigger latency.
Final local installer 3488041 bytes vs baseline 133323003 (97.4% smaller), SHA256
`4423b06cf8983766e990bd4ed6b49d1bbf2fc49f8476abd9074e52e296269105`.
Shell/Host/helper import tables have no VC redistributable imports after static CRT
packaging; Release capture/transaction tests under that configuration pass.

Release GUI acceptance now pauses at matching idle/main/settings/closed states and
uses the Release Host. Five Win32 process-tree samples per state include every live
Host/Electron/WebView2 descendant. Corrected baseline bootstrap explicitly hides its
startup window before idle sampling. At synthetic 150% DPR, baseline/Tauri private
bytes (MiB) are idle 199.22/25.69, main 219.65/221.65, settings 223.52/210.99,
closed 221.89/19.54. Visible main is not a memory improvement. These are isolated
Release integration fixtures, not post-capture peak-memory measurements. The final
shipping executable, with isolated settings, measures idle/main/closed 25.0/219.83/18.23
MiB across its complete process tree; settings bytes and descendant cleanup pass.

Matched-viewport synthetic DPR screenshots establish exact main/settings body pixels
at 100/150/200%. At 125%, equal CSS viewports produce a one-column preview-size
rounding difference and nonzero body differences; it does not pass pixel parity.
Browser previews omit Electron native caption buttons, so full-window chrome and
all interactive states remain unverified. Synthetic DPR does not certify OS DPI changes.

Final NSIS fresh Unicode path, inherited reinstall, official signed updater/Host EOF,
and full legacy inventory failure/retry fixtures pass. A cloned frozen production
Electron client uses its genuine updater IPC/service with a loopback feed to install
private fixture-only 0.6.1, restart and preserve settings/path/unknown files. Six serial
journeys and one final deadline-regression journey pass. Update handoff now waits for
natural exit using fresh RestartManager sessions; a deliberately live old client causes
safe exit 2 in 15.34 seconds without replacement. This does not verify the public feed.
Locked native preparation is shared by CI/packaging; stale manifests/version drift are
rejected and only the official signer signs final installer bytes.

Windows Sandbox and Hyper-V management are disabled (optional-feature state 2);
no usable clean-machine environment is present. The remote migration branch returns
404; latest successful remote CI verifies the old baseline, not this work. No signing
secrets are configured. Clean-machine, public-provider upgrade, physical input/compositor,
SDR/topology, OS DPI and sleep/reconnect remain open. Earlier release waivers do not
certify the replacement; #32 remains open and the product is not release-certified.

The user's installed baseline at `D:\lumiere` remains restored/running after the P5 fixture
incident recorded in #31. All 309 frozen hashes were verified again; original registration
and settings remain intact. Fixed fixtures retain process handles, pin `/D` and wait for
installer commit before restoring the three owned HKCU keys.

## Next action

The unattended local acceptance and pipeline cleanup are complete. The user must perform
physical Region/keyboard/tray, complete UI states at real OS 100/125/150/200% DPI,
SDR/HDR mixed-monitor/hotplug/compositor/clipboard viewing and sleep/hibernate/reconnect
checks, plus a virgin Windows standard-user install/uninstall/reinstall. Remote CI needs
separately authorized push of this source; public-provider update/signing provisioning
needs an authorized release candidate. Record those observations in #32 before closing
acceptance. Do not publish or change the real installed baseline.
