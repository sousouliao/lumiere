# Current Project State

- Updated: 2026-10-08
- Frontier: Windows v0.7.0 is published; P0–P6 migration is maintainer-accepted under [#25](https://github.com/sousouliao/lumiere/issues/25) / [#32](https://github.com/sousouliao/lumiere/issues/32).
- Decision: [ADR 0021](../decisions/0021-windows-only-tauri-rust-migration.md).
- Public release: [v0.7.0](https://github.com/sousouliao/lumiere/releases/tag/v0.7.0), stable Windows x64; tag/source commit `bbbbf902707a268c5730787331ce6248eb84e4d9`.
- Installer: 3485657 bytes; SHA256 `1e9e7c39865a576842c2607ade5b6a7d3d2daf0be98b18324579c97d36a2d264`.
- The maintainer authorized publication and explicitly selected v0.7.0, superseding the earlier unattended no-push/no-publication restriction.

## Product

Tauri 2.12.1 / retained React owns the Windows shell; an independent Rust capture
library and supervised resident JSONL Host own WGC/D3D11/DXGI. Shell/Host remain
resident, while the WebView opens on demand and is destroyed on close. Native tray,
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

Exact commands, measurements, failed-to-fixed attempts and original evidence belong
to [#32](https://github.com/sousouliao/lumiere/issues/32). P0–P5 evidence remains in #26–#31.

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

Maintain the published Windows product through scoped Issues. Future versions follow
the [release contract](../contracts/releases.md) and [runbook](../runbooks/releasing.md);
published bytes/tags are immutable. Do not recreate a migration backlog or reinterpret
maintainer reports and synthetic fixtures as broader platform guarantees.
