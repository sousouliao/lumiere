export interface UpdateSnapshot {
  currentVersion: string
  windowsUpdate?: WindowsUpdateState
}

export type WindowsUpdateState =
  | { status: 'disabled' | 'idle' | 'checking' | 'up-to-date' }
  | { status: 'available' | 'ready' | 'installing'; availableVersion: string }
  | { status: 'downloading'; availableVersion: string; percent: number }
  | {
      status: 'failed'
      message: string
      retry: 'check' | 'download' | 'install'
      availableVersion?: string
    }

export type UpdateCheckResult = UpdateSnapshot &
  ({ status: 'idle' | 'up-to-date' | 'failed' } | { status: 'available'; availableVersion: string })

export interface LumiereUpdateApi {
  getUpdateSnapshot(): Promise<UpdateSnapshot>
  checkForUpdates(): Promise<UpdateCheckResult>
  downloadUpdate(): Promise<UpdateSnapshot>
  installUpdate(): Promise<UpdateSnapshot>
  onUpdateChanged(listener: (snapshot: UpdateSnapshot) => void): () => void
}
