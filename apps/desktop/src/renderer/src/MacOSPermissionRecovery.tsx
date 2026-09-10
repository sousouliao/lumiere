import { useState } from 'react'
import { Button } from '@/components/motion/button/base'
import type { MacOSPermissionRecoverySnapshot } from '../../shared/macos-permission-recovery-command'
import { macOSPermissionRecoveryContent } from './macos-permission-recovery-content'

export function MacOSPermissionRecovery({
  snapshot,
  disabled,
}: {
  snapshot: MacOSPermissionRecoverySnapshot
  disabled: boolean
}): React.JSX.Element | null {
  const content = macOSPermissionRecoveryContent(snapshot.phase)
  const [pendingAction, setPendingAction] = useState<string | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  if (!content) return null

  const run = async (action: string, operation: () => Promise<unknown>): Promise<void> => {
    setPendingAction(action)
    setMessage(null)
    try {
      const result = await operation()
      if (action === 'copy') setMessage('Terminal command copied.')
      if (
        action === 'check' &&
        typeof result === 'object' &&
        result !== null &&
        'phase' in result &&
        result.phase === 'grant-required'
      ) {
        setMessage('Permission is not available yet. Turn on Lumiere, then check again.')
      }
    } catch {
      setMessage('That action didn’t finish. Try again.')
    } finally {
      setPendingAction(null)
    }
  }

  const controls = (() => {
    switch (snapshot.phase) {
      case 'reset-required':
        return (
          <>
            <RecoveryButton
              label={pendingAction === 'reset' ? 'Resetting…' : 'Reset and restart'}
              disabled={disabled || pendingAction !== null}
              onClick={() =>
                void run('reset', () => window.lumierePlatform.resetMacOSScreenCapturePermission())
              }
            />
            <RecoveryButton
              label="Not now"
              disabled={disabled || pendingAction !== null}
              onClick={() =>
                void run('defer', () =>
                  window.lumierePlatform.deferMacOSScreenCapturePermissionReset(),
                )
              }
            />
          </>
        )
      case 'grant-required':
        return (
          <>
            <RecoveryButton
              label="Open System Settings"
              disabled={disabled || pendingAction !== null}
              onClick={() =>
                void run('settings', () => window.lumierePlatform.openMacOSScreenCaptureSettings())
              }
            />
            <RecoveryButton
              label={pendingAction === 'check' ? 'Checking…' : 'Check again'}
              disabled={disabled || pendingAction !== null}
              onClick={() =>
                void run('check', () => window.lumierePlatform.checkMacOSScreenCapturePermission())
              }
            />
          </>
        )
      case 'restart-required':
        return (
          <>
            <RecoveryButton
              label="Restart now"
              disabled={disabled || pendingAction !== null}
              onClick={() =>
                void run('restart', () =>
                  window.lumierePlatform.restartAfterMacOSScreenCapturePermission(),
                )
              }
            />
            <RecoveryButton
              label={pendingAction === 'check' ? 'Checking…' : 'Check again'}
              disabled={disabled || pendingAction !== null}
              onClick={() =>
                void run('check', () => window.lumierePlatform.checkMacOSScreenCapturePermission())
              }
            />
          </>
        )
      case 'reset-failed':
        return (
          <>
            <RecoveryButton
              label={pendingAction === 'copy' ? 'Copying…' : 'Copy Terminal command'}
              disabled={disabled || pendingAction !== null}
              onClick={() =>
                void run('copy', () => window.lumierePlatform.copyMacOSScreenCaptureResetCommand())
              }
            />
            <RecoveryButton
              label={pendingAction === 'reset' ? 'Trying again…' : 'Try again'}
              disabled={disabled || pendingAction !== null}
              onClick={() =>
                void run('reset', () => window.lumierePlatform.resetMacOSScreenCapturePermission())
              }
            />
          </>
        )
      case 'inactive':
        return null
    }
  })()

  return (
    <div className="blocking-recovery macos-permission-recovery" role="status">
      <div className="notice-copy">
        <h1>{content.title}</h1>
        <p>{content.detail}</p>
      </div>
      <div className="capture-recovery">
        <div className="capture-recovery-actions">{controls}</div>
        {message ? <p className="capture-recovery-error">{message}</p> : null}
      </div>
    </div>
  )
}

function RecoveryButton({
  label,
  disabled,
  onClick,
}: {
  label: string
  disabled: boolean
  onClick: () => void
}): React.JSX.Element {
  return (
    <Button
      variant="ghost"
      size="sm"
      hoverScale={1}
      pressScale={1}
      className="capture-recovery-action"
      disabled={disabled}
      onClick={onClick}
    >
      {label}
    </Button>
  )
}
