# Current Project State

- Updated: 2026-10-09
- Frontier: Windows v0.7.1 remains published and maintainer-verified; unreleased current-user launch-at-login is implemented and locally checked under [#33](https://github.com/sousouliao/lumiere/issues/33), awaiting maintainer login/Startup Apps acceptance. P0–P6 migration remains accepted under [#25](https://github.com/sousouliao/lumiere/issues/25) / [#32](https://github.com/sousouliao/lumiere/issues/32).
- Decision: [ADR 0021](../decisions/0021-windows-only-tauri-rust-migration.md).
- Public release: [v0.7.1](https://github.com/sousouliao/lumiere/releases/tag/v0.7.1), stable Windows x64; tag/source commit `8f6184d3fc034d7d6177a1f6c2d703a38ee0e140`.
- Installer: 3524429 bytes; SHA256 `699b308656bb72fcdc654f77d7d069e0ec939d10953f673158da06f7d20ab4b3`.
- The maintainer authorized publication and reported the installed latest release working without issues on 2026-10-09.

## Product

Tauri 2.12.1 / retained React owns the Windows shell; an independent Rust capture
library and supervised resident JSONL Host own WGC/D3D11/DXGI. Shell/Host remain
resident, while the main WebView opens on demand and is destroyed on close. The dark
tray menu WebView is preloaded and reused; its icon follows Windows system theme
notifications without polling. Native tray,
shortcuts, quiet capture and v1–v5 settings migration are retained. No Electron,
.NET or macOS implementation/resource/stub remains; Git owns history.

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

Unreleased #33: five scoped Rust tests, 16 SettingsView tests, desktop TypeScript,
scoped ESLint/Clippy, formatting, renderer/native Release builds, and the explicit
Release autostart WebView fixture pass locally. Actual IPC toggles, synthetic Windows
approval records, focus refresh, failure readback and four visual states were checked.
Local NSIS update fixtures preserve enabled/blocked registration; uninstall removes
owned startup values and retains unknown files. Original registration/settings were
restored. Exact commands and remaining real login/Windows Startup Apps observations
are owned by #33. The maintainer authorized v0.8.0 publication before those manual
observations and will test the installed public build; this is an explicit deferral,
not completed platform evidence. No public release includes this feature yet.

The v0.7.1 patch evidence is:

- [Release 37827856922](https://github.com/sousouliao/lumiere/actions/runs/37827856922) succeeded for the tagged commit, including shared checks, Windows Rust checks/build and updater signature verification.
- Downloaded all five public assets; their SHA256 digests match GitHub metadata. The installer matches `SHA256SUMS`, both updater feeds identify v0.7.1, legacy SHA512 matches the downloaded installer, and the public tag targets the release commit. Release is non-draft/non-prerelease.
- Local `cargo test --locked -p lumiere-desktop tray_` passed 3 tests; `cargo test --locked -p lumiere-capture-windows` passed 7 with 2 hardware tests ignored; `pnpm --filter @lumiere/desktop test:shared` passed 41 tests. TypeScript, scoped ESLint, build, Clippy and formatting passed. Local full lint/format excluded ignored `.cache` tooling; remote shared checks passed unchanged.
- Before publication the maintainer confirmed Region, reviewed tray actions and theme changes; after publication they installed the latest release and reported no issues. This is maintainer evidence, not an agent hardware observation or a wider platform claim.

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

Publish Windows v0.8.0, then complete maintainer sign-out/sign-in and Windows Startup
Apps disable/restore acceptance under #33. Future versions follow
the [release contract](../contracts/releases.md) and [runbook](../runbooks/releasing.md);
published bytes/tags are immutable. Do not recreate a migration backlog or reinterpret
maintainer reports and synthetic fixtures as broader platform guarantees.
