# ADR 0020: Allow explicit unsigned Windows in-app updates

- Status: Accepted
- Date: 2026-10-08
- Owner: [Issue #24](https://github.com/sousouliao/lumiere/issues/24)

## Context

ADR 0018 disabled updating together with the dormant signing/sparse-identity lane.
Windows NSIS can install an update silently without a code-signing certificate;
Lumiere already uses electron-updater and per-user NSIS. The maintainer requested
periodic checks, explicit downloads and an explicit silent restart into the new version.

## Decision

Use electron-updater 6.8.9 independently of signing and sparse identity. The packaged
app-update.yml identifies the official sousouliao/lumiere GitHub Releases source;
latest.yml and the installer are generated together. Require HTTPS and SHA-512 file
verification; continue publishing SHA256SUMS. Authenticode publisher verification is
explicitly disabled for this unsigned lane. This trusts the official release source,
not a cryptographically authenticated publisher or independently signed update feed.

Check after 30 seconds and every six hours, without opening windows or automatically
downloading. The settings version row owns check/download/progress/retry/restart actions.
Download remains main-owned when the window closes. Ordinary quit does not install.
After capture ends, explicit restart blocks new captures, confirms Host exit and invokes
quitAndInstall(true, true). Development builds do not update themselves.

## Consequences

This supersedes only ADR 0018's prohibition of updater metadata and Windows updates.
Unsigned warnings, application-control restrictions, the WGC capture border and the
absence of sparse identity remain. Do not promise an absence of OS prompts or automatic
rollback. macOS retains its manual release-page update behavior.

Existing v0.5.0 installations need one manual upgrade to obtain the updater. Local
installed-version upgrade tests and real GitHub source observation are separate gates;
public publishing remains separately authorized. Missing hardware acceptance remains
open in its original Issue and is never inferred from update or installation success.
