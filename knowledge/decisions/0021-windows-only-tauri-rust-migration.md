# ADR 0021: Replace Electron and .NET with a Windows-only Tauri/Rust product

- Status: Accepted; implementation in progress
- Date: 2026-10-08
- Owner: [Issue #25](https://github.com/sousouliao/lumiere/issues/25)

## Decision

The maintainer approved a staged migration from clean commit
`cc0c011af0bc53be696d4f84bda803df24315f6e`. Windows x64 is the only delivered
platform. Remove all macOS-specific implementation, resources, tests, scripts and
active support claims by the final cutover. Git preserves the former implementation;
do not maintain an archived source tree or speculative platform stubs.

Use Tauri 2 stable and the existing React renderer. A language-neutral JSON Lines
contract separates the application from a thin Rust Host executable and an independent
Windows capture library. Keep v5 capabilities/Display and v6 native Region/cancellation
wire semantics. Native resources and the full-resolution frozen selection frame stay
inside the Host. Keep existing WGC/D3D11/DXGI acquisition and fixed sRGB Visual Match
behavior; the migration introduces no new output profile or fidelity claim.

The native application and Host stay resident, but the settings WebView is created on
demand and destroyed on close. Shortcut capture does not depend on JavaScript. Preserve
the current Windows UI, preferences, quiet feedback, native selection and explicit
in-app update behavior, including the v0.6.0 changes.

Use official Tauri updater signatures, independent of Authenticode. Publish legacy
Electron and Tauri update metadata from the same installation payload. Upgrade the
existing per-user installation in place, preserving settings and user-owned files.
Remove application-owned legacy files using an explicit inventory, not the legacy
uninstaller's recursive installation-directory removal.

## Execution and verification

Advance baseline, contract/transport, Display, native Region, Tauri shell, installer/
updates, and final cleanup as dependent slices, each with its own commit and Issue
verification. During migration the former implementation remains a reference, not a
second final production path. Remove it only after the replacing behavior passes.

Repository, packaged runtime, UI comparison, HDR hardware, installation migration and
performance evidence remain distinct. Existing release-specific acceptance waivers
do not certify this implementation. Missing physical-input, topology, clean-machine
or compositor observations remain explicitly open. Do not publish as part of this work.

This decision supersedes the cross-platform and Electron/.NET choices of ADR 0006
for the new product. Existing public releases and historical records remain factual.
Update owning contracts as the corresponding implementation lands; do not describe
planned behavior as completed.
