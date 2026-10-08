import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { copyFile, mkdir, readdir, readFile, writeFile } from 'node:fs/promises'
import { join, relative, resolve } from 'node:path'
import { execFileSync } from 'node:child_process'

// Run against an untouched packaged baseline, before replacing the bundler.
const root = resolve(import.meta.dirname, '..')
const build = join(root, 'artifacts/windows/build')
const app = join(build, 'win-unpacked')
const commit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()
const { version } = JSON.parse(await readFile(join(root, 'apps/desktop/package.json'), 'utf8'))
const output = join(root, 'artifacts/windows/baseline', commit)
await mkdir(output, { recursive: true })
const files = []
await visit(app)
files.sort((a, b) => a.path.localeCompare(b.path, 'en'))
const installer = `Lumiere-Setup-${version}-x64.exe`
await copyFile(join(build, installer), join(output, installer))
const manifest = {
  version: 1,
  sourceCommit: commit,
  productVersion: version,
  appId: 'io.github.sousouliao.lumiere',
  executable: 'Lumiere.exe',
  installer: { path: installer, ...(await digest(join(build, installer))) },
  installedBytes: files.reduce((total, file) => total + file.bytes, 0),
  files,
}
await writeFile(join(output, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`)
if (process.argv.includes('--legacy-manifest')) {
  await writeFile(
    join(root, `apps/desktop/build/windows-installer/legacy-files.v${version}.json`),
    `${JSON.stringify({ version: 1, sourceCommit: commit, productVersion: version, appId: manifest.appId, executable: manifest.executable, files: files.map((file) => file.path) }, null, 2)}\n`,
  )
}
console.log(
  JSON.stringify({
    output,
    installer: manifest.installer,
    installedBytes: manifest.installedBytes,
    files: files.length,
  }),
)

async function visit(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name)
    if (entry.isSymbolicLink()) throw new Error(`Baseline must not contain links: ${path}`)
    if (entry.isDirectory()) await visit(path)
    else if (entry.isFile())
      files.push({ path: relative(app, path).replaceAll('\\', '/'), ...(await digest(path)) })
  }
}

async function digest(path) {
  const hash = createHash('sha256')
  let bytes = 0
  for await (const chunk of createReadStream(path)) {
    bytes += chunk.length
    hash.update(chunk)
  }
  return { bytes, sha256: hash.digest('hex') }
}
