import { spawn } from 'node:child_process'
import {
  access,
  chmod,
  copyFile,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  stat,
} from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const desktopRoot = join(repositoryRoot, 'apps', 'desktop')
const hostRoot = join(repositoryRoot, 'hosts', 'macos')
const builderConfigPath = join(desktopRoot, 'electron-builder.json')
const artifactsDirectory = join(repositoryRoot, 'artifacts', 'macos')
const architectures = ['arm64', 'x64']
const finalAppPaths = Object.freeze(
  Object.fromEntries(
    architectures.map((architecture) => [
      architecture,
      join(artifactsDirectory, 'apps', architecture, 'Lumiere.app'),
    ]),
  ),
)

export const macOSPackagingPolicy = Object.freeze({
  appId: 'io.github.sousouliao.lumiere',
  productName: 'Lumiere',
  minimumSystemVersion: '15.0',
  architectures,
  electronLanguages: ['en'],
  finalAppPaths,
  maximumAppBytes: 270 * 1024 * 1024,
  maximumAsarBytes: 8 * 1024 * 1024,
})

export function swiftTriple(architecture) {
  if (architecture === 'arm64') return 'arm64-apple-macosx15.0'
  if (architecture === 'x64') return 'x86_64-apple-macosx15.0'
  throw new Error(`Unsupported macOS packaging architecture: ${architecture}`)
}

export async function packageMacOS({ targets = ['dir'] } = {}) {
  if (process.platform !== 'darwin') {
    throw new Error('The macOS application bundle must be built on macOS.')
  }
  if (!targets.includes('dir') || targets.some((target) => target !== 'dir' && target !== 'dmg')) {
    throw new Error('macOS packaging targets must include dir and may additionally include dmg.')
  }

  const desktopPackage = await readJson(join(desktopRoot, 'package.json'))
  const builderConfig = await readJson(builderConfigPath)
  validateConfiguration(desktopPackage, builderConfig)

  const developerDirectory = process.env.DEVELOPER_DIR || (await discoverXcodeDeveloperDirectory())
  if (!developerDirectory) {
    throw new Error('A complete Xcode installation is required to package Lumiere for macOS.')
  }
  const buildEnvironment = { ...process.env, DEVELOPER_DIR: developerDirectory }

  await run('pnpm', ['--filter', '@lumiere/desktop', 'build'], {
    cwd: repositoryRoot,
    env: process.env,
  })

  for (const architecture of architectures) {
    await buildAndStageHost(architecture, buildEnvironment)
  }

  await run(
    'pnpm',
    [
      '--filter',
      '@lumiere/desktop',
      'exec',
      'electron-builder',
      '--mac',
      ...targets,
      '--arm64',
      '--x64',
      '--config',
      'electron-builder.json',
    ],
    { cwd: repositoryRoot, env: process.env },
  )

  const appPaths = {}
  for (const architecture of architectures) {
    const builtAppPath = join(
      artifactsDirectory,
      'build',
      architecture === 'arm64' ? 'mac-arm64' : 'mac',
      'Lumiere.app',
    )
    const finalAppPath = finalAppPaths[architecture]
    await access(builtAppPath)
    await mkdir(dirname(finalAppPath), { recursive: true })
    await rm(finalAppPath, { force: true, recursive: true })
    await rename(builtAppPath, finalAppPath)
    await verifyBundle(finalAppPath, desktopPackage.version, architecture)
    appPaths[architecture] = finalAppPath
    console.log(`Packaged ${architecture} application: ${finalAppPath}`)
  }

  return { appPaths, architectures, version: desktopPackage.version }
}

async function buildAndStageHost(architecture, environment) {
  const triple = swiftTriple(architecture)
  const scratchPath = join(hostRoot, '.build', 'distribution', architecture)
  const swiftArguments = [
    'swift',
    'build',
    '--package-path',
    hostRoot,
    '--configuration',
    'release',
    '--triple',
    triple,
    '--scratch-path',
    scratchPath,
  ]

  await run('/usr/bin/xcrun', swiftArguments, { cwd: repositoryRoot, env: environment })
  const binaryDirectory = await capture('/usr/bin/xcrun', [...swiftArguments, '--show-bin-path'], {
    cwd: repositoryRoot,
    env: environment,
  })
  const sourcePath = join(binaryDirectory.trim(), 'LumiereMacHost')
  const stagedDirectory = join(hostRoot, '.build', 'distribution', 'staged', architecture)
  const stagedPath = join(stagedDirectory, 'LumiereMacHost')
  await mkdir(stagedDirectory, { recursive: true })
  await copyFile(sourcePath, stagedPath)
  await chmod(stagedPath, 0o755)
}

