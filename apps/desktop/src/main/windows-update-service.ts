import type { UpdateSnapshot, WindowsUpdateState } from '../shared/update-command'
import { compareVersions } from './manual-update-check'

export interface WindowsUpdateClient {
  checkForUpdates(): Promise<{ updateInfo: { version: string } } | null>
  downloadUpdate(): Promise<string[]>
  quitAndInstall(isSilent: boolean, isForceRunAfter: boolean): void
  on(event: 'download-progress', listener: (progress: { percent: number }) => void): void
  on(event: 'update-downloaded', listener: () => void): void
  on(event: 'error', listener: (error: Error) => void): void
}

/** One main-owned update lifetime, independent of windows and capture results. */
export class WindowsUpdateService {
  private state: WindowsUpdateState
  private operation: Promise<UpdateSnapshot> | null = null
  private readonly firstCheck: NodeJS.Timeout | null
  private interval: NodeJS.Timeout | null = null
  private disposed = false

  public constructor(
    private readonly currentVersion: string,
    private readonly client: WindowsUpdateClient | null,
    private readonly changed: (snapshot: UpdateSnapshot) => void,
    private readonly captureBusy: () => boolean,
    private readonly stopHost: () => Promise<void>,
    private readonly resumeHost: () => void,
  ) {
    this.state = { status: client ? 'idle' : 'disabled' }
    this.firstCheck = client
      ? setTimeout(() => {
          void this.check()
          this.interval = setInterval(() => void this.check(), 6 * 60 * 60 * 1_000)
          this.interval.unref()
        }, 30_000)
      : null
    this.firstCheck?.unref()
    client?.on('download-progress', ({ percent }) => {
      if (this.state.status === 'downloading') {
        this.publish({ ...this.state, percent: Math.min(100, Math.max(0, percent)) })
      }
    })
    client?.on('update-downloaded', () => {
      if (this.state.status === 'downloading') {
        this.publish({ status: 'ready', availableVersion: this.state.availableVersion })
      }
    })
    client?.on('error', (error) => {
      if (this.state.status === 'installing') {
        this.resumeHost()
        this.fail(
          'install',
          'Couldn’t restart for the update. Try again.',
          error,
          this.state.availableVersion,
        )
      }
    })
  }

  public getSnapshot(): UpdateSnapshot {
    return { currentVersion: this.currentVersion, windowsUpdate: this.state }
  }

  public isInstalling(): boolean {
    return this.state.status === 'installing'
  }

  public check(): Promise<UpdateSnapshot> {
    const client = this.client
    if (this.operation) return this.operation
    if (!client || this.disposed || ['ready', 'installing'].includes(this.state.status)) {
      return Promise.resolve(this.getSnapshot())
    }
    return this.run(async () => {
      this.publish({ status: 'checking' })
      try {
        const result = await client.checkForUpdates()
        if (!result) throw new Error('Update information is unavailable.')
        const version = result.updateInfo.version
        this.publish(
          compareVersions(version, this.currentVersion) > 0
            ? { status: 'available', availableVersion: version }
            : { status: 'up-to-date' },
        )
      } catch (error) {
        this.fail('check', 'Couldn’t check for updates. Try again.', error)
      }
    })
  }

  public download(): Promise<UpdateSnapshot> {
    const client = this.client
    if (this.operation) return this.operation
    const state = this.state
    if (
      !client ||
      this.disposed ||
      !(
        state.status === 'available' ||
        (state.status === 'failed' && state.retry === 'download')
      ) ||
      !state.availableVersion
    )
      return Promise.resolve(this.getSnapshot())
    const availableVersion = state.availableVersion
    return this.run(async () => {
      this.publish({ status: 'downloading', availableVersion, percent: 0 })
      try {
        await client.downloadUpdate()
        this.publish({ status: 'ready', availableVersion })
      } catch (error) {
        this.fail('download', 'Couldn’t download the update. Try again.', error, availableVersion)
      }
    })
  }

  public install(): Promise<UpdateSnapshot> {
    const client = this.client
    if (this.operation) return this.operation
    const state = this.state
    if (
      !client ||
      this.disposed ||
      !(state.status === 'ready' || (state.status === 'failed' && state.retry === 'install')) ||
      !state.availableVersion
    )
      return Promise.resolve(this.getSnapshot())
    if (this.captureBusy()) return Promise.resolve(this.getSnapshot())
    const availableVersion = state.availableVersion
    // Installing is visible synchronously, before a competing capture can start.
    this.publish({ status: 'installing', availableVersion })
    return this.run(async () => {
      try {
        await this.stopHost()
        client.quitAndInstall(true, true)
      } catch (error) {
        this.resumeHost()
        this.fail('install', 'Couldn’t restart for the update. Try again.', error, availableVersion)
      }
    })
  }

  public dispose(): void {
    this.disposed = true
    if (this.firstCheck) clearTimeout(this.firstCheck)
    if (this.interval) clearInterval(this.interval)
  }

  private run(action: () => Promise<void>): Promise<UpdateSnapshot> {
    const operation = Promise.resolve()
      .then(action)
      .then(() => this.getSnapshot())
    this.operation = operation
    void operation.finally(() => {
      if (this.operation === operation) this.operation = null
    })
    return operation
  }

  private publish(state: WindowsUpdateState): void {
    if (this.disposed) return
    this.state = state
    this.changed(this.getSnapshot())
  }

  private fail(
    retry: 'check' | 'download' | 'install',
    message: string,
    error: unknown,
    availableVersion?: string,
  ): void {
    console.error(`operation=WindowsUpdate stage=${retry} result=failed`, error)
    this.publish({
      status: 'failed',
      retry,
      message,
      ...(availableVersion ? { availableVersion } : {}),
    })
  }
}
