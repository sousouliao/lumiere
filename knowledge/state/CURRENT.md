# Current Project State

- Updated: 2026-09-11
- Milestone: 1 — Cross-platform HDR-aware MVP with sRGB Visual Match
- Posture: native capture and shared product surface complete; Windows distribution remains unsigned preview-only.
- Issues #12, #15, and #16 were resolved on Windows on 2026-09-11; #12 ended as not planned after the signing route failed.

## Current Position

Electron/React drives Swift and .NET Hosts through platform-host v5 JSON Lines.
Display and frozen-frame Region capture support Clipboard, Folder, and Both delivery.
Main owns persisted output, save-directory, shortcut, after-capture, and HDR-reminder
settings. Region uses a logical-resolution preview and crops the retained backing frame;
the reusable Overlay and native preparation run in parallel. During Region selection,
pointer display changes serially replace the native frozen session, preview, and Overlay
bounds through Host-issued opaque target tokens.
The desktop shell starts resident without showing its main window: macOS runs as a
menu-bar-only accessory app, while Windows retains the corresponding tray behavior in
source pending current runtime verification. Explicit open/settings actions reveal the
prewarmed window; missing macOS Screen Recording permission is the startup exception and
shows guidance without automatically triggering the system prompt.

Foundation and milestone 1A–1C are recorded complete in
[#1](https://github.com/Mournerliao/lumiere/issues/1),
[#4](https://github.com/Mournerliao/lumiere/issues/4),
[#7](https://github.com/Mournerliao/lumiere/issues/7), and
[#9](https://github.com/Mournerliao/lumiere/issues/9).
HDR-preserved export and broader cross-platform fidelity certification remain unstarted.

## Verification Boundary

- **Quiet failure feedback — [#16](https://github.com/sousouliao/lumiere/issues/16):**
  independent Toast removed; success/cancellation stay quiet, background failures use
  silent system notifications, foreground failures use a footer summary with fixed-size
  details, blocking recovery replaces the capture actions, and all recovery shares one
  capture busy state. `pnpm check`, `pnpm test:shared` (27 files, 139 tests), and
  `pnpm build` pass on this Mac.
  Electron fixture checks cover notification routing/clicks, closed-window recovery,
  stale actions, folder/permission recovery, and cancellation; renderer unit checks cover
  fixed-size notice placement. Packaged macOS Display/Region output, cancellation, native
  notification presentation, and notification click recovery pass. On Windows, an isolated
  packaged `0.4.0` run produced a real background partial result: clipboard delivery passed,
  folder delivery failed against a deliberately non-directory target, and the app returned
  the expected folder-recovery notice through the system-notification path.
- **macOS runtime:** recorded Display/Region delivery, settings persistence, cancellation,
  frozen-frame commit, external-4K backing geometry, and bounded repeat checks pass.
  Region latency on the named SDR target was 607 ms cold and 343.5 ms warm median
  (311–415 ms). Built-in Retina XDR geometry still needs observation after the correction.
- **macOS manual update check:** System settings checks the latest stable GitHub Release
  through typed main/preload IPC, reports idle/checking/current/available/failure states,
  and opens the fixed releases page for manual download. `pnpm check`, `pnpm test:shared`
  (28 files, 151 tests), `pnpm test:macos` (2 files, 6 tests), the macOS Host suite
  (34 tests), and `pnpm build` pass on this Mac. The development runtime reported
  `0.2.0 · Up to date` before `v0.3.0` publication; installed-release behavior is not yet
  separately observed.
- **Multi-display Region repository slice:** platform-host v4 target tokens, the shared
  display watcher, switching state, preview/session replacement, and stale-generation guards
  are implemented. `pnpm test:shared` (27 files, 138 tests), `pnpm build`, and the macOS
  Host suite (34 tests) pass on this Mac. The maintainer accepted real dual-display switching;
  Windows build/runtime remains unverified.
- **Windows runtime:** `hosts/windows/scripts/verify.ps1` passes with Host 33, Capture 85,
  Graphics 48, and Interop 35 tests. On the named 3840×2160 HDR display at 150% scaling,
  optimized Region preparation produced 2560×1440 previews: 950 ms cold and 666 ms warm
  median across ten samples, versus 1821 ms cold and 2196.5 ms warm median from the installed
  protocol-v3 baseline. Ten prepare/cancel cycles released successfully; a final 100×100
  logical commit produced a 150×150 sRGB Visual Match PNG from the retained HDR frame.
- **Distribution:** macOS `v0.4.0` was published from `0ac3eaa` with separate arm64/x64
  DMGs and menu-bar-only silent startup. CI audit, shared checks, macOS and Swift Host
  tests, both builds, signatures, architecture checks, publication, and downloaded
  public-release checksums pass. The packaged arm64 candidate showed permission guidance
  without an automatic system prompt when Screen Recording access was missing and remained
  resident after its window closed. The prior `v0.3.2` arm64 permission grant, one-relaunch
  recovery, and Display/Region observations remain the latest granted-state runtime
  evidence. The x64 app and Host passed build, signing, and architecture checks but did not
  receive a separate permission or resident-lifecycle runtime observation.
  Windows unsigned `v0.2.0-preview.1` publication and named-machine installer lifecycle
  are recorded. A local unsigned `0.4.0` installer also builds successfully. SignPath
  Foundation declined the application on 2026-09-09, so signed sparse identity, borderless
  consent, automatic updates, and a stable Windows release are deferred and unclaimed.
- **Fidelity and CI:** named macOS bright/dark fixtures and one Windows HDR-target sRGB
  reference passed. These do not certify broad fidelity or HDR preservation. The last
  recorded shell/engine CI checkpoint is `fccf812`, not current-HEAD evidence.

Exact prior commands, measurements, and platform qualifications remain available in the
[Git version before this condensation](https://github.com/Mournerliao/lumiere/blob/e1b48d1dd691ab567b376d1ba27edf27428ba448/knowledge/state/CURRENT.md#verification-truth).
Owning Issues hold acceptance criteria; new verification belongs there rather than in a
running history here.

## Execution Frontiers

No GitHub implementation Issue is currently open. Signed Windows distribution is deferred
by ADR 0017 rather than treated as completed. Remaining broad fidelity, multi-display Windows
observation, fresh-machine preview installation, and HDR-preserved export are not implied by
the closed slices and should receive new owning Issues when selected.
