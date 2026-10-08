# Code-signing policy

Lumiere does not currently have a Windows code-signing certificate. The SignPath
Foundation application submitted on 2026-09-05 was declined on 2026-09-09 because the
project did not yet have the public adoption and external trust signals required by the
Foundation program. The project may reapply later or evaluate a paid provider through a
separate decision, but neither route is currently planned.

Current Windows stable and prerelease installers are unsigned, publish their SHA-256 digest
alongside the artifact, and do not enable production sparse identity. Explicit in-app updates verify updater
signatures and signed versions independently of Windows publisher identity.
Stable status describes the application release rather than a verified publisher identity.
Private keys are never stored in this repository. An authorized release requires the matching
Tauri updater signing key/password in protected GitHub Actions secrets; these have not been
provisioned by the migration. Local keys stay outside Git with restricted access.

## Team roles

- Committer and reviewer: [sousouliao](https://github.com/sousouliao)
- Signing approver: not assigned while no signing provider is configured

Security-relevant signing or release-workflow changes require maintainer review. If an
official release is suspected of compromise, publishing stops until the affected
certificate, workflow, and artifacts have been investigated and replaced or revoked.
