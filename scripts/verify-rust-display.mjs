import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const require = createRequire(resolve(root, 'apps/desktop/package.json'))
const Ajv2020 = require('ajv/dist/2020').default
const validator = new Ajv2020({ strict: false, allErrors: true })
for (const version of [5, 6]) {
  validator.addSchema(
    JSON.parse(
      await readFile(resolve(root, `protocol/platform-host/v${version}.schema.json`), 'utf8'),
    ),
  )
}
const folder = resolve(root, 'artifacts/windows/rust-display')
await mkdir(folder, { recursive: true })
const profile = process.argv.includes('--release') ? 'release' : 'debug'
const child = spawn(resolve(root, `target/${profile}/lumiere-windows-host.exe`), [], {
  stdio: ['pipe', 'pipe', 'pipe'],
  windowsHide: true,
})
let pending = ''
let diagnostic = ''
const responses = []
const started = performance.now()
child.stdout.setEncoding('utf8').on('data', (data) => {
  pending += data
  while (pending.includes('\n')) {
    const index = pending.indexOf('\n')
    const response = JSON.parse(pending.slice(0, index))
    pending = pending.slice(index + 1)
    responses.push(response)
    if (response.id === 'display') child.stdin.end()
  }
})
child.stderr.setEncoding('utf8').on('data', (data) => {
  diagnostic += data
})
const completed = new Promise((resolve, reject) => {
  child.once('error', reject)
  child.once('close', (code) =>
    code === 0 ? resolve() : reject(new Error(`Host exit ${code}: ${diagnostic}`)),
  )
})
const timer = setTimeout(() => child.kill(), 30_000)
try {
  child.stdin.write(
    `${JSON.stringify({ version: 5, id: 'caps', method: 'getCapabilities', params: {} })}\n`,
  )
  child.stdin.write(
    `${JSON.stringify({ version: 5, id: 'display', method: 'captureDisplay', params: { delivery: 'folder', saveDirectory: folder } })}\n`,
  )
  await completed
  const capture = responses.find((response) => response.id === 'display')
  const evidence = { elapsedMs: performance.now() - started, responses, diagnostic }
  await writeFile(resolve(folder, 'evidence.json'), `${JSON.stringify(evidence, null, 2)}\n`)
  for (const response of responses) {
    const validate = validator.getSchema(
      'https://lumiere.local/protocol/platform-host/v5.schema.json',
    )
    if (!validate(response)) throw new Error(JSON.stringify(validate.errors))
  }
  if (
    capture?.result?.status !== 'completed' ||
    capture.result.deliveries[0].status !== 'success'
  ) {
    throw new Error(JSON.stringify(evidence))
  }
  console.log(JSON.stringify(evidence, null, 2))
} finally {
  clearTimeout(timer)
}
