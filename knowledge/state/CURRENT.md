# Current Project State

- Updated: 2026-10-08
- Milestone: 1 — Cross-platform HDR-aware MVP with sRGB Visual Match
- Public distribution: `v0.5.0`, including unsigned Windows x64 NSIS; not replaced or republished.
- Windows candidate: `0.6.0`, update implementation at `88e1d60`; not ready for public finalization while applicable native/hardware gates remain open.

## Current Position

Electron/React supervises Swift and .NET Hosts through JSON Lines. Capabilities and
Display use v5; native Region uses v6 and request-id cancellation on both platforms.
Native Region selects/crops the same full-resolution frozen Visual Match frame.
The Windows Electron Region preview remains only an explicit diagnostic fallback;
shared migration cleanup waits for independent platform acceptance under #19.

ADR 0020 enables unsigned Windows in-app updates independently of signing and sparse
identity. Packaged builds check periodically, download only on request, show progress,
and silently install/relaunch only on explicit restart after capture and Host shutdown.
Ordinary quit does not install. macOS keeps manual release-page updates. Public v0.5.0
needs one manual upgrade to obtain this capability. Windows still has unsigned warnings,
application-control restrictions and the WGC capture border; HDR-preserved export and
broad fidelity certification remain unstarted.

## Verification Boundary

- Windows candidate repository gates: `pnpm install --frozen-lockfile`, `pnpm check`,
  `pnpm test:shared` (34 files, 204 tests), `pnpm build` via packaging, and
  `pnpm package:windows` pass. Native `verify.ps1` passes Host 41, Capture 93,
  Graphics 50 and Interop 35 tests. These are local results, not new cross-platform CI.
- Installed updates: real local-source `0.6.0` → test-only `0.6.1` rejects a wrong
  SHA-512, supports network retry/progress, refuses install during capture and retains
  settings. The maintainer physically clicked download/restart and confirmed successful
  `0.6.1` launch; the installed process uses `--updated`. Default/custom-path installation,
  uninstall cleanup and reinstall were observed on this development machine. Public-source
  observation and non-development Windows installation are distinct, outstanding gates.
- RTX 5080, named 3840×2160 HDR target at 150%: packaged drag, Escape/right-click,
  too-small selection, 60-second expiry, clipboard/folder/both, partial delivery and
  Host-loss recovery observed. A red-to-blue underlying-window fixture retained the red
  frozen crop, yielding a 450×225 sRGB PNG from a 300×150 logical selection.
- 100 mixed cycles: 74 successes, 25 cancellations, one clipboard partial delivery
  with successful file output; later captures recovered. A 30+ minute resident run
  includes controlled Host replacement and settled memory/handle observations. Available
  GPU memory checkpoints were stable; this is bounded evidence, not universal leak exclusion.
- 30 hot injected-shortcut samples with screen-recording corroboration: software
  callback → visible/foreground HWND p90 50 ms (26–59 ms). This does not certify the
  compositor-visible/input-ready endpoint or physical-key latency. Windows SDR/scaling
  coverage beyond the named target, multi-display/negative coordinates and sleep/reconnect
  remain unaccepted. Standalone development preview also hit Electron GPU-process crashes;
  the no-self-update policy passes configuration tests, but normal preview runtime is unverified.

## Execution Frontiers

- [#24](https://github.com/sousouliao/lumiere/issues/24): local installed update acceptance
  completed; no public publication. Next release work must observe the real GitHub feed.
- [#20](https://github.com/sousouliao/lumiere/issues/20): current Windows shared seam checks
  pass; record current macOS shared checks before closing its remaining cross-platform gate.
- [#22](https://github.com/sousouliao/lumiere/issues/22): finish independent Windows
  display/hardware and compositor timing acceptance; do not substitute Mac multi-display evidence.
- [#23](https://github.com/sousouliao/lumiere/issues/23): finish non-development-machine,
  sleep/reconnect and remaining desktop acceptance before release preparation.
- [#21](https://github.com/sousouliao/lumiere/issues/21) and parent
  [#19](https://github.com/sousouliao/lumiere/issues/19): macOS native selection, hardware,
  timing and Intel SDR acceptance remain independent; retain the staged shared fallback.

Exact commands, artifact hashes, devices, observations and remaining criteria belong to
their owning Issues. Separate UI-only footer removal remains unstaged and preserved.
