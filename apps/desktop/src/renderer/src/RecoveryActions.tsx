import { useState } from 'react'
import { Button } from '@/components/motion/button/base'
import {
  captureRecoveryActions,
  type CaptureCompletion,
  type CaptureRecoveryAction,
} from '../../shared/capture-command'
import type { LumierePlatform } from '../../shared/platform-contract'

export function RecoveryActions({
  completion,
  platform,
  disabled = false,
}: {
  completion: CaptureCompletion
  platform: LumierePlatform
  disabled?: boolean
}): React.JSX.Element | null {
  const [pending, setPending] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const actions = captureRecoveryActions(completion.result, platform)
  if (!actions.length) return null
  const recover = async (action: CaptureRecoveryAction): Promise<void> => {
    setPending(true)
    setError(null)
    try {
      await window.lumierePlatform.recoverCapture(completion.id, action)
    } catch {
      setError(
        action === 'refresh'
          ? 'Could not check capture availability. Try again.'
          : 'Could not open the recovery action. Try again.',
      )
    } finally {
      setPending(false)
    }
  }
  return (
    <div className="capture-recovery">
      <div className="capture-recovery-actions">
        {actions.map(({ action, label }) => (
          <Button
            key={action}
            variant="ghost"
            size="sm"
            hoverScale={1}
            pressScale={1}
            className="capture-recovery-action"
            disabled={disabled || pending}
            onClick={() => void recover(action)}
          >
            {label}
          </Button>
        ))}
      </div>
      {error ? (
        <p className="capture-recovery-error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  )
}
