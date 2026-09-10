# ADR 0013: Windows NSIS distribution with external-location identity

- Status: Superseded in part by ADR 0017
- Date: 2026-09-02

## Context

Lumiere is an Electron application with a separate .NET WGC capture Host. It needs a
traditional installer and updater while `graphicsCaptureWithoutBorder` requires package
identity and explicit user consent. Windows and macOS release lanes must remain independent.

## Decision

Ship Windows x64 as an assisted, per-user NSIS installer with a selectable installation
directory. Publish the .NET 10 Host self-contained as a folder and keep trimming,
single-file publishing, and NativeAOT disabled.

Production installers carry a signed external-location sparse MSIX. Its identity matches
metadata embedded in the Host executable and declares `graphicsCaptureWithoutBorder`.
Registration is best-effort: failure, denied consent, or unsupported Windows keeps the WGC
system border and does not make capture fail. Unsigned preview installers omit production
identity and automatic update configuration.

GitHub Releases is the stable update source. `electron-updater` downloads the complete,
signed Setup executable because external signing changes the final artifact after
electron-builder runs. `latest.yml` and SHA-256 checksums are therefore generated only
after the final SignPath signing step. Drafts and prereleases are excluded.

## Consequences

- Windows packaging and runtime claims require Windows-specific verification.
- Production packaging requires an approved SignPath project and a Publisher value that
  exactly matches both the sparse manifest and embedded Host identity metadata.
- The installer remains responsible for sparse identity registration and removal.
- A full MSIX migration, Store distribution, ARM64, differential updates, and additional
  installer technologies remain outside this decision.
