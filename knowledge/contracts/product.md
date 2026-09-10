# Product Contract

## Goal

Ship a fast and honest Windows and macOS screenshot tool. The MVP supports region
and display capture, native HDR-aware acquisition, and practical sRGB output for
clipboard and folder workflows without claiming HDR preservation.

## Users

- Windows and macOS users who need fast, reliable screenshots during normal desktop work.
- Designers, engineers, writers, and reviewers who care about credible HDR rendering.
- Power users with HDR and mixed-monitor setups who need honest capability boundaries.

## MVP Scope

- Shared main-window, tray/menu-bar, and configured-shortcut capture entry points.
- Region capture with drag-to-select and release-to-capture behavior.
- Display capture for the active target.
- Platform-native HDR-aware capture through WGC on Windows and ScreenCaptureKit on macOS.
- Clipboard, folder, and both-target output.
- Local output, path, timestamp, shortcut, after-capture, and HDR-alert settings.
- Target-aware HDR readiness states: ready, unavailable, degraded, or unvalidated.
- One official MVP output path: **sRGB Visual Match**.
- Installable application artifacts for Windows and macOS. The Windows MVP uses an unsigned
  traditional setup executable with a custom install path, published checksum, and explicit
  unknown-publisher guidance. The macOS MVP is
  distributed directly as an ad-hoc-signed application with an explicit manual Gatekeeper
  first-launch step; Developer ID signing and notarization are not MVP requirements.

## Product Language

- **HDR-aware MVP** — platform-native capture with honest HDR state; not certification
  and not an HDR-preserved artifact claim.
- **Compatible output** — useful output for common platform consumers; possibly SDR-compatible.
- **sRGB Visual Match** — a native HDR-aware source converted to an SDR-compatible
  sRGB artifact, tuned to avoid obvious washed-out, gray, or blown-out output.
- **Default Visual Match Conversion** — fixed, non-user-adjustable MVP conversion.
- **Artifact success** — the artifact was copied or saved; no fidelity implication.
- **Target-app limitation** — processing introduced by a receiving app after Lumiere
  produced a valid artifact; never an excuse for a Lumiere output defect.
- **HDR-preserved export** — a future named path with documented format, color,
  metadata, viewer assumptions, and real hardware evidence on every claimed platform.

## Out Of Scope For MVP

- Broad or universal HDR-preserved export guarantees.
- P3 or HDR10 as normal supported user-selectable modes.
- General image editing, annotations, gallery/history, cloud upload, or sharing.
- Onboarding or telemetry.
- Universal display-topology, viewer, DPI/Retina, accessibility, or long-run certification.

## Definition Of Product Success

- The app launches, captures repeatedly, cancels, and exits cleanly on supported
  Windows and macOS versions.
- Region and display capture do not leave stale overlay or capture state.
- Clipboard and folder outputs are usable in named common consumers on both platforms.
- Supported HDR scenes avoid blocking visual failures defined by the claims contract.
- HDR status does not claim more than Lumiere can prove for the active target.
- The MVP installs, launches, upgrades, uninstalls, and reinstalls on clean Windows
  and macOS machines using the platform's supported distribution path.
