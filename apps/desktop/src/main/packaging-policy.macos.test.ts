import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const desktopRoot = process.cwd()

describe('macOS packaging policy', () => {
  it('keeps the stable identity, split host layout, minimal resources, and signing boundary', async () => {
    const desktopPackage = JSON.parse(
      await readFile(resolve(desktopRoot, 'package.json'), 'utf8'),
    ) as {
      dependencies?: Record<string, unknown>
      devDependencies?: Record<string, unknown>
      productName?: unknown
      version?: unknown
    }
    const builderConfig = JSON.parse(
      await readFile(resolve(desktopRoot, 'electron-builder.json'), 'utf8'),
    ) as {
      appId?: unknown
      dmg?: Record<string, unknown>
      electronLanguages?: unknown
      productName?: unknown
      extraResources?: unknown
      mac?: Record<string, unknown>
    }
    const repositoryPackage = JSON.parse(
      await readFile(resolve(desktopRoot, '..', '..', 'package.json'), 'utf8'),
    ) as { scripts?: Record<string, unknown> }

    expect(desktopPackage.version).toMatch(/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/)
    expect(desktopPackage.productName).toBe('Lumiere')
    expect(builderConfig.appId).toBe('io.github.sousouliao.lumiere')
    expect(builderConfig.productName).toBe('Lumiere')
    expect(builderConfig.electronLanguages).toEqual(['en'])
    expect(builderConfig.extraResources).toEqual([
      {
        from: 'resources/icons/mac/trayTemplate.png',
        to: 'icons/mac/trayTemplate.png',
      },
      {
        from: 'resources/icons/mac/trayTemplate@2x.png',
        to: 'icons/mac/trayTemplate@2x.png',
      },
      {
        from: '../../hosts/macos/.build/distribution/staged/${arch}/LumiereMacHost',
        to: 'macos-host/LumiereMacHost',
      },
    ])
    expect(desktopPackage.dependencies).toEqual({
      'electron-updater': '6.8.9',
    })
    expect(desktopPackage.devDependencies).toMatchObject({
      clsx: '2.1.1',
      motion: '13.1.0',
      react: '19.2.8',
      'react-dom': '19.2.8',
      'tailwind-merge': '3.6.0',
    })
    expect(builderConfig.mac).toMatchObject({
      target: 'dir',
      minimumSystemVersion: '15.0',
      extendInfo: { LSUIElement: true },
      identity: '-',
      hardenedRuntime: true,
      binaries: ['Contents/Resources/macos-host/LumiereMacHost'],
      notarize: false,
    })
    expect(builderConfig.dmg).toMatchObject({
      artifactName: '${productName}-${version}-macos-${arch}.${ext}',
      title: 'Lumiere ${version}',
      backgroundColor: '#f4f2ee',
      filesystem: 'HFS+',
      format: 'UDZO',
      sign: false,
      writeUpdateInfo: false,
      contents: [
        { x: 140, y: 160, type: 'file' },
        { x: 380, y: 160, type: 'link', path: '/Applications' },
      ],
    })
    expect(repositoryPackage.scripts?.['release:macos']).toBe('node ./scripts/release-macos.mjs')
  })

  it('derives split release names and a deterministic checksum manifest from final bytes', async () => {
    const releaseModule: unknown = await import(
      // @ts-expect-error Repository release scripts intentionally remain plain Node modules.
      '../../../../scripts/release-macos.mjs'
    )
    const {
      checksumManifestForArtifacts,
      checksumManifestLine,
      macOSReleaseArtifactName,
      sha256File,
    } = releaseModule as {
      checksumManifestForArtifacts: (paths: string[]) => Promise<string>
      checksumManifestLine: (digest: string, fileName: string) => string
      macOSReleaseArtifactName: (version: string, architecture: string) => string
      sha256File: (filePath: string) => Promise<string>
    }
    const directory = await mkdtemp(join(tmpdir(), 'lumiere-release-policy-'))
    const arm64ArtifactPath = join(directory, 'Lumiere-0.1.0-macos-arm64.dmg')
    const x64ArtifactPath = join(directory, 'Lumiere-0.1.0-macos-x64.dmg')

    try {
      await writeFile(arm64ArtifactPath, 'abc', 'utf8')
      await writeFile(x64ArtifactPath, 'def', 'utf8')
      const arm64Digest = await sha256File(arm64ArtifactPath)
      const x64Digest = await sha256File(x64ArtifactPath)

      expect(macOSReleaseArtifactName('0.1.0', 'arm64')).toBe('Lumiere-0.1.0-macos-arm64.dmg')
      expect(macOSReleaseArtifactName('0.1.0-beta.1+build.7', 'x64')).toBe(
        'Lumiere-0.1.0-beta.1+build.7-macos-x64.dmg',
      )
      expect(arm64Digest).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad')
      expect(await checksumManifestForArtifacts([arm64ArtifactPath, x64ArtifactPath])).toBe(
        checksumManifestLine(arm64Digest, 'Lumiere-0.1.0-macos-arm64.dmg') +
          checksumManifestLine(x64Digest, 'Lumiere-0.1.0-macos-x64.dmg'),
      )
    } finally {
      await rm(directory, { force: true, recursive: true })
    }
  })

  it('recognizes the versioned Xcode application names used by hosted macOS runners', async () => {
    const packagingModule: unknown = await import(
      // @ts-expect-error Repository packaging scripts intentionally remain plain Node modules.
      '../../../../scripts/package-macos.mjs'
    )
    const { isXcodeApplicationName } = packagingModule as {
      isXcodeApplicationName: (name: string) => boolean
    }

    expect(isXcodeApplicationName('Xcode.app')).toBe(true)
    expect(isXcodeApplicationName('Xcode_26.0.app')).toBe(true)
  })
})