async function verifyBundle(appPath, version, architecture) {
  const contentsPath = join(appPath, 'Contents')
  const executablePath = join(contentsPath, 'MacOS', 'Lumiere')
  const hostPath = join(contentsPath, 'Resources', 'macos-host', 'LumiereMacHost')
  const infoPlistPath = join(contentsPath, 'Info.plist')

  await run('/usr/bin/codesign', ['--verify', '--deep', '--strict', '--verbose=2', appPath])
  await expectCommandOutput(
    '/usr/bin/plutil',
    ['-extract', 'CFBundleIdentifier', 'raw', infoPlistPath],
    macOSPackagingPolicy.appId,
  )
  await expectCommandOutput(
    '/usr/bin/plutil',
    ['-extract', 'CFBundleShortVersionString', 'raw', infoPlistPath],
    version,
  )
  await expectCommandOutput(
    '/usr/bin/plutil',
    ['-extract', 'LSMinimumSystemVersion', 'raw', infoPlistPath],
    macOSPackagingPolicy.minimumSystemVersion,
  )
  await expectCommandOutput(
    '/usr/bin/plutil',
    ['-extract', 'LSUIElement', 'raw', infoPlistPath],
    'true',
  )
  await expectArchitecture(executablePath, architecture)
  await expectArchitecture(hostPath, architecture)
  await expectAllMachOBinaries(contentsPath, architecture)
  await expectEnglishOnlyLocales(contentsPath)
  await expectRuntimeIcons(contentsPath)
  await expectBundleSizes(appPath)

  console.log(`${architecture} Electron executable build metadata:`)
  await run('/usr/bin/vtool', ['-show-build', executablePath])
  console.log(`${architecture} LumiereMacHost build metadata:`)
  await run('/usr/bin/vtool', ['-show-build', hostPath])
}

async function expectArchitecture(binaryPath, expectedArchitecture) {
  const output = await capture('/usr/bin/lipo', ['-archs', binaryPath])
  const actual = new Set(output.trim().split(/\s+/))
  const expected = expectedArchitecture === 'x64' ? 'x86_64' : expectedArchitecture
  if (actual.size !== 1 || !actual.has(expected)) {
    throw new Error(`${binaryPath} has unexpected architectures: ${output.trim()}`)
  }
}

async function expectAllMachOBinaries(rootPath, architecture) {
  const expected = architecture === 'x64' ? 'x86_64' : architecture
  const files = await collectFiles(rootPath)
  let machOCount = 0
  for (const filePath of files) {
    const output = await captureOptional('/usr/bin/lipo', ['-archs', filePath])
    if (output === undefined) continue
    const actual = new Set(output.trim().split(/\s+/))
    if (actual.size !== 1 || !actual.has(expected)) {
      throw new Error(`${filePath} has unexpected architectures: ${output.trim()}`)
    }
    machOCount += 1
  }
  if (machOCount === 0) {
    throw new Error(`${rootPath} contains no verifiable Mach-O binaries.`)
  }
}

async function collectFiles(rootPath) {
  const files = []
  for (const entry of await readdir(rootPath, { withFileTypes: true })) {
    const entryPath = join(rootPath, entry.name)
    if (entry.isDirectory()) files.push(...(await collectFiles(entryPath)))
    else if (entry.isFile()) files.push(entryPath)
  }
  return files
}

async function expectEnglishOnlyLocales(contentsPath) {
  const localeDirectories = [
    join(contentsPath, 'Resources'),
    join(contentsPath, 'Frameworks', 'Electron Framework.framework', 'Versions', 'A', 'Resources'),
  ]
  for (const directory of localeDirectories) {
    const locales = (await readdir(directory))
      .filter((name) => name.endsWith('.lproj'))
      .map((name) => name.slice(0, -'.lproj'.length).toLowerCase().replaceAll('_', '-'))
    if (locales.length === 0 || locales.some((locale) => !locale.startsWith('en'))) {
      throw new Error(`${directory} contains unexpected Electron locales: ${locales.join(', ')}`)
    }
  }
}

async function expectRuntimeIcons(contentsPath) {
  const iconsPath = join(contentsPath, 'Resources', 'icons', 'mac')
  const actual = (await readdir(iconsPath)).sort()
  const expected = ['trayTemplate.png', 'trayTemplate@2x.png'].sort()
  if (actual.length !== expected.length || actual.some((name, index) => name !== expected[index])) {
    throw new Error(`${iconsPath} contains unexpected runtime icons: ${actual.join(', ')}`)
  }
}

