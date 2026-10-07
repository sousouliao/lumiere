import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const desktopRoot = process.cwd()

describe('Windows packaging policy', () => {
  it('enables unsigned NSIS updates from the actual public repository without sparse identity', async () => {
    const config = JSON.parse(
      await readFile(resolve(desktopRoot, 'electron-builder.windows.json'), 'utf8'),
    ) as {
      publish: unknown
      win: { verifyUpdateCodeSignature: unknown }
      nsis: { perMachine: unknown }
      extraResources: unknown
    }
    expect(config.publish).toEqual([
      { provider: 'github', owner: 'sousouliao', repo: 'lumiere', releaseType: 'release' },
    ])
    expect(config.win.verifyUpdateCodeSignature).toBe(false)
    expect(config.nsis.perMachine).toBe(false)
    expect(JSON.stringify(config.extraResources)).not.toContain('windows-identity')
  })
  it('keeps the installer sharp and makes drive-root destinations explicit', async () => {
    const builderConfig = JSON.parse(
      await readFile(resolve(desktopRoot, 'electron-builder.windows.json'), 'utf8'),
    ) as {
      nsis?: { include?: unknown }
    }
    const installerInclude = await readFile(
      resolve(desktopRoot, String(builderConfig.nsis?.include)),
      'utf8',
    )

    expect(installerInclude).toContain('ManifestDPIAware true')
    expect(installerInclude).toContain(
      '!define MUI_PAGE_CUSTOMFUNCTION_SHOW lumiereDirectoryPageShow',
    )
    expect(installerInclude).toContain('Function normalizeLumiereDriveRoot')
    expect(installerInclude).toContain('StrCpy $INSTDIR "$1${APP_FILENAME}"')
  })
})
