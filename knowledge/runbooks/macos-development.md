# macOS Native Development Runbook

This runbook owns the Swift ScreenCaptureKit host build, protocol smoke test, runtime
launch, permission recovery, and macOS-only verification boundary.

## Prerequisites

- macOS 15 or newer
- Xcode with a matching macOS SDK and Swift toolchain
- Apple Silicon for HDR ScreenCaptureKit acquisition; Intel remains an SDR-only path
- the shared-shell prerequisites in
  [`cross-platform-development.md`](cross-platform-development.md)

Confirm the selected toolchain:

```sh
xcodebuild -version
xcrun swift --version
xcrun --sdk macosx --show-sdk-version
```

If Command Line Tools are selected instead of the installed Xcode, either update the
global selection with `xcode-select` outside this repository or scope commands with
`DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer`.

## Build And Test

From the repository root:

```sh
pnpm test:macos
swift build --package-path hosts/macos
swift test --package-path hosts/macos
```

The Vitest command owns macOS path and packaging-policy semantics. Swift tests own the
native Host. Both run only in macOS CI and are separate from the cross-platform
`pnpm test:shared` suite. The Host targets macOS 15 or newer. Development builds use the
active architecture; distribution builds produce independent `arm64` and `x64` applications.
Developer ID signing and notarization are not current release requirements.

## Packaged Application

Build the production Electron shell and independent Release Host/application bundles for
Apple Silicon and Intel, each with its own coherent ad-hoc signature:

```sh
pnpm package:macos
```

The command produces `artifacts/macos/apps/arm64/Lumiere.app` and
`artifacts/macos/apps/x64/Lumiere.app`. Both use bundle identifier
`io.github.sousouliao.lumiere`, the version from `apps/desktop/package.json`, and a minimum
system version of macOS 15. Each menu-bar-only app contains only its matching Swift Host
architecture, the English Electron locale family, and the two runtime menu-bar icons. The command verifies
the bundle signature, identity, version, minimum system version, every Mach-O architecture,
and the current package-size budgets. It does not produce disk images or notarize the apps.

## Direct-Release Artifact

Build both verified application bundles, their versioned architecture-specific DMGs, and
one SHA-256 manifest:

```sh
pnpm release:macos
```

For version `<version>`, the final release files are:

```text
artifacts/macos/Lumiere-<version>-macos-arm64.dmg
artifacts/macos/Lumiere-<version>-macos-x64.dmg
artifacts/macos/SHA256SUMS
```

The version comes from `apps/desktop/package.json`. Each DMG contains the matching-architecture
`Lumiere.app` and an Applications link; the command does not publish a GitHub Release or notarize
either app. Public publication is owned by the unified [release runbook](releasing.md). Verify the final
bytes from the artifact directory with:

```sh
cd artifacts/macos
shasum -a 256 -c SHA256SUMS
```

Mount both DMGs and inspect each contained app independently before recording release-artifact
truth. Run the Apple Silicon app natively and the Intel app on an Intel Mac or under Rosetta;
one architecture's runtime observation does not verify the other. Public-release,
quarantine/Gatekeeper, replacement-upgrade, uninstall, reinstall, and installed-lifecycle
observations belong to the following verification slice. That slice may use a named real Mac
after targeted Lumiere state and packaged-identity permission cleanup; a separate clean
non-development Mac is optional follow-up evidence, not an MVP gate.

## Protocol Smoke Test

Build the host, then send exactly one platform-host v5 JSON Lines request:

```sh
printf '%s\n' '{"version":5,"id":"capabilities-smoke","method":"getCapabilities","params":{}}' \
  | hosts/macos/.build/debug/LumiereMacHost
```

Standard output must contain only the matching protocol response. Structured native
diagnostics belong on standard error.

## Electron Runtime

The normal repository-root development command incrementally builds the current Debug
Host before Electron starts and selects that artifact ahead of any older Release build:

```sh
pnpm dev
```

The preparation step respects an explicit `DEVELOPER_DIR`. Otherwise it uses an installed
Xcode application for the child build without changing the global `xcode-select` setting;
this avoids mixing a Command Line Tools compiler with a newer Xcode SDK.

Use `LUMIERE_MAC_HOST_PATH` only to exercise a specific custom or prebuilt Host. An
explicit override skips the automatic Debug build and remains the only discovery
candidate for that launch.

The region and display actions should become available. The active target is the display
under the pointer when the capability or display-capture request reaches the native
host, with the current main screen and then the system primary display used only as
recovery fallbacks. Region capture freezes one native frame during `prepareRegion`, then
the Overlay selects in target-local logical geometry against that frozen preview.
`commitRegion` crops the same frame; a stale session, topology change, or out-of-bounds
rectangle returns `capture-unavailable`. A successful capture writes an
RGBA8/sRGB PNG under `~/Pictures/Lumiere` using the
`Lumiere-yyyy-MM-dd-HHmmss.png` rule and returns the exact path through the
platform-host interface.

## Screen Recording Permission

The native executable owns Screen Recording permission. The first display capture may
show the macOS privacy prompt. After granting permission, quit and relaunch the owning
process before retrying if macOS requests it.

For a denied or stale development identity, inspect System Settings → Privacy &
Security → Screen & System Audio Recording. `tccutil reset ScreenCapture` may reset the
development permission database, but it also removes Screen Recording grants for other
applications and should be used only as an explicit recovery action.

Packaged replacement upgrades use a narrower product recovery path. After an
upgrade-associated permission failure, the user may explicitly choose `Reset and restart`;
Lumiere then runs only
`/usr/bin/tccutil reset ScreenCapture io.github.sousouliao.lumiere`, persists the recovery
step, and relaunches. It never resets another bundle or TCC service. The next surface asks
the user to explicitly request access through the Native Host so macOS can register and
prompt for Lumiere; System Settings and a manual check remain fallbacks, and returning to
the app checks again automatically. A grant observed in the running process requests one
restart, while a newly launched process that already sees the grant returns directly to
ready. If the reset command fails, the UI offers the same
exact command for manual copying. This flow is packaged-macOS-only and must not be treated
as Windows behavior.

Command-line host, development Electron, and the packaged ad-hoc-signed application may be
treated as different identities by TCC. One identity's permission result does not
verify another.

## Verification Boundary

- Swift build and tests verify repository behavior only.
- A JSON Lines smoke test verifies host startup and protocol behavior only.
- An Electron button producing a valid PNG verifies the development integration path.
- Inspect the artifact's dimensions and embedded sRGB profile separately from visual
  match.
- Record the named Mac, display, source dynamic range, scene, receiving app, and
  observed result before claiming hardware verification.
- SDR capture on an SDR display does not verify HDR acquisition or HDR-to-sRGB tone
  mapping. Windows remains independently unverified.
