# AGENTS.md

Lumiere is a Windows HDR-aware screenshot tool. Tauri/React owns the desktop shell;
Rust owns a resident JSONL Host and independent WGC/D3D11/DXGI capture library.
This file is a map, not a project manual.

## Start

Read only:

1. `knowledge/README.md` — knowledge ownership map.
2. `knowledge/state/CURRENT.md` — one-screen project state and frontier.
3. The referenced GitHub Issue, when one exists.

Then follow that task's links and load only relevant contracts, ADRs, or runbooks.
Do not preload the knowledge base.

For release preparation, version selection, changelog, tags, or public publishing, read
`knowledge/contracts/releases.md` and `knowledge/runbooks/releasing.md` before acting.

## Operating Model

Lumiere uses **Contract → Frontier → Verification**:

- Contracts define what is correct.
- GitHub Issues own non-trivial work, acceptance criteria, dependencies, and status.
- `CURRENT.md` owns only current project posture; Git owns history.
- Verification records exact commands and observed platform behavior in the owning
  GitHub Issue and current-state handoff.
- ADRs record durable decisions and trade-offs.

Work through `Orient → Classify → Execute → Verify → Handoff`. Use the risk and
truth levels in `knowledge/contracts/engineering.md`. Default to one writer;
planner, evaluator, sub-agent, and Ralph mechanisms are conditional escalation tools.

## Required Boundaries

- Follow `knowledge/contracts/architecture.md` for platform and module ownership.
- Follow `knowledge/contracts/claims.md` for output semantics and HDR language.
- Follow `knowledge/contracts/ui.md` for shared desktop UI work.
- Read `apps/desktop/DESIGN.md` before changing React renderer surfaces or reusable UI components.
- Follow existing code patterns before introducing abstractions.
- Use deterministic native-resource disposal and structured native logging.
- Keep artifact success, visual match, and HDR preservation separate.

## Verification And Handoff

- Use `knowledge/runbooks/cross-platform-development.md` for the shared shell and the
  owning platform runbook for native build, runtime, and recovery.
- Treat CI, local commands, and platform observations as distinct truth; one platform
  never verifies another.
- Leave each slice clean and reviewable. Record only completed behavior, exact checks,
  remaining acceptance criteria/blockers, and the next concrete action.

## Knowledge Hygiene

Update only the artifact that owns the changed fact. Do not create duplicate
backlogs, checkbox task ledgers, session/loop logs, generated context dumps, or
transcript memory. Update `CURRENT.md` only when frontier or verification posture changes.

Use Conventional Commit prefixes: `feat:`, `fix:`, `docs:`, `chore:`, and `test:`.
