import type { MacOSPermissionRecoveryPhase } from '../../shared/macos-permission-recovery-command'

interface RecoveryContent {
  title: string
  detail: string
  status: string
  tone: 'critical' | 'caution'
}

export function macOSPermissionRecoveryContent(
  phase: MacOSPermissionRecoveryPhase,
): RecoveryContent | null {
  switch (phase) {
    case 'inactive':
      return null
    case 'reset-required':
      return {
        title: 'Screen recording access needs a reset',
        detail: 'macOS needs fresh permission for this version of Lumiere.',
        status: 'Capture paused for update',
        tone: 'critical',
      }
    case 'grant-required':
      return {
        title: 'Allow screen recording again',
        detail: 'Open System Settings, turn on Lumiere, then return here.',
        status: 'Waiting for permission',
        tone: 'caution',
      }
    case 'restart-required':
      return {
        title: 'Restart Lumiere to finish',
        detail: 'After turning on access in System Settings, restart Lumiere once.',
        status: 'Restart required',
        tone: 'caution',
      }
    case 'reset-failed':
      return {
        title: 'Permission reset didn’t finish',
        detail: 'Copy the Terminal command, run it once, then reopen Lumiere.',
        status: 'Permission reset failed',
        tone: 'critical',
      }
  }
}
