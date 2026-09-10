# 0016: Native Screen Recording Permission Recovery

Date: 2026-09-10

## Decision

The macOS Host owns the explicit Screen Recording permission request. Platform-host v5
adds the macOS-only `requestScreenCapturePermission` operation with `granted`,
`restart-required`, and `not-granted` results. Electron exposes that operation only
through the packaged macOS recovery surface; capability polling and application startup
remain read-only and never prompt.

Permission reconciliation has two contexts. A grant observed while Lumiere is already
running requires a restart. A grant observed while restoring `grant-required` or
`restart-required` state in a newly launched process means that launch has already
satisfied macOS's restart requirement and returns the UI to ready.

## Context

The v0.3.1 recovery flow reset Lumiere's TCC entry and then only opened System Settings
or queried Electron's permission status. With no new native request, Lumiere was absent
from System Settings until a screenshot command happened to call
`CGRequestScreenCaptureAccess()`. That out-of-band grant left persisted recovery state at
`grant-required`, so the first required relaunch produced a second restart prompt.

## Consequences

- The user explicitly initiates the system prompt from the blocking recovery surface.
- System Settings and manual checking remain fallbacks, and window focus performs a
  read-only check after the user returns.
- The persisted state format remains version 1; startup reconciliation changes behavior
  without migrating stored data.
- Windows accepts protocol v5 for its existing operations but receives no permission
  request method or UI behavior.
- Electron desktop capture is not introduced as a second permission or capture path.

## Rejected Alternatives

- Opening System Settings without first requesting access preserves the dead end when no
  Lumiere entry exists.
- Prompting automatically at startup removes explicit user intent and makes denial harder
  to explain.
- Treating every observed grant as restart-required cannot recognize that a fresh process
  has already completed the required restart.
