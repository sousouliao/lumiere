import { access, readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const repositoryRoot = resolve(process.cwd(), '..', '..')

describe('unified release workflow', () => {
  it('is manually dispatched from main and takes version and platforms from release metadata', async () => {
    const workflow = await readFile(
      resolve(repositoryRoot, '.github', 'workflows', 'release.yml'),
      'utf8',
    )

    expect(workflow).toContain('workflow_dispatch:')
    expect(workflow).not.toContain("tags:\n      - 'v*.*.*'")
    expect(workflow).toContain("if: github.ref != 'refs/heads/main'")
    expect(workflow).toContain('pnpm release:inspect -- --github-output "$GITHUB_OUTPUT"')
    expect(workflow).toContain("if: needs.audit.outputs.macos == 'true'")
    expect(workflow).toContain("if: needs.audit.outputs.windows == 'true'")
  })

  it('publishes only after every selected platform succeeds or is skipped', async () => {
    const workflow = await readFile(
      resolve(repositoryRoot, '.github', 'workflows', 'release.yml'),
      'utf8',
    )

    expect(workflow).toContain("needs.macos.result == 'success' || needs.macos.result == 'skipped'")
    expect(workflow).toContain(
      "needs.windows.result == 'success' || needs.windows.result == 'skipped'",
    )
    expect(workflow).toContain('release-metadata.mjs checksums')
    expect(workflow).toContain('gh release create "$RELEASE_TAG" artifacts/publish/*')
    expect(workflow).toContain('--draft=false')
    expect(workflow).not.toContain('--generate-notes')
  })

  it('publishes every Windows release through the unsigned installer lane', async () => {
    const workflow = await readFile(
      resolve(repositoryRoot, '.github', 'workflows', 'release.yml'),
      'utf8',
    )

    expect(workflow).toContain('run: pnpm package:windows')
    expect(workflow).toContain('path: artifacts/windows/build/Lumiere-Setup-*-x64.exe')
    expect(workflow).not.toContain('signpath/github-action-submit-signing-request')
    expect(workflow).not.toContain('pnpm release:windows:finalize')
    expect(workflow).not.toContain('latest.yml')
    expect(workflow).not.toContain('SIGNPATH_')
  })

  it('replaces the tag-triggered Windows publisher', async () => {
    await expect(
      access(resolve(repositoryRoot, '.github', 'workflows', 'windows-release.yml')),
    ).rejects.toThrow()
  })
})
