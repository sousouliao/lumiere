# Current Project State

- Updated: 2026-10-12
- Frontier: Windows v0.9.1 is published with a taller minimum/default window, warm-white settings status badges, disabled main-window context menus, and compact update controls with loading indicators and explicit download/restart actions.
- Shared checks and the scoped Windows Release WebView fixture pass; the v0.9.1 release workflow independently passes all jobs. The maintainer's installed v0.9.0 acceptance on 2026-10-11 remains evidence for their tested scope, not installed v0.9.1 acceptance or a wider hardware matrix.
- Current-user launch-at-login under [#33](https://github.com/sousouliao/lumiere/issues/33) retains explicitly deferred real login/Startup Apps acceptance. P0–P6 migration acceptance under [#25](https://github.com/sousouliao/lumiere/issues/25) / [#32](https://github.com/sousouliao/lumiere/issues/32) remains unchanged.
- Decision: [ADR 0021](../decisions/0021-windows-only-tauri-rust-migration.md).
- Public release: [v0.9.1](https://github.com/sousouliao/lumiere/releases/tag/v0.9.1), stable Windows x64; tag/source commit `5edfd33d77e11a390d0c5996f121edd052fd968f`.
- Installer: 3578548 bytes; SHA256 `74fceab28a81f0072b690490c0b867adc968b87a3abc53dbdf8f100b49a0ef07`.

## Product

Tauri 2.12.1 / retained React owns the Windows shell; an independent Rust capture
library and supervised resident JSONL Host own WGC/D3D11/DXGI. Shell/Host remain
resident, while the main WebView opens on demand and is destroyed on close. The dark
tray menu WebView is preloaded and reused; its icon follows Windows system theme
notifications without polling. Native tray,
shortcuts, quiet capture and v1–v5 settings migration are retained. No Electron,
.NET or macOS implementation/resource/stub remains; Git owns history.

System settings offers Launch at login, off by default, using only current-user
Windows registration. Login startup stays tray-only. Windows-disabled startup is
reported without overriding it; unknown approval records remain unavailable.

Capture settings offers Hide Lumiere during capture, on by default, persisted in v6
settings with v1–v5 migration. The shell hides an originally visible, non-minimized
main window on its owning thread without DWM transitions and waits for composition
before dispatching capture. Display and Region both restore that window after any
outcome; background capture creates no main WebView. Off retains the visible window.
Framework visibility and native hiding are synchronized so restoration can show the
window again. Debug and Release GUI fixtures verify scoped restoration behavior.

Windows v5 Display and v6 same-frame Region/cancellation retain floating-point
acquisition and fixed sRGB Visual Match, with the same PNG for clipboard and folder.
No HDR-preserved export, borderless capture or wider compositor-fidelity claim.
Installer Authenticode status remains NotSigned; updater integrity signatures do
not establish Windows publisher identity.

Updates check after 30 seconds and every 6 hours; download and restart installation
are explicit, and ordinary quit does not install. Release System settings retains
check/progress/retry/restart controls. Debug intentionally disables update operations.
Both update feeds reference one NSIS; Tauri verifies minisign bytes and signed versions.
Natural-exit handoff, transactional replacement/rollback and explicit legacy cleanup
preserve installation location, settings and unknown files. Private keys stay outside
Git; matching encrypted Actions signing secrets are configured.

## Verification

v0.9.1 UI maintenance: `pnpm check`, `pnpm test:shared` (61 tests), and `pnpm build` pass locally. With `LUMIERE_GUI_CYCLES=3`, `cargo test --locked --release -p lumiere-desktop --features custom-protocol native_shell_webview_lifecycle -- --ignored --nocapture` passes. The fixture observes the Capture settings fitting at 440×400 logical pixels, warm-white badges, canceled context-menu events, and five scripted updater states in the real Windows WebView. Scripted states do not download or install an update. An Edge renderer check with mocked IPC separately verifies Check → Checking → new version → Downloading → Restart to update → Restarting, spinner rotation, and action dispatch.

[Release 38159859710](https://github.com/sousouliao/lumiere/actions/runs/38159859710) passes Audit, Build Windows, and Publish for the tagged source. All five downloaded public assets match GitHub sizes/digests; the installer matches `SHA256SUMS`, both updater feeds identify the same bytes/version, and the latest-download feed matches the versioned signature. Public notes match the finalized changelog; release is non-draft/non-prerelease and its tag targets the release commit. `target/release/deps/Lumiere-7edc4410401e9982.exe official_updater_accepts_signed_bytes_and_rejects_tampering --ignored --nocapture` passes against downloaded v0.9.1 bytes using the unchanged verifier built for the UI fixture. Authenticode remains NotSigned.

CI dependency caching at `18f265b` is verified by successful Windows CI
[cold](https://github.com/sousouliao/lumiere/actions/runs/38133340528/attempts/1) and
[cache-hit](https://github.com/sousouliao/lumiere/actions/runs/38133340528/attempts/2)
runs of the same commit: job time 16m56s → 4m42s, native build 7m53s → 2m22s,
Rust gates 6m26s → 32s, including 50s cache restoration in the second run.
All existing gates ran. Locally, `cargo test --locked --workspace --features
lumiere-desktop/custom-protocol official_updater_accepts_signed_bytes_and_rejects_tampering
-- --ignored --nocapture` passed against the staged signed installer and reused the
test build (0.31s preparation, 0.59s test). Release-workflow elapsed time remains
unmeasured after this change; observe it at the next authorized release.

[#36](https://github.com/sousouliao/lumiere/issues/36): the native FP16/scRGB preview
fixture preserves unmasked HDR/negative texture values and byte-identical PNG output.
Selection-only masking persists through capture feedback; theme-colored crosshairs
disappear on release. WGC
support/target reuse passes resource fixtures with unchanged budgets and bounded
post-release sampling. Optimized development capture/codecs remove the measured Debug
conversion/encoding delay. Clipboard retries only transient access contention for at
most one second; controlled contention/timeout and repeated 4K delivery pass without
an unconditional delay. Exact commands, measurements and hardware scope are in the Issue.

[#35](https://github.com/sousouliao/lumiere/issues/35): Windows
`native_shell_webview_lifecycle` passes three cycles in Debug and Release, checking
persisted hiding on/off, Display restoration, Region cancellation/restoration,
minimized state and main-WebView destruction. Framework visibility synchronization
fixes the hidden-window restoration regression. These fixtures do not establish every
physical shortcut, failure mode or compositor/hardware combination.

[#34](https://github.com/sousouliao/lumiere/issues/34): current Windows checks cover
snapshot ordering/listener cleanup, typed Host-result projection and stderr diagnostics.
Local default Rust tests pass 43 tests, shared tests pass 61 tests, and eight explicit
native capture/resource/supervision tests pass. `pnpm check`, workspace Clippy with
`lumiere-desktop/custom-protocol`, Rustfmt, schema cases and renderer/native builds pass.
The release workflow independently passes its audit and Windows build gates.

#33: five scoped Rust tests, 16 SettingsView tests, desktop TypeScript,
scoped ESLint/Clippy, formatting, renderer/native Release builds, and the explicit
Release autostart WebView fixture pass locally. Actual IPC toggles, synthetic Windows
approval records, focus refresh, failure readback and four visual states were checked.
Local NSIS update fixtures preserve enabled/blocked registration; uninstall removes
owned startup values and retains unknown files. Original registration/settings were
restored. Exact commands and remaining real login/Windows Startup Apps observations
are owned by #33. The maintainer authorized v0.8.0 publication before those manual
observations and will test the installed public build; this is an explicit deferral,
not completed platform evidence. The public v0.8.0 notes disclose that pending check.

The v0.9.0 publication evidence is:

- [Release 38131166684](https://github.com/sousouliao/lumiere/actions/runs/38131166684) succeeded for the tagged commit; Audit, Build Windows and Publish all completed successfully.
- Downloaded all five public assets; SHA256 digests and sizes match GitHub metadata. The installer matches `SHA256SUMS`; both updater feeds identify v0.9.0, legacy SHA512 matches the installer, and Tauri metadata matches the detached signature and versioned installer URL.
- The public tag targets the release commit; release is non-draft/non-prerelease, and the latest-download Tauri endpoint returns v0.9.0 with the same signature. Authenticode remains NotSigned.
- `official_updater_accepts_signed_bytes_and_rejects_tampering` passes against the downloaded public installer/metadata, validating signed bytes and rejecting tampered bytes/signature/version.

The following v0.7.0 migration evidence remains owned by
[#32](https://github.com/sousouliao/lumiere/issues/32); P0–P5 evidence remains in #26–#31:

- [Windows CI 37795791946](https://github.com/sousouliao/lumiere/actions/runs/37795791946) and [Release 37796168720](https://github.com/sousouliao/lumiere/actions/runs/37796168720) succeeded for the tagged commit; shared tests are 5 files / 41 tests.
- Public release is non-draft/non-prerelease. Five assets, tag target, SHA256 manifest, SHA512 legacy metadata, signed Tauri metadata and latest-download aliases agree. Official verification of downloaded public bytes rejects tampered bytes/signature/version.
- A cloned frozen production 0.6.0 client using its original GitHub provider discovers/downloads 0.7.0; downloaded cache SHA matches the public installer. This observation did not invoke installation. Local full legacy upgrade and four v0.7.0 installer/updater fixtures already pass.
- Release WebView manual checking against the public endpoint returns `up-to-date` with the complete snapshot and visible `Check again`; pre-publication failure/retry also passed. GUI fixtures use an isolated singleton identity and settings; no user development instance was closed.
- The maintainer reported all outstanding physical input, OS DPI/UI-state, SDR/HDR/multi-display/topology, sleep/reconnect and clean-machine checks completed without issues on 2026-10-08. This is maintainer evidence, not agent observation or an invented hardware matrix.
- P6 automated synthetic-DPR body pixels match at 100/150/200%; 125% retains a one-column rounding difference and nonzero differences. That raw result remains unchanged and does not claim complete-window/all-state pixel equality.

P6 process-tree memory and hot-path comparisons establish their recorded fixture/
hardware scope, not universal performance or public-binary peak memory. The user's
`D:\lumiere` baseline still matches all 309 frozen file hashes; fixture registration
backups are restored and no isolated installer/legacy-download processes remain.

## Next action

The maintainer can verify the installed v0.9.1 settings and updater UI; local WebView observations do not replace installed-release acceptance.
Confirm the explicitly deferred sign-out/sign-in and Windows Startup Apps disable/restore
observations under #33 separately; the general usage report does not specify those steps.
Future versions follow the
[release contract](../contracts/releases.md) and [runbook](../runbooks/releasing.md);
published bytes/tags are immutable. Do not recreate a migration backlog or reinterpret
maintainer reports and synthetic fixtures as broader platform guarantees.
