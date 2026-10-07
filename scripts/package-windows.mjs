import { spawn } from 'node:child_process'
import { access, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { parseSemver } from './release-metadata.mjs'

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const desktopRoot = join(repositoryRoot, 'apps', 'desktop')
const hostRoot = join(repositoryRoot, 'hosts', 'windows')
const stagingRoot = join(repositoryRoot, 'artifacts', 'windows', 'staging')
const hostStaging = join(stagingRoot, 'windows-host')
const identityStaging = join(stagingRoot, 'windows-identity')
const identitySource = join(stagingRoot, 'windows-identity-source')
const generatedBuilderConfig = join(stagingRoot, 'electron-builder.generated.json')
const packageName = 'io.github.sousouliao.lumiere'
const applicationId = 'LumiereHost'

export const windowsPackagingPolicy = Object.freeze({
  packageName,
  applicationId,
  runtimeIdentifier: 'win-x64',
  minimumWindowsVersion: '10.0.19041.0',
})

async function main() {
  if (process.platform !== 'win32') {
    throw new Error('The Windows installer must be built on Windows.')
  }

  const operation = process.argv[2] ?? 'preview'
  if (!['preview', 'prepare-release', 'build-installer'].includes(operation)) {
    throw new Error(`Unknown Windows packaging operation: ${operation}`)
  }

  const desktopPackage = await readJson(join(desktopRoot, 'package.json'))
  const baseConfig = await readJson(join(desktopRoot, 'electron-builder.windows.json'))
  const version = parseSemver(desktopPackage.version)
  const release = operation !== 'preview'
  if (release && version.prerelease.length > 0) {
    throw new Error(
      `Signed Windows production packages require a stable three-component version: ${desktopPackage.version}`,
    )
  }

  if (operation === 'preview' || operation === 'prepare-release') {
    await prepare(desktopPackage, release)
  }
  if (operation === 'preview' || operation === 'build-installer') {
    await buildInstaller(baseConfig, desktopPackage.version, release)
  }
}

async function prepare(desktopPackage, release) {
  await rm(stagingRoot, { force: true, recursive: true })
  await mkdir(hostStaging, { recursive: true })
  await mkdir(identityStaging, { recursive: true })
  await mkdir(identitySource, { recursive: true })

  await run('pnpm', ['--filter', '@lumiere/desktop', 'build'], { cwd: repositoryRoot })

  const publishArguments = [
    'publish',
    join(hostRoot, 'src', 'Lumiere.Windows.Host', 'Lumiere.Windows.Host.csproj'),
    '--configuration',
    'Release',
    '--runtime',
    windowsPackagingPolicy.runtimeIdentifier,
    '--self-contained',
    'true',
    '--output',
    hostStaging,
    '-p:PublishSingleFile=false',
    '-p:PublishTrimmed=false',
  ]

  if (release) {
    const publisher = requiredEnvironment('LUMIERE_WINDOWS_PUBLISHER')
    const hostManifest = join(stagingRoot, 'Lumiere.Windows.Host.manifest')
    await writeFile(hostManifest, createHostManifest(publisher), 'utf8')
    publishArguments.push(`-p:ApplicationManifest=${hostManifest}`)
    await writeFile(
      join(identitySource, 'AppxManifest.xml'),
      createSparseManifest(desktopPackage.version, publisher),
      'utf8',
    )
    const makeAppx = await findWindowsSdkTool('makeappx.exe')
    await run(makeAppx, [
      'pack',
      '/d',
      identitySource,
      '/nv',
      '/p',
      join(identityStaging, 'Lumiere.Identity.msix'),
      '/o',
    ])
  }

  await run('dotnet', publishArguments, { cwd: repositoryRoot })
}

async function buildInstaller(baseConfig, version, release) {
  await access(join(hostStaging, 'Lumiere.Windows.Host.exe'))
  const electronRoot = join(desktopRoot, 'node_modules', 'electron')
  const electronDist = join(electronRoot, 'dist')
  await ensureElectronDistribution(electronRoot, electronDist)
  const config = structuredClone(baseConfig)
  config.electronDist = electronDist
  if (release) {
    await access(join(identityStaging, 'Lumiere.Identity.msix'))
    config.extraResources.push({
      from: '../../artifacts/windows/staging/windows-identity',
      to: 'windows-identity',
    })
  }
  await writeFile(generatedBuilderConfig, `${JSON.stringify(config, null, 2)}\n`, 'utf8')
  await run(
    'pnpm',
    [
      '--filter',
      '@lumiere/desktop',
      'exec',
      'electron-builder',
      '--win',
      'nsis',
      '--x64',
      '--config',
      generatedBuilderConfig,
      '--publish',
      'never',
    ],
    { cwd: repositoryRoot },
  )
  console.log(
    `Packaged installer: ${join(repositoryRoot, 'artifacts', 'windows', 'build', `Lumiere-Setup-${version}-x64.exe`)}`,
  )
}

function createHostManifest(publisher) {
  return `<?xml version="1.0" encoding="utf-8"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
    </windowsSettings>
  </application>
  <msix xmlns="urn:schemas-microsoft-com:msix.v1" publisher="${escapeXml(publisher)}" packageName="${packageName}" applicationId="${applicationId}" />
</assembly>
`
}

function createSparseManifest(version, publisher) {
  const publisherDisplayName =
    process.env.LUMIERE_WINDOWS_PUBLISHER_DISPLAY_NAME || 'SignPath Foundation'
  return `<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:uap11="http://schemas.microsoft.com/appx/manifest/uap/windows10/11"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap10 uap11 rescap">
  <Identity Name="${packageName}" Publisher="${escapeXml(publisher)}" Version="${version}.0" ProcessorArchitecture="neutral" />
  <Properties>
    <DisplayName>Lumiere</DisplayName>
    <PublisherDisplayName>${escapeXml(publisherDisplayName)}</PublisherDisplayName>
    <Logo>resources\\icons\\windows\\app.png</Logo>
    <uap10:AllowExternalContent>true</uap10:AllowExternalContent>
  </Properties>
  <Resources><Resource Language="en-us" /></Resources>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="${windowsPackagingPolicy.minimumWindowsVersion}" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
    <rescap:Capability Name="unvirtualizedResources" />
    <uap11:Capability Name="graphicsCaptureWithoutBorder" />
  </Capabilities>
  <Applications>
    <Application Id="${applicationId}" Executable="resources\\windows-host\\Lumiere.Windows.Host.exe" uap10:RuntimeBehavior="win32App" uap10:TrustLevel="mediumIL">
      <uap:VisualElements AppListEntry="none" DisplayName="Lumiere" Description="Lumiere capture host" BackgroundColor="transparent" Square150x150Logo="resources\\icons\\windows\\app.png" Square44x44Logo="resources\\icons\\windows\\app.png" />
    </Application>
  </Applications>
</Package>
`
}

async function findWindowsSdkTool(name) {
  const programFilesX86 = process.env['ProgramFiles(x86)']
  if (!programFilesX86) throw new Error('ProgramFiles(x86) is unavailable.')
  const binRoot = join(programFilesX86, 'Windows Kits', '10', 'bin')
  const versions = (await readdir(binRoot, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((left, right) => right.localeCompare(left, undefined, { numeric: true }))
  for (const version of versions) {
    const candidate = join(binRoot, version, 'x64', name)
    try {
      await access(candidate)
      return candidate
    } catch {
      // Continue to the next installed SDK.
    }
  }
  throw new Error(`${name} was not found in the Windows 10 SDK.`)
}

// pnpm records approved packages as built when it imports them from a warm
// store, so a fresh checkout or CI runner can end up without the Electron
// distribution; download it through Electron's own install script instead of
// failing later inside electron-builder.
async function ensureElectronDistribution(electronRoot, electronDist) {
  try {
    await access(join(electronDist, 'electron.exe'))
    return
  } catch {
    // The distribution is downloaded below.
  }
  console.log('Electron distribution is missing; running the Electron install script.')
  await run('node', ['install.js'], { cwd: electronRoot })
  await access(join(electronDist, 'electron.exe'))
}

function requiredEnvironment(name) {
  const value = process.env[name]
  if (!value) throw new Error(`${name} is required for a production Windows package.`)
  return value
}

function escapeXml(value) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('"', '&quot;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
}

async function readJson(path) {
  return JSON.parse(await readFile(path, 'utf8'))
}

function run(command, args, options = {}) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, { ...options, stdio: 'inherit' })
    child.once('error', rejectPromise)
    child.once('exit', (code, signal) => {
      if (code === 0) return resolvePromise()
      rejectPromise(
        new Error(
          signal
            ? `${command} was terminated by ${signal}.`
            : `${command} exited with code ${String(code)}.`,
        ),
      )
    })
  })
}

const invokedUrl = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : undefined
if (import.meta.url === invokedUrl) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
