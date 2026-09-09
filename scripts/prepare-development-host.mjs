import { spawn } from 'node:child_process'
import { access, readdir } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')

if (process.platform === 'darwin' && !process.env.LUMIERE_MAC_HOST_PATH) {
  const developerDirectory = process.env.DEVELOPER_DIR || (await discoverXcodeDeveloperDirectory())
  console.log('Preparing current macOS development Host...')
  await run('/usr/bin/xcrun', ['swift', 'build', '--package-path', 'hosts/macos'], {
    cwd: repositoryRoot,
    env: developerDirectory ? { ...process.env, DEVELOPER_DIR: developerDirectory } : process.env,
  })
} else if (process.platform === 'win32' && !process.env.LUMIERE_WINDOWS_HOST_PATH) {
  console.log('Preparing current Windows development Host...')
  await run(
    'dotnet',
    [
      'build',
      'hosts/windows/src/Lumiere.Windows.Host/Lumiere.Windows.Host.csproj',
      '--configuration',
      'Debug',
      '-p:Platform=x64',
      '--verbosity',
      'minimal',
      '/nr:false',
    ],
    { cwd: repositoryRoot, env: process.env },
  )
}

async function discoverXcodeDeveloperDirectory() {
  const entries = await readdir('/Applications', { withFileTypes: true })
  const xcodeApplications = entries
    .filter((entry) => entry.isDirectory() && /^Xcode(?:[-_].+)?\.app$/.test(entry.name))
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

function run(command, args, options) {
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
