---
name: lumiere-release
description: Prepare and publish one Lumiere release from the current repository state.
disable-model-invocation: true
---

# Publish Lumiere

Treat an explicit invocation of this skill as the user's instruction to **prepare and
publish** a release. This supplies both authorizations defined by the release contract:
choose the release from the evidence, create the release commits, push to `main`, dispatch
the unified GitHub workflow, wait for it, and verify the public release. Do not ask the user
to choose a version or platform when the repository evidence answers the question.

## Sources of truth

Before acting, read these files completely and operate their current contents rather than
copying release rules into this skill:

1. [`knowledge/contracts/releases.md`](../../../knowledge/contracts/releases.md)
2. [`knowledge/runbooks/releasing.md`](../../../knowledge/runbooks/releasing.md)
3. [`knowledge/state/CURRENT.md`](../../../knowledge/state/CURRENT.md)

Read the selected platform runbooks when their build, runtime, signing, or recovery gates
apply. During the audit, read the owning Issues and PRs needed to account for every shipped
change and its evidence.

## Execution

1. Establish the worktree, branch, remotes, GitHub authentication, latest stable public
   release, and its target commit. Preserve unrelated user changes.
2. Audit the entire reachable delta through the candidate commit. Apply the contract to
   decide whether a release is warranted, its version, its platforms, its changelog, and
   every required check. State the decision briefly and continue without a confirmation
   round trip.
3. Run the runbook's **Prepare a Release** sequence through its completion criterion,
   including the coherent candidate commit.
4. Immediately run the runbook's **Publish** sequence. Use the actual publication date,
   push the release commit to `main`, dispatch only the repository's unified release
   workflow, wait through non-actionable progress, and perform every public-release check.
5. If publication changes current posture or verification truth, apply the runbook's later
   `CURRENT.md` documentation update as a separate commit and push it after the tagged
   release has been verified.

Keep working until the public release satisfies the runbook's publication completion
criterion. Retry only recovery paths permitted by the runbook, and leave GitHub approval or
signing requests pending when they require the user's account action.

## Stop conditions

Stop and report the exact evidence when:

- the audit finds no release-worthy change;
- a reachable change lacks required evidence or another release gate fails;
- unrelated worktree changes overlap the release edits and cannot be preserved safely;
- recovery would require moving/deleting a tag, replacing published bytes, changing product
  behavior, or another action outside the release contract;
- GitHub requires an approval or secret that only the user can supply.

Do not weaken checks or modify unrelated product code to force a release through a failed
gate.

## Handoff

On success, report the version, selected platforms, release and documentation commit IDs,
public release link, workflow result, published assets, checksum verification, and exact
checks run. On a stop condition, report what completed, what remains unpublished, and the
single concrete action needed to resume.
