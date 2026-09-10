# Changelog

All notable user-visible changes to Lumiere are documented in this file.

## [Unreleased]

Target version: `0.4.0`

Release platforms: macOS

### Changed

- Run Lumiere as a menu-bar app that starts silently without a Dock icon or main window; open the interface explicitly from `Open Lumiere` or `Settings…`.
- Keep menu-bar capture commands and global shortcuts available after the main window is closed.
- Show the permission recovery interface on launch whenever Screen Recording access is missing, while leaving the system permission prompt behind an explicit user action.

### Known limitations

- The app is ad-hoc signed and not notarized, so first launch requires the documented manual Gatekeeper exception.
- HDR-preserved export is not supported; the official output is sRGB Visual Match.
- This release includes macOS artifacts only; the updated Windows tray behavior has not yet received current runtime verification.

## [0.3.2] - 2026-09-10

Release platforms: macOS

### Fixed

- Request Screen Recording access directly after a permission reset, so Lumiere appears in System Settings without requiring a screenshot shortcut.
- Recognize the first post-grant relaunch as the required restart instead of asking users to restart Lumiere twice.

## [0.3.1] - 2026-09-10

Release platforms: macOS

### Fixed

- Recover Screen Recording access after an app upgrade with a guided, Lumiere-only permission reset and restart flow.

### Known limitations

- The app is ad-hoc signed and not notarized, so first launch requires the documented manual Gatekeeper exception.
- HDR-preserved export is not supported; the official output is sRGB Visual Match.
- Updating the ad-hoc-signed app can still require granting Screen Recording permission again; Lumiere now guides the recovery after a failed capture.

## [0.3.0] - 2026-09-09

Release platforms: macOS

### Added

- Check for a newer Lumiere version from System settings and open the latest GitHub Release for manual download.

### Known limitations

- Update checks and downloads require access to GitHub; downloading and installing updates remain manual.
- The app is ad-hoc signed and not notarized, so first launch requires the documented manual Gatekeeper exception.
- HDR-preserved export is not supported; the official output is sRGB Visual Match.
- Updating the ad-hoc-signed app may require granting Screen Recording permission again; see the macOS installation guide for recovery steps.

## [0.2.0] - 2026-09-09

Release platforms: macOS

### Added

- Move between displays during Region selection; the preview and captured region follow the pointer to the new display.
- Recover from screenshot failures in the main window, with silent system notifications for background failures and partial delivery.

### Changed

- Open Region selection faster with a smaller preview while retaining full-resolution output from the same frozen frame.
- Download separate, smaller disk images for Apple Silicon (`arm64`) and Intel (`x64`) Macs.
- Keep successful captures and cancellation quiet, with a shared busy state across capture buttons, shortcuts, and the menu bar.

### Fixed

- Refresh HDR availability when the pointer moves between displays.
- Keep failure details and recovery controls within the main window's fixed size.

### Known limitations

- The app is ad-hoc signed and not notarized, so first launch requires the documented manual Gatekeeper exception.
- HDR-preserved export is not supported; the official output is sRGB Visual Match.
- This release includes macOS artifacts only; Windows signing and runtime verification remain in progress.
- Updating the ad-hoc-signed app may require granting Screen Recording permission again; see the macOS installation guide for recovery steps.

## [0.2.0-preview.1] - 2026-09-04

Release platforms: Windows

### Added

- First unsigned Windows preview for x64 PCs running Windows 10 or newer.
- Assisted per-user installation with a selectable destination and desktop and Start menu shortcuts.
- Display and frozen-frame Region capture with Clipboard, folder, and combined delivery.

### Known limitations

- The installer is unsigned and Windows will show an unknown-publisher warning; verify `SHA256SUMS` before running it.
- Production borderless capture identity and automatic updates are intentionally disabled in this preview.
- HDR-preserved export is not supported; the official output is sRGB Visual Match.

## [0.1.0] - 2026-09-03

Release platforms: macOS

### Added

- First direct macOS release for Apple Silicon and Intel Macs running macOS 15 or newer.
- Display and frozen-frame Region capture with target-aware HDR status and compatible RGBA8/sRGB Visual Match output.
- Clipboard, folder, and combined delivery with configurable save location and optional reveal-after-capture behavior.
- Configurable global shortcuts, menu-bar commands, and non-blocking HDR status reminders.

### Known limitations

- The app is ad-hoc signed and not notarized, so first launch requires the documented manual Gatekeeper exception.
- HDR-preserved export is not supported; the official output is sRGB Visual Match.
- Windows release artifacts are not included in this version.

[0.3.2]: https://github.com/Mournerliao/lumiere/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Mournerliao/lumiere/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/Mournerliao/lumiere/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Mournerliao/lumiere/compare/v0.2.0-preview.1...v0.2.0
[0.2.0-preview.1]: https://github.com/Mournerliao/lumiere/compare/v0.1.0...v0.2.0-preview.1
[0.1.0]: https://github.com/Mournerliao/lumiere/releases/tag/v0.1.0
