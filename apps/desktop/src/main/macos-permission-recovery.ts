import { mkdir, readFile, rename, writeFile } from 'node:fs/promises'
import { dirname } from 'node:path'
import type { CaptureCommandResult } from '../shared/capture-command'
import type {
  MacOSPermissionRecoveryPhase,
  MacOSPermissionRecoverySnapshot,
} from '../shared/macos-permission-recovery-command'

const STATE_VERSION = 1 as const

interface PersistedState {
  version: typeof STATE_VERSION
  lastLaunchedVersion: string
  recoveryVersion?: string
  phase: MacOSPermissionRecoveryPhase
}

interface MacOSPermissionRecoveryOptions {
  filePath: string
  currentVersion: string
  hasPriorInstallation: boolean
  resetPermission(): Promise<void>
  permissionIsGranted(): boolean
  relaunch(): void
  changed(snapshot: MacOSPermissionRecoverySnapshot): void
}

export class MacOSPermissionRecovery {
  private state: PersistedState
  private pendingWrite: Promise<void> = Promise.resolve()

  public constructor(private readonly options: MacOSPermissionRecoveryOptions) {
    this.state = {
      version: STATE_VERSION,
      lastLaunchedVersion: options.currentVersion,
      phase: 'inactive',
    }
  }

  public async load(): Promise<void> {
    const persisted = await this.readState()
    const updated = persisted
      ? persisted.lastLaunchedVersion !== this.options.currentVersion
      : this.options.hasPriorInstallation
    const continuingCurrentRecovery = persisted?.recoveryVersion === this.options.currentVersion
    const restoredPhase = continuingCurrentRecovery ? persisted.phase : 'inactive'

    this.state = {
      version: STATE_VERSION,
      lastLaunchedVersion: this.options.currentVersion,
      ...(updated || continuingCurrentRecovery
        ? { recoveryVersion: this.options.currentVersion }
        : {}),
      phase: restoredPhase === 'restart-required' ? 'inactive' : restoredPhase,
    }
    await this.persist()
  }

  public getSnapshot(): MacOSPermissionRecoverySnapshot {
    return { phase: this.state.phase }
  }

  public async observeCaptureResult(result: CaptureCommandResult): Promise<void> {
    if (result.status === 'success' && this.state.recoveryVersion === this.options.currentVersion) {
      await this.transition('inactive', false)
      return
    }
    if (
      result.status === 'failed' &&
      result.notice.recovery === 'permissions' &&
      this.state.recoveryVersion === this.options.currentVersion &&
      (this.state.phase === 'inactive' || this.state.phase === 'reset-required')
    ) {
      await this.transition('reset-required')
    }
  }

  public async resetAndRestart(): Promise<MacOSPermissionRecoverySnapshot> {
    this.assertPhase('reset-required', 'reset-failed')
    try {
      await this.options.resetPermission()
      await this.transition('grant-required')
    } catch {
      await this.transition('reset-failed')
      return this.getSnapshot()
    }
    this.options.relaunch()
    return this.getSnapshot()
  }

  public async defer(): Promise<MacOSPermissionRecoverySnapshot> {
    this.assertPhase('reset-required')
    await this.transition('inactive')
    return this.getSnapshot()
  }

  public async checkAgain(): Promise<MacOSPermissionRecoverySnapshot> {
    this.assertPhase('grant-required', 'restart-required')
    if (this.options.permissionIsGranted()) {
      await this.transition('restart-required')
    }
    return this.getSnapshot()
  }

  public restart(): void {
    this.assertPhase('restart-required')
    this.options.relaunch()
  }

  private async transition(
    phase: MacOSPermissionRecoveryPhase,
    keepRecoveryVersion = true,
  ): Promise<void> {
    this.state = {
      ...this.state,
      phase,
      ...(keepRecoveryVersion ? {} : { recoveryVersion: undefined }),
    }
    await this.persist()
    this.options.changed(this.getSnapshot())
  }

  private assertPhase(...allowed: MacOSPermissionRecoveryPhase[]): void {
    if (!allowed.includes(this.state.phase)) {
      throw new Error('The macOS permission recovery action is unavailable.')
    }
  }

  private async readState(): Promise<PersistedState | null> {
    try {
      const value: unknown = JSON.parse(await readFile(this.options.filePath, 'utf8'))
      return parseState(value)
    } catch {
      return null
    }
  }

  private async persist(): Promise<void> {
    const write = async (): Promise<void> => {
      const temporaryPath = `${this.options.filePath}.tmp`
      await mkdir(dirname(this.options.filePath), { recursive: true })
      await writeFile(temporaryPath, `${JSON.stringify(this.state, null, 2)}\n`, 'utf8')
      await rename(temporaryPath, this.options.filePath)
    }
    this.pendingWrite = this.pendingWrite.then(write, write)
    await this.pendingWrite
  }
}

function parseState(value: unknown): PersistedState {
  if (
    typeof value !== 'object' ||
    value === null ||
    Array.isArray(value) ||
    !('version' in value) ||
    value.version !== STATE_VERSION ||
    !('lastLaunchedVersion' in value) ||
    typeof value.lastLaunchedVersion !== 'string' ||
    !('phase' in value) ||
    !isPhase(value.phase) ||
    ('recoveryVersion' in value && typeof value.recoveryVersion !== 'string')
  ) {
    throw new Error('The macOS permission recovery state is invalid.')
  }
  return {
    version: STATE_VERSION,
    lastLaunchedVersion: value.lastLaunchedVersion,
    ...('recoveryVersion' in value && typeof value.recoveryVersion === 'string'
      ? { recoveryVersion: value.recoveryVersion }
      : {}),
    phase: value.phase,
  }
}

function isPhase(value: unknown): value is MacOSPermissionRecoveryPhase {
  return (
    value === 'inactive' ||
    value === 'reset-required' ||
    value === 'grant-required' ||
    value === 'restart-required' ||
    value === 'reset-failed'
  )
}
