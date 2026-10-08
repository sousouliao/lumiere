import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const desktop = join(root, 'apps', 'desktop')
const staging = join(root, 'artifacts', 'windows', 'tauri')
const release = join(root, 'artifacts', 'windows', 'release')
const operation = process.argv[2] ?? 'preview'
if (process.platform !== 'win32') throw new Error('Windows packaging requires Windows x64.')
if (!['preview', 'prepare-release', 'build-installer'].includes(operation)) {
  throw new Error(`Unknown Windows packaging operation: ${operation}`)
}
const pkg = JSON.parse(await readFile(join(desktop, 'package.json'), 'utf8'))
const config = JSON.parse(await readFile(join(desktop, 'src-tauri', 'tauri.conf.json'), 'utf8'))
if (pkg.version !== config.version) throw new Error('Package and Tauri versions must agree.')
// This command only stages local artifacts. It never tags, pushes or publishes.
await mkdir(staging, { recursive: true })
await mkdir(release, { recursive: true })
if (operation !== 'build-installer') {
  await run('cargo', [
    'build',
    '--locked',
    '--release',
    '-p',
    'lumiere-windows-host',
    '-p',
    'lumiere-installer',
  ])
  await run('pnpm', ['exec', 'tauri', 'build', '--no-bundle', '--ci'], { cwd: desktop })
}
const payload = { version: pkg.version, files: [] }
for (const name of ['Lumiere.exe', 'lumiere-windows-host.exe']) {
  const bytes = await readFile(join(root, 'target', 'release', name))
  payload.files.push({ path: name, sha256: createHash('sha256').update(bytes).digest('hex') })
}
await writeFile(join(staging, 'payload.json'), `${JSON.stringify(payload, null, 2)}\n`)
// Raw NSIS, one payload and no in-place binary patch after the manifest is hashed.
// Official updater infers the installer format from these bytes.
const bundleConfig = join(staging, 'bundle.json')
await writeFile(
  bundleConfig,
  JSON.stringify({ bundle: { active: true, createUpdaterArtifacts: false } }),
)
if (operation === 'prepare-release') {
  console.log(`Prepared native files and transaction manifest: ${staging}`)
} else {
  await run(
    'pnpm',
    [
      'exec',
      'tauri',
      'bundle',
      '--ci',
      '--bundles',
      'nsis',
      '--no-binary-patching',
      '--config',
      bundleConfig,
    ],
    { cwd: desktop },
  )
  const installer = `Lumiere-Setup-${pkg.version}-x64.exe`
  await copyFile(
    join(root, 'target', 'release', 'bundle', 'nsis', `Lumiere_${pkg.version}_x64-setup.exe`),
    join(release, installer),
  )
  const signingEnv = { ...process.env }
  if (!signingEnv.TAURI_SIGNING_PRIVATE_KEY && !signingEnv.TAURI_SIGNING_PRIVATE_KEY_PATH) {
    const keyRoot = join(homedir(), '.lumiere', 'updater')
    signingEnv.TAURI_SIGNING_PRIVATE_KEY_PATH = join(keyRoot, 'lumiere.key')
    signingEnv.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (
      await readFile(join(keyRoot, 'password.txt'), 'utf8')
    ).trim()
  }
  await run(
    'pnpm',
    ['exec', 'tauri', 'signer', 'sign', '--app-version', pkg.version, join(release, installer)],
    { cwd: desktop, env: signingEnv },
  )
  await run('node', ['scripts/finalize-windows-release.mjs'])
  console.log(`Local installer and matching legacy/Tauri metadata: ${release}`)
}
function run(command, args, options = {}) {
  return new Promise((resolveRun, reject) => {
    const executable = command === 'pnpm' ? process.execPath : command
    const arguments_ = command === 'pnpm' ? [process.env.npm_execpath, ...args] : args
    const child = spawn(executable, arguments_, { cwd: root, stdio: 'inherit', ...options })
    child.on('error', reject)
    child.on('exit', (code) =>
      code === 0 ? resolveRun() : reject(new Error(`${command} failed (${code})`)),
    )
  })
}
