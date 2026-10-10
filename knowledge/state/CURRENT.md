# Current Project State

- Updated: 2026-10-10
- Source frontier: configurable main-window hiding during capture under [#35](https://github.com/sousouliao/lumiere/issues/35) is implemented with scoped repository checks; Windows native build/runtime acceptance remains pending.
- Prior source slice: snapshot synchronization, typed Host-result and native diagnostic cleanup under [#34](https://github.com/sousouliao/lumiere/issues/34) is repository-checked as scoped below. The maintainer owns Windows native shell build/tests and real runtime acceptance; this source is not a new published release.
- Frontier: Windows v0.8.0 is published with current-user launch-at-login under [#33](https://github.com/sousouliao/lumiere/issues/33); maintainer installation/login/Startup Apps acceptance remains pending by explicit authorization. P0–P6 migration acceptance under [#25](https://github.com/sousouliao/lumiere/issues/25) / [#32](https://github.com/sousouliao/lumiere/issues/32) remains unchanged.
- Decision: [ADR 0021](../decisions/0021-windows-only-tauri-rust-migration.md).
- Public release: [v0.8.0](https://github.com/sousouliao/lumiere/releases/tag/v0.8.0), stable Windows x64; tag/source commit `6805dd6fb415ef00e1c5523a482b4ea72f1dfd1a`.
- Installer: 3545566 bytes; SHA256 `682ac6fe732e7b989686806d589ca8f3834d0ef08e26bbf0f97db5a0568a2dd1`.
- The maintainer authorized v0.8.0 publication on 2026-10-09 and will install it to test; the latest completed installed-release observation remains v0.7.1.

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
The reported intermittent translucency is consistent with the old unsynchronized
hide/capture sequence; Windows observation must confirm the cause and resulting behavior.

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

[#35](https://github.com/sousouliao/lumiere/issues/35): SettingsView passes 20 tests,
desktop TypeScript and renderer build pass, and scoped ESLint/Prettier/Rustfmt and
`git diff --check` pass on macOS. A temporary crate loading production settings.rs
passes two tests covering v1–v5 migration, v6 save/load and boolean validation.
A Windows-target source fixture compiles capture_window.rs against windows 0.62.2
with a minimal Tauri interface shim; this checks API signatures, not the real Tauri
runtime. Static renderer previews at 480×370 and 440×340 retain the existing layout;
Capture content scrolls when needed. Full shell `cargo check --locked --offline -p
lumiere-desktop --target x86_64-pc-windows-msvc` stops at uncached aho-corasick 1.1.5.
The existing Windows GUI fixture now checks persisted on/off choices and Display
window restoration, but it has not been executed here. Native build/tests, actual
on/off capture pixels, Region cancellation/failure and absence of translucent remnants
remain maintainer Windows acceptance in #35.

[#34](https://github.com/sousouliao/lumiere/issues/34) source cleanup: one renderer observer owns asynchronous listener readiness,
initial snapshot/event ordering and cleanup; capture-contract owns decoded result types
and validation, which remain typed through shell projection. Native failure stages/HRESULT
and Host request/delivery diagnostics use stderr without changing JSONL output.
On macOS, `pnpm test:shared` passes 57 tests; desktop TypeScript, renderer build,
scoped ESLint/Prettier/Rustfmt and `git diff --check` pass. Contract tests pass 8 tests with
`RUSTUP_TOOLCHAIN=stable cargo test --locked --offline -p lumiere-capture-contract -- --skip current_windows_requests_conform`;
the skipped existing test hardcodes Windows paths and remains for Windows. Windows-target
`cargo check` and Clippy for capture-windows/Host (including tests) pass without executing
Windows code. A temporary source-based shell transport/projection fixture passes 5 tests;
one real-child test remains ignored. Independent read-only review found no blocking
regression; its missing busy-capture diagnostic was fixed. Full Tauri native build/tests
and GUI/capture observations remain unverified by the agent and assigned to the maintainer.

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

The v0.8.0 publication evidence is:

- [Release 37946086840](https://github.com/sousouliao/lumiere/actions/runs/37946086840) succeeded for the tagged commit, including shared checks, Windows Rust checks/build and updater signature verification. Local shared tests pass 47 tests.
- Downloaded all five public assets; their SHA256 digests and sizes match GitHub metadata. The installer matches `SHA256SUMS`; both updater feeds identify v0.8.0, legacy SHA512 matches the installer, and Tauri metadata matches the detached signature and versioned installer URL.
- The public tag targets the release commit; release is non-draft/non-prerelease, and the latest-download Tauri endpoint returns v0.8.0 with the same signature. Authenticode remains NotSigned.
- Maintainer installation and real login/Startup Apps observations remain open in #33; synthetic approval records and source checks do not close them.

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

Under [#35](https://github.com/sousouliao/lumiere/issues/35), build/test the current source on Windows,
run `native_shell_webview_lifecycle` explicitly with its isolated fixture directory,
and verify Display/Region with hiding on/off, repeated capture, cancellation/failure,
quiet/minimized/closed windows and translucent remnants. Continue the existing #34
snapshot/diagnostic acceptance in the same Windows pass.

Under [#34](https://github.com/sousouliao/lumiere/issues/34), build the current source on Windows, run the native checks in the Windows runbook,
and verify settings/update state while opening/closing the WebView, Display/Region
capture and cancellation, partial delivery, and structured failure diagnostics.

Install published v0.8.0 and complete maintainer sign-out/sign-in and Windows Startup
Apps disable/restore acceptance under #33. Future versions follow
the [release contract](../contracts/releases.md) and [runbook](../runbooks/releasing.md);
published bytes/tags are immutable. Do not recreate a migration backlog or reinterpret
maintainer reports and synthetic fixtures as broader platform guarantees.
