# Release Contract

Lumiere releases on demand. There is no release calendar, preassigned milestone scope,
or platform-specific version sequence. The user chooses when to consider a release; the
agent owns the version, content, platform, and evidence decision.

## Two Authorizations

"Prepare a release" authorizes repository analysis and a release-candidate change. It
does not authorize a push, tag, workflow dispatch, or public GitHub Release. "Publish"
authorizes finalization, the release commit and push, and dispatch of the unified release
workflow described in [`../runbooks/releasing.md`](../runbooks/releasing.md).

## Audit Baseline

Use the latest published, non-prerelease GitHub Release as the comparison baseline. Read
its tag and target commit from GitHub, then inspect every reachable change through the
candidate commit, the owning Issues and PRs, current contracts, and recorded verification.
Do not assume the tag exists locally.

Account for every shipped change. Include a user-visible change when it is complete,
honestly supportable, and reachable in a selected platform artifact. Internal tests,
refactors, CI, and documentation enter the changelog only when they change the user's
experience. An incomplete change may be omitted only when it is unreachable in the
artifact. A reachable change without its required evidence blocks release.

If the audit finds no release-worthy change, recommend no release. Create a maintenance
Patch only when the user explicitly wants a new artifact despite that result.

## Version Decision

During `0.x` development:

- use Patch for compatible fixes, reliability work, visual polish, packaging maintenance,
  and internal changes;
- use Minor for any new user capability, newly released platform, default-behavior change,
  or compatibility break;
- use `1.0.0` only when the product contract's core experience, Windows and macOS
  distribution lanes, and their stability claims are all verified and supportable.

After `1.0.0`, use SemVer normally: compatible fixes are Patch, compatible capabilities
are Minor, and incompatible behavior or data changes are Major. The amount of code or
number of commits never determines the level. Use a prerelease identifier only when the
user explicitly requests a Preview, Beta, or RC.

## Platform Decision

Choose macOS, Windows, or both from the behavior changed and the evidence available. A
single version and GitHub Release may contain one or both platforms, while each platform
retains its independent build and verification truth. Never project one platform's result
onto the other.

## Signing Posture

Stable and prerelease status describe product maturity and are independent of platform
code-signing status. Windows releases may use the unsigned NSIS distribution selected by
ADR 0018 when no signing provider is configured. Such releases must publish a checksum and
disclose the unknown-publisher warning, retained WGC system capture border, and absence of
automatic updates. They must not claim publisher identity, sparse identity, or borderless
capture consent.

## Changelog

Root [`../../CHANGELOG.md`](../../CHANGELOG.md) is the public, chronological record of
notable user-visible changes. It is not a task ledger or commit log. During preparation,
populate `[Unreleased]` with exactly:

1. `Target version: \`X.Y.Z\``;
2. `Release platforms: macOS`, `Windows`, or `macOS, Windows` in that order;
3. one or more nonempty `Added`, `Changed`, `Fixed`, or `Known limitations` lists.

Write outcomes in user language. Omit empty categories. Preserve released entries; correct
an inaccurate historical statement with a new explicit documentation change rather than
silently rewriting what a release claimed.

## Release Gate

Before finalization, the prepared version, selected platforms, and changelog must match
the candidate source and available evidence. Before publication, `[Unreleased]` must be
finalized with the publication date, the desktop package version must match the newest
released changelog entry, README installation guidance must cover every selected platform,
relevant repository and platform checks must pass, and the candidate commit must be pushed
to `main`.

The workflow may publish only after every selected platform artifact succeeds. The public
release must contain release notes derived from the matching changelog entry, the selected
installers, Windows updater metadata only when an active signed route requires it, and one
checksum manifest covering all installer or disk-image bytes.
