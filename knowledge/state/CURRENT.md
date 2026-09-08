# Current Project State

- Updated: 2026-09-07
- Milestone: 1 — Cross-platform HDR-aware MVP with sRGB Visual Match
- Posture: native capture and shared product surface complete; Windows distribution pending.
- Frontier Issues #12, #14, #15, and #16 were checked on GitHub on 2026-09-07.

## Current Position

Electron/React drives Swift and .NET Hosts through platform-host v4 JSON Lines.
Display and frozen-frame Region capture support Clipboard, Folder, and Both delivery.
Main owns persisted output, save-directory, shortcut, after-capture, and HDR-reminder
settings. Region uses a logical-resolution preview and crops the retained backing frame;
the reusable Overlay and native preparation run in parallel. During Region selection,
pointer display changes serially replace the native frozen session, preview, and Overlay
bounds through Host-issued opaque target tokens.

Foundation and milestone 1A–1C are recorded complete in
[#1](https://github.com/Mournerliao/lumiere/issues/1),
[#4](https://github.com/Mournerliao/lumiere/issues/4),
[#7](https://github.com/Mournerliao/lumiere/issues/7), and
[#9](https://github.com/Mournerliao/lumiere/issues/9).
HDR-preserved export and broader cross-platform fidelity certification remain unstarted.

## Verification Boundary

- **Current shared-shell slice — [#16](https://github.com/sousouliao/lumiere/issues/16):**
  independent Toast removed; success/cancellation stay quiet, background failures use
  silent system notifications, foreground failures use a footer summary with fixed-size
  details, blocking recovery replaces the capture actions, and all recovery shares one
  capture busy state. `pnpm check`, `pnpm test:shared` (25 files, 129 tests), and
  `pnpm build` pass on this Mac.
  Electron fixture checks cover notification routing/clicks, closed-window recovery,
  stale actions, folder/permission recovery, and cancellation; renderer unit checks cover
  fixed-size notice placement. Real macOS Host Display
  output and Region cancellation pass. Fixture notification checks do not establish OS
  banner delivery; packaged macOS notification presentation and Windows remain unverified.
- **macOS runtime:** recorded Display/Region delivery, settings persistence, cancellation,
  frozen-frame commit, external-4K backing geometry, and bounded repeat checks pass.
  Region latency on the named SDR target was 607 ms cold and 343.5 ms warm median
  (311–415 ms). Built-in Retina XDR geometry still needs observation after the correction.
- **Multi-display Region repository slice:** platform-host v4 target tokens, the shared
  display watcher, switching state, preview/session replacement, and stale-generation guards
  are implemented. `pnpm test:shared` (27 files, 138 tests), `pnpm build`, and the macOS
  Host suite (34 tests) pass on this Mac. Real dual-display switching and Windows build/runtime
  remain unverified.
- **Windows runtime:** recorded HDR/SDR capture, independent delivery outcomes, settings,
  cancellation, and lifecycle checks pass. Those observations predate the outstanding
  Region performance slice and do not verify its current Windows implementation.
- **Distribution:** macOS `v0.1.0` Universal release lifecycle is recorded verified on the
  named Mac. Current source builds separate arm64/x64 apps and DMGs; construction,
  signatures, launch, and Host handshake passed, but split-artifact capture and repeat
  behavior remain unverified because the new bundle paths lacked TCC permission.
  Windows unsigned `v0.2.0-preview.1` publication and named-machine installer lifecycle
  are recorded; fresh-machine installation of the published artifact remains unverified.
- **Fidelity and CI:** named macOS bright/dark fixtures and one Windows HDR-target sRGB
  reference passed. These do not certify broad fidelity or HDR preservation. The last
  recorded shell/engine CI checkpoint is `fccf812`, not current-HEAD evidence.

Exact prior commands, measurements, and platform qualifications remain available in the
[Git version before this condensation](https://github.com/Mournerliao/lumiere/blob/e1b48d1dd691ab567b376d1ba27edf27428ba448/knowledge/state/CURRENT.md#verification-truth).
Owning Issues hold acceptance criteria; new verification belongs there rather than in a
running history here.

## Execution Frontiers

- **Quiet failure feedback — [#16](https://github.com/sousouliao/lumiere/issues/16):**
  shared implementation and local macOS capture checks complete. Next: observe native
  notification presentation/click behavior in packaged macOS and Windows builds,
  including platform busy/recovery behavior. The Issue remains open for these checks.

- **Region performance — [#15](https://github.com/sousouliao/lumiere/issues/15):** macOS
  latency and full-resolution commit are recorded verified. Next: Windows build, runtime,
  latency, and resource-stability verification for the optimized preview path.
- **Windows distribution — [#12](https://github.com/sousouliao/lumiere/issues/12):** the
  [SignPath Foundation application was submitted on 2026-09-05](https://github.com/sousouliao/lumiere/issues/12#issuecomment-5551597700).
  Next: await approval and the assigned certificate Subject/Publisher, then
  verify signing, production sparse identity, updates, borderless/fallback behavior,
  and clean-machine lifecycle.
- **macOS distribution — [#14](https://github.com/Mournerliao/lumiere/issues/14) complete:**
  retain the published Universal release evidence; before releasing split artifacts,
  authorize their bundle paths and verify packaged capture, delivery, and repeat behavior.
- **Milestone exit:** blocked until both distribution lanes satisfy their independent
  criteria. Repository success, artifact delivery, visual match, and HDR preservation
  remain separate claims; one platform never verifies another.
