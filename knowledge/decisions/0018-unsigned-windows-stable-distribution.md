# ADR 0018: Allow unsigned Windows stable distribution

- Status: Accepted
- Date: 2026-09-11

## Context

ADR 0017 deferred Windows stable releases after the SignPath Foundation application was
declined. Lumiere is currently maintained for personal use, and purchasing a commercial
code-signing certificate is not justified. The unsigned NSIS installer, checksum
publication, and Windows runtime path already work independently of signing.

Code signing establishes publisher identity and enables the sparse package used for
borderless WGC capture, but it does not determine whether the application behavior itself
is stable. Keeping the entire Windows release lane in prerelease solely because no signing
provider is configured prevents an otherwise supportable stable artifact.

## Decision

Allow stable and prerelease Windows GitHub Releases to publish the same unsigned, assisted
per-user NSIS installer. Every unsigned Windows release must publish a SHA-256 checksum and
prominently disclose the unknown-publisher or SmartScreen warning.

Unsigned installers omit the external-location sparse identity and Windows updater
metadata. WGC therefore retains its system capture border, and automatic Windows updates
remain disabled. A stable GitHub Release describes product maturity; it does not claim a
verified Windows publisher identity.

Retain the dormant SignPath, sparse-identity, and updater implementation for possible later
use, but keep it disconnected from the active release workflow. Enabling any signed route
requires a new ADR and Issue with signing, identity, update, provenance, and clean-machine
verification.

## Consequences

- Windows stable releases may show SmartScreen or unknown-publisher warnings after download.
- Users must verify the installer against `SHA256SUMS` before making a local trust decision.
- Sparse identity, borderless-capture consent, automatic updates, and signed publisher
  identity remain unavailable and unclaimed.
- ADR 0017's preview-only restriction and the production-signing requirements in ADRs 0004
  and 0013 are superseded; their historical implementation rationale remains valid.
