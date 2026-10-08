import { access } from 'node:fs/promises'
import process from 'node:process'

const requiredPaths = [
  'apps/desktop/src-tauri/Cargo.toml',
  'apps/desktop/src/renderer',
  'crates/capture-contract/Cargo.toml',
  'crates/capture-windows/Cargo.toml',
  'hosts/windows/Cargo.toml',
  'hosts/windows/rust/main.rs',
  'protocol/platform-host/v5.schema.json',
  'protocol/platform-host/v6.schema.json',
  'tools/windows-installer/Cargo.toml',
]

const forbiddenPaths = [
  'src',
  'tests',
  'Lumiere.sln',
  'Directory.Build.props',
  'Directory.Packages.props',
  'knowledge/evidence',
  'hosts/windows/Lumiere.Windows.sln',
  'hosts/macos/Package.swift',
  'apps/desktop/electron.vite.config.ts',
  'apps/desktop/src/main/index.ts',
  'apps/desktop/src/preload/index.ts',
]

const missing = []
const forbidden = []

for (const path of requiredPaths) {
  if (!(await exists(path))) {
    missing.push(path)
  }
}

for (const path of forbiddenPaths) {
  if (await exists(path)) {
    forbidden.push(path)
  }
}

if (missing.length > 0 || forbidden.length > 0) {
  if (missing.length > 0) {
    console.error(`Missing required paths:\n${missing.map((path) => `- ${path}`).join('\n')}`)
  }

  if (forbidden.length > 0) {
    console.error(`Forbidden legacy paths:\n${forbidden.map((path) => `- ${path}`).join('\n')}`)
  }

  process.exitCode = 1
}

async function exists(path) {
  try {
    await access(path)
    return true
  } catch {
    return false
  }
}
