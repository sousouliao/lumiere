export const macOSPermissionRecoveryCommandChannels = {
  getSnapshot: 'macos-permission-recovery:get-snapshot',
  changed: 'macos-permission-recovery:changed',
  resetAndRestart: 'macos-permission-recovery:reset-and-restart',
  defer: 'macos-permission-recovery:defer',
  openSettings: 'macos-permission-recovery:open-settings',
  requestPermission: 'macos-permission-recovery:request-permission',
  checkAgain: 'macos-permission-recovery:check-again',
  restart: 'macos-permission-recovery:restart',
  copyResetCommand: 'macos-permission-recovery:copy-reset-command',
} as const

export type MacOSPermissionRecoveryPhase =
  'inactive' | 'reset-required' | 'grant-required' | 'restart-required' | 'reset-failed'

export interface MacOSPermissionRecoverySnapshot {
  phase: MacOSPermissionRecoveryPhase
}

export interface LumiereMacOSPermissionRecoveryApi {
  getMacOSPermissionRecoverySnapshot(): Promise<MacOSPermissionRecoverySnapshot>
  onMacOSPermissionRecoveryChanged(
    listener: (snapshot: MacOSPermissionRecoverySnapshot) => void,
  ): () => void
  resetMacOSScreenCapturePermission(): Promise<MacOSPermissionRecoverySnapshot>
  deferMacOSScreenCapturePermissionReset(): Promise<MacOSPermissionRecoverySnapshot>
  openMacOSScreenCaptureSettings(): Promise<void>
  requestMacOSScreenCapturePermission(): Promise<MacOSPermissionRecoverySnapshot>
  checkMacOSScreenCapturePermission(): Promise<MacOSPermissionRecoverySnapshot>
  restartAfterMacOSScreenCapturePermission(): Promise<void>
  copyMacOSScreenCaptureResetCommand(): Promise<void>
}
