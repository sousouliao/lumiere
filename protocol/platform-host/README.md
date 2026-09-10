# Platform Host Protocol

This directory owns the language-neutral seam between the Electron shell and each
native capture host. Versioned JSON Schemas are the source of truth; TypeScript, C#,
and Swift bindings must conform to the schema they declare.

## Transport

The protocol uses UTF-8 JSON Lines over a child process' standard input and output.
Each line is one complete request or response. Hosts reserve standard output for
protocol messages and write structured diagnostics to standard error.

Every request carries `version`, a caller-generated `id`, `method`, and `params`.
Every response echoes `version` and `id`, and contains exactly one of `result` or
`error`. The shell may have multiple requests in flight, so ordering is determined by
`id`, not line position.

Unknown versions, methods, fields, or enum values are protocol errors. Additive
changes require a new schema version when an older host cannot safely reject or ignore
them. Versions 1–4 are frozen in their matching schemas. Version 5 is the current
shared contract in [`v5.schema.json`](v5.schema.json).

## Version 5

Version 5 adds the macOS-only `requestScreenCapturePermission` operation. It has empty
parameters and returns `granted` when access was already usable, `restart-required` when
the explicit native request granted access for the next process, or `not-granted` when
the user must use System Settings. The Shell never sends this operation to Windows.

The operation is user initiated and may call `CGRequestScreenCaptureAccess()`.
`getCapabilities` remains a pure query and must never prompt. Version 5 otherwise retains
the Version 4 target-token and frozen Region behavior unchanged.

## Version 4

Version 4 retains the frozen Region session model and adds stable target identity for
multi-display switching. `getCapabilities` may issue an opaque, short-lived
`activeTarget` token with its logical size. `prepareRegion` receives that token and
consumes the matching native target snapshot exactly once. The Shell never interprets
the token as an Electron display id or a native display handle.

Electron therefore uses its own display ids and DIP bounds only for Overlay placement,
while macOS and Windows retain authoritative display snapshots inside their Hosts. A
stale token or changed topology returns `capture-unavailable`; it never silently falls
back to the current pointer target.

## Version 3

`getCapabilities` is a pure query. It reports capture modes and delivery targets
independently and must not issue a target token. Display capture remains a one-request
`captureDisplay` operation: the Host resolves the display under the pointer when the
request arrives, then uses the current main display and system primary display only as
recovery fallbacks.

Region capture is a short-lived native frozen-frame session:

1. `prepareRegion` hides no Shell state itself; the Shell hides Lumiere-owned surfaces
   first, then the Host captures one complete native frame, retains it, and returns a
   session id plus an encoded sRGB preview path.
2. `commitRegion` crops that same frozen frame with a `target-logical` rectangle and
   runs the existing Visual Match and delivery path. It never captures a second frame.
3. `cancelRegion` releases the session. Unknown session ids still return `released`.

`target-logical` geometry is unchanged from v2: origin at the target's top-left,
logical desktop units, positive width and height, and `capture-unavailable` for a
stale session, changed topology, or out-of-bounds rectangle.

The preview file is Host-private. Electron main may grant a revocable custom-protocol
URL to the sandboxed Overlay; renderer IPC carries neither file paths nor image bytes.
Raw pixels and native handles never cross this seam.

### Delivery results

A `completed` capture result means acquisition and the single sRGB Visual Match
conversion completed. It does not mean every requested delivery succeeded. The
`deliveries` array contains exactly one result for each requested target:

- clipboard success has no artifact path;
- folder success includes its final file path;
- a failed target includes its own typed failure;
- target order is not significant and duplicate targets are invalid.

The Host must attempt both targets from the same conversion result when `both` is
requested. The shell derives full success, partial success, or total delivery failure
from these per-target results and never treats artifact delivery as proof of visual
match or HDR preservation.

Folder and Both capture parameters may include an absolute, platform-native
`saveDirectory`. Its absence preserves the Host's Pictures/Lumiere default. A
clipboard-only request must not include it. The shell owns directory selection and
preference persistence; the Host owns directory creation, fixed timestamp naming, file
writing, and the folder delivery result. Directory failure remains target-local so a
successful clipboard delivery is preserved.

### Cancellation ownership

Before `prepareRegion` succeeds, the shell owns Overlay startup and local abort. After
the Host accepts prepare, the Host owns the frozen native frame, preview file, lease,
and deterministic release. Overlay Esc, timeout, Host teardown, and application
shutdown all converge on `cancelRegion` or Host disposal. Native or task cancellation
of Display or Region commit returns `cancelled`.

## Version 2

Version 2 remains compatibility history. It used one `capture` method, advertised an
`activeTarget` token from `getCapabilities`, and dispatched Region capture only after
pointer release. Delivery results, `saveDirectory`, and target-local geometry from v2
are retained in v3. Region timing is superseded by
[`knowledge/decisions/0013-frozen-region-capture-session.md`](../../knowledge/decisions/0013-frozen-region-capture-session.md).

## Fixtures

[`fixtures/v1`](fixtures/v1), [`fixtures/v2`](fixtures/v2),
[`fixtures/v3`](fixtures/v3), [`fixtures/v4`](fixtures/v4), and
[`fixtures/v5`](fixtures/v5) are executable examples. The desktop protocol tests
validate every fixture against its owning schema. Fixtures illustrate wire shape; they
do not replace the cross-field checks described above.