async function expectBundleSizes(appPath) {
  const asarPath = join(appPath, 'Contents', 'Resources', 'app.asar')
  const asarBytes = (await stat(asarPath)).size
  const duOutput = await capture('/usr/bin/du', ['-sk', appPath])
  const appBytes = Number.parseInt(duOutput.trim().split(/\s+/)[0], 10) * 1024
  if (asarBytes > macOSPackagingPolicy.maximumAsarBytes) {
    throw new Error(`${asarPath} is ${formatMiB(asarBytes)}, exceeding the 8 MiB packaging budget.`)
  }
  if (appBytes > macOSPackagingPolicy.maximumAppBytes) {
    throw new Error(`${appPath} is ${formatMiB(appBytes)}, exceeding the 270 MiB packaging budget.`)
  }
  console.log(`Bundle sizes: app ${formatMiB(appBytes)}, app.asar ${formatMiB(asarBytes)}`)
}

function formatMiB(bytes) {
  return `${(bytes / 1024 / 1024).toFixed(2)} MiB`
}

async function expectCommandOutput(command, args, expected) {
  const actual = (await capture(command, args)).trim()
  if (actual !== expected) {
    throw new Error(
      `${args.join(' ')} returned ${JSON.stringify(actual)}; expected ${JSON.stringify(expected)}.`,
    )
  }
}

function validateConfiguration(desktopPackage, builderConfig) {
  if (desktopPackage.productName !== macOSPackagingPolicy.productName) {
    throw new Error('The desktop productName does not match the macOS packaging policy.')
  }
  if (builderConfig.appId !== macOSPackagingPolicy.appId) {
    throw new Error('The electron-builder appId does not match the macOS packaging policy.')
  }
  if (builderConfig.productName !== macOSPackagingPolicy.productName) {
    throw new Error('The electron-builder productName does not match the macOS packaging policy.')
  }
  if (builderConfig.mac?.minimumSystemVersion !== macOSPackagingPolicy.minimumSystemVersion) {
    throw new Error('The electron-builder minimum system version does not match the policy.')
  }
  if (builderConfig.mac?.extendInfo?.LSUIElement !== true) {
    throw new Error('The macOS bundle must run as a menu-bar-only application.')
  }
  if (builderConfig.mac?.identity !== '-') {
    throw new Error('The macOS bundle must use an explicit ad-hoc signing identity.')
  }
  if (
    !Array.isArray(builderConfig.electronLanguages) ||
    builderConfig.electronLanguages.length !== 1 ||
    builderConfig.electronLanguages[0] !== 'en'
  ) {
    throw new Error('The macOS bundle must keep only the supported English Electron locale.')
  }
}

async function discoverXcodeDeveloperDirectory() {
  const entries = await readdir('/Applications', { withFileTypes: true })
  const xcodeApplications = entries
    .filter((entry) => entry.isDirectory() && isXcodeApplicationName(entry.name))
    .map((entry) => entry.name)
    .sort((left, right) => {
      if (left === 'Xcode.app') return -1
      if (right === 'Xcode.app') return 1
      return left.localeCompare(right)
    })

  for (const application of xcodeApplications) {
    const developerDirectory = join('/Applications', application, 'Contents', 'Developer')
    try {
      await access(developerDirectory)
      return developerDirectory
    } catch {
      // Continue to another installed Xcode application.
    }
  }
  return undefined
}

export function isXcodeApplicationName(name) {
  return /^Xcode(?:[-_].+)?\.app$/.test(name)
}

async function readJson(path) {
  return JSON.parse(await readFile(path, 'utf8'))
}

function run(command, args, options = {}) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, { ...options, stdio: 'inherit' })
    child.once('error', rejectPromise)
    child.once('exit', (code, signal) => {
      if (code === 0) {
        resolvePromise()
        return
      }
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

function capture(command, args, options = {}) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, { ...options, stdio: ['ignore', 'pipe', 'inherit'] })
    let stdout = ''
    child.stdout.setEncoding('utf8')
    child.stdout.on('data', (chunk) => {
      stdout += chunk
    })
    child.once('error', rejectPromise)
    child.once('exit', (code, signal) => {
      if (code === 0) {
        resolvePromise(stdout)
        return
      }
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

function captureOptional(command, args, options = {}) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, { ...options, stdio: ['ignore', 'pipe', 'ignore'] })
    let stdout = ''
    child.stdout.setEncoding('utf8')
    child.stdout.on('data', (chunk) => {
      stdout += chunk
    })
    child.once('error', rejectPromise)
    child.once('exit', (code) => {
      resolvePromise(code === 0 ? stdout : undefined)
    })
  })
}

const invokedUrl = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : undefined
if (import.meta.url === invokedUrl) {
  packageMacOS().catch((error) => {
    console.error(error instanceof Error ? error.message : error)
    process.exitCode = 1
  })
}
