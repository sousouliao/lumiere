import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { CaptureCommandResult } from '../shared/capture-command'
import { MacOSPermissionRecovery } from './macos-permission-recovery'

const temporaryDirectories: string[] = []
const permissionDenied: CaptureCommandResult = {
  status: 'failed',
  feedback: 'Permission required',
  notice: {
    tone: 'critical',
    title: 'Screen recording permission is required',
    detail: 'Allow screen recording in System Settings, then try again.',
    recovery: 'permissions',
  },
}

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((path) => rm(path, { recursive: true })))
})

describe('MacOSPermissionRecovery', () => {
  it('does not offer an upgrade reset on a first installation', async () => {
    const recovery = await createRecovery({ hasPriorInstallation: false })

    await recovery.observeCaptureResult(permissionDenied)

    expect(recovery.getSnapshot()).toEqual({ phase: 'inactive' })
  })

  it('offers a reset to users upgrading from a version without recovery state', async () => {
    const recovery = await createRecovery({ hasPriorInstallation: true })

    await recovery.observeCaptureResult(permissionDenied)

    expect(recovery.getSnapshot()).toEqual({ phase: 'reset-required' })
  })

  it('detects later version changes and carries the grant step across a relaunch', async () => {
    const filePath = await statePath()
    await writeFile(
      filePath,
      JSON.stringify({ version: 1, lastLaunchedVersion: '0.3.0', phase: 'inactive' }),
      'utf8',
    )
    const relaunch = vi.fn()
    const resetPermission = vi.fn(() => Promise.resolve())
    const recovery = await createRecovery({ filePath, relaunch, resetPermission })
    await recovery.observeCaptureResult(permissionDenied)

    await recovery.resetAndRestart()

    expect(resetPermission).toHaveBeenCalledOnce()
    expect(relaunch).toHaveBeenCalledOnce()
    expect(recovery.getSnapshot()).toEqual({ phase: 'grant-required' })

    const relaunched = await createRecovery({ filePath })
    expect(relaunched.getSnapshot()).toEqual({ phase: 'grant-required' })
  })

  it('requires an observed grant before offering the final restart', async () => {
    const filePath = await statePath()
    let granted = false
    const recovery = await createRecovery({
      filePath,
      hasPriorInstallation: true,
      permissionIsGranted: () => granted,
    })
    await recovery.observeCaptureResult(permissionDenied)
    await recovery.resetAndRestart()

    expect(await recovery.checkAgain()).toEqual({ phase: 'grant-required' })
    granted = true
    expect(await recovery.checkAgain()).toEqual({ phase: 'restart-required' })

    const relaunched = await createRecovery({ filePath })
    expect(relaunched.getSnapshot()).toEqual({ phase: 'inactive' })
  })

  it('keeps the reset available after deferring and exposes a bounded failure state', async () => {
    const recovery = await createRecovery({
      hasPriorInstallation: true,
      resetPermission: vi.fn(() => Promise.reject(new Error('tccutil failed'))),
    })
    await recovery.observeCaptureResult(permissionDenied)
    expect(await recovery.defer()).toEqual({ phase: 'inactive' })

    await recovery.observeCaptureResult(permissionDenied)
    expect(await recovery.resetAndRestart()).toEqual({ phase: 'reset-failed' })
  })

  it('retires the recovery for the current version after a successful capture', async () => {
    const filePath = await statePath()
    const recovery = await createRecovery({ filePath, hasPriorInstallation: true })
    await recovery.observeCaptureResult({ status: 'success', feedback: 'Copied' })
    await recovery.observeCaptureResult(permissionDenied)

    expect(recovery.getSnapshot()).toEqual({ phase: 'inactive' })
    await expect(readFile(filePath, 'utf8')).resolves.not.toContain('recoveryVersion')
  })
})

async function createRecovery({
  filePath,
  hasPriorInstallation = false,
  resetPermission = vi.fn(() => Promise.resolve()),
  permissionIsGranted = () => false,
  relaunch = vi.fn(),
}: {
  filePath?: string
  hasPriorInstallation?: boolean
  resetPermission?: () => Promise<void>
  permissionIsGranted?: () => boolean
  relaunch?: () => void
} = {}): Promise<MacOSPermissionRecovery> {
  const recovery = new MacOSPermissionRecovery({
    filePath: filePath ?? (await statePath()),
    currentVersion: '0.4.0',
    hasPriorInstallation,
    resetPermission,
    permissionIsGranted,
    relaunch,
    changed: vi.fn(),
  })
  await recovery.load()
  return recovery
}

async function statePath(): Promise<string> {
  const directory = await mkdtemp(join(tmpdir(), 'lumiere-permission-recovery-'))
  temporaryDirectories.push(directory)
  return join(directory, 'state.json')
}
