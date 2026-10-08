# Engineering Contract

## Work Control

GitHub Issues are the control plane for non-trivial work. An executable Issue states:

- one user- or system-visible goal;
- relevant contract and ADR links rather than copied content;
- acceptance criteria that can be observed;
- required repository, Windows, and hardware gates;
- native Issue dependencies for blockers.

Lumiere separates renderer/protocol repository checks from Windows native runtime and
hardware evidence. Rust owns the Windows Host and native capture library. One writer
advances the current Issue; unavailable hardware observations remain explicitly open.

Each lane may have one open, acceptance-ready frontier Issue, and `CURRENT.md` names
the next concrete action for every active lane. Dependencies between lane Issues must
be explicit. Multiple eligible lane frontiers do not authorize concurrent writers on
overlapping files: one writer or worktree advances one current working Issue at a time,
then leaves a clean, structured handoff before switching lanes or machines. Small L0
maintenance changes may be performed directly when creating an Issue would cost more
than the change.

## Risk-Adaptive Execution

Classify work before implementation:

- **L0** — small, local, reversible: implement directly and run targeted checks.
- **L1** — normal feature or fix: use one Issue with explicit acceptance criteria.
- **L2** — cross-module, lifetime, output-semantic, or public-claim change: research
  or specify first, add an ADR for a durable choice, and use fresh-context review.
- **L3** — multi-session work: split dependent vertical Issues, advance one current
  working Issue per writer or worktree, and switch execution lanes only through a
  structured handoff.
- **L4** — bounded automation/Ralph loop: require machine-verifiable acceptance,
  an iteration cap, and no unapproved deployment or destructive action.

Increase ceremony only in response to risk, duration, ambiguity, or an observed
failure mode. Default to one writer. Use sub-agents only for bounded, independent
research or verification where isolated context adds value.

Long work must advance in reviewable vertical slices. A handoff contains only:

- completed behavior and commit/worktree state;
- exact verification and commands run;
- remaining acceptance criteria or blocker;
- the next concrete action.

Do not preserve transcripts, exploratory reasoning, or session narration as project state.

Renderer implementation may be checked without native capture, but its truth is
still scoped by the checks actually run. Repository CI can establish repository
truth; menus, shortcuts, windows, permissions, native capture, delivery, HDR behavior,
and hardware observations require the owning platform. Work in one lane never closes
another lane's acceptance criteria.

## Completion

Completion is determined by Issue acceptance criteria and applicable truth level,
not by an agent's declaration. Tests should exercise the production behavior being
claimed. Cached results and incomplete artifacts never count as current verification.

Truth levels are distinct:

1. **Repository done** — implementation, relevant tests, format, and static checks pass.
2. **Platform verified** — the named Windows build/test and runtime smoke pass.
3. **Hardware verified** — native capture, HDR behavior, sRGB Visual Match, and named
   receiving apps are observed on the named platform/display and recorded with the
   owning release work.

MVP release requires the applicable Windows truth level. Public HDR-preserved
claims require level 3 for every named platform. One platform's verification must never
be projected to another, and level 1 or 2 must never be projected upward.

## Code Rules

- Follow existing patterns before creating abstractions.
- Keep public interfaces narrow and platform ownership explicit.
- Use explicit optional values and typed expected-failure results.
- Use structured logging and deterministic native-resource disposal.
- Keep user-facing output success separate from fidelity/HDR claims.
- Do not bypass relevant formatting or test gates.

## Knowledge Hygiene

Each fact has one owner as listed in `knowledge/README.md`. Update only that owner.
At a milestone boundary, remove stale links and superseded documents instead of
appending historical sections to current-state files. A repeated workflow may become
a Skill, lint, test, or automation only after its repeated value is observed.
