# On-Demand Release Runbook

This runbook operates the release contract in
[`../contracts/releases.md`](../contracts/releases.md). Platform build details remain in
the [Windows](windows-development.md) runbook.

## Prepare a Release

1. Confirm the worktree and branch state without discarding unrelated user changes. Read
   the latest published, non-prerelease GitHub Release and its target commit; fetch the tag
   only when a local comparison requires it.
2. Audit the complete release delta, owning Issues/PRs, current contracts, and platform
   evidence. Classify every shipped change as user-visible, internal, unreachable, or a
   release blocker.
3. Apply the release contract to choose the version and Windows x64 artifact set. Update
   both `apps/desktop/package.json` and `apps/desktop/src-tauri/tauri.conf.json`, then populate `CHANGELOG.md` `[Unreleased]` with that target,
   platform set, and concise English release content.
4. Run `pnpm release:check`, the checks proportional to the selected changes, and any
   platform or hardware checks required by the claims. A reachable unverified behavior is
   a blocker, not a changelog omission.
5. Commit the coherent candidate as `chore: prepare vX.Y.Z`. Stop before push, tag,
   workflow dispatch, or public release unless the user has separately authorized publish.

Preparation is complete when the candidate commit contains one valid `[Unreleased]`
section, package and target versions agree, selected platforms have adequate evidence,
and no reachable release blocker remains.

## Publish

1. Re-audit changes since the prepared commit. If product behavior changed, return to
   preparation and regenerate the candidate decision.
2. Finalize the changelog using the current publication date:

   ```sh
   pnpm release:finalize -- --date YYYY-MM-DD
   pnpm release:check -- --publishing
   ```

3. Run the relevant final checks, commit as `chore: release vX.Y.Z`, push that commit to
   `main`, and confirm the local commit equals `origin/main`.
4. Provision the matching Tauri updater private-key/password secrets through an authorized
   maintainer; never log or commit them. Authenticode remains a separate policy.
   Dispatch `.github/workflows/release.yml` from `main`. Do not create or push the release
   tag manually. The workflow reads the version and platform set from the finalized source.
5. Wait for the workflow. Do not describe skipped platform jobs as failures. Leave any
   signing approval or environment request pending for the user when GitHub requires their
   account action.
6. After success, inspect the public Release: it must not be a draft, its stable/prerelease
   state must match the chosen version, its tag must target the release commit, its asset
   set must match the chosen platforms, and every EXE digest must match the single
   `SHA256SUMS`.
7. Update `knowledge/state/CURRENT.md` in a later documentation commit only when the release
   changed current posture or verification truth. Never rewrite the tagged source to add
   that observation.

Publication is complete only when the public release, tag target, asset set, checksum
verification, and selected platform claims all agree.

## Recovery

- A build or signing failure creates no public release. Fix the owning problem and rerun
  from the same release commit when source need not change; otherwise prepare a new commit.
- A failed publish step may leave a Draft Release. Rerunning the same workflow may recover
  it only when its tag target equals the workflow commit; assets are replaced from the
  newly staged set before it becomes public.
- A public release with the same version is immutable to this workflow. Investigate before
  any manual GitHub action.
- A same-name tag pointing elsewhere is a hard stop. Do not move or delete it as recovery.
- After a published defect, prepare a new Patch or Minor according to impact. Do not replace
  released bytes in place.
