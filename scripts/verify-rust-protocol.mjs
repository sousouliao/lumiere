import { spawn } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const require = createRequire(resolve(root, 'apps/desktop/package.json'))
const Ajv2020 = require('ajv/dist/2020').default
const validator = new Ajv2020({ strict: false, allErrors: true })
for (let version = 1; version <= 6; version++) {
  validator.addSchema(
    JSON.parse(
      await readFile(resolve(root, `protocol/platform-host/v${version}.schema.json`), 'utf8'),
    ),
  )
}
const host = resolve(root, 'target/debug/lumiere-windows-host.exe')
const child = spawn(host, [], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true })
let output = ''
let diagnostic = ''
child.stdout.setEncoding('utf8').on('data', (data) => {
  output += data
})
child.stderr.setEncoding('utf8').on('data', (data) => {
  diagnostic += data
})
const completion = new Promise((resolve, reject) => {
  child.once('error', reject)
  child.once('exit', (code) =>
    code === 0 ? resolve() : reject(new Error(`Host exited ${code}: ${diagnostic}`)),
  )
})
const timer = setTimeout(() => {
  child.kill()
}, 10_000)
try {
  for (const version of [5, 6]) {
    child.stdin.write(
      `${JSON.stringify({ version, id: `caps-${version}`, method: 'getCapabilities', params: {} })}\n`,
    )
    child.stdin.write(
      `${JSON.stringify({ version, id: `invalid-${version}`, method: 'getCapabilities', params: { extra: true } })}\n`,
    )
  }
  child.stdin.write(
    `${JSON.stringify({ version: 6, id: 'cancel', method: 'cancelRegion', params: { requestId: 'missing' } })}\n`,
  )
  child.stdin.end()
  await completion
  const lines = output.trim().split('\n')
  if (lines.length !== 5) throw new Error(`Expected 5 correlated responses, got ${lines.length}`)
  for (const line of lines) {
    const response = JSON.parse(line)
    const validate = validator.getSchema(
      `https://lumiere.local/protocol/platform-host/v${response.version}.schema.json`,
    )
    if (!validate(response)) throw new Error(JSON.stringify(validate.errors))
  }
  console.log('Rust Host responses conform to the existing v5/v6 JSON Schemas (5 cases).')
} finally {
  clearTimeout(timer)
}
