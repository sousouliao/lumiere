# ADR 0017: Defer signed Windows distribution

- Status: Superseded by ADR 0018
- Date: 2026-09-11

## Context

ADR 0013 selected SignPath Foundation signing for the Windows NSIS installer and sparse
identity. The Foundation application submitted on 2026-09-05 was declined on 2026-09-09
because Lumiere does not yet have the required public adoption and external trust signals.
The remaining options are a later reapplication or a paid signing provider.

## Decision

Stop the current signed Windows distribution lane. Retain the working unsigned NSIS preview,
checksum publication, and dormant signing implementation, but do not treat them as signed
release capability. Windows previews keep the normal WGC system border and automatic updates
disabled. A future signing route requires a new ADR and Issue; it is not an active Milestone 1
exit criterion.

## Consequences

- Issue #12 closes as not planned rather than as completed signing work.
- Public documentation must call Windows artifacts unsigned previews and explain the warning.
- Sparse identity, borderless-consent, signed updates, and signed clean-machine verification
  remain unclaimed.
- The NSIS, self-contained Host, and fallback implementation from ADR 0013 remain reusable.
