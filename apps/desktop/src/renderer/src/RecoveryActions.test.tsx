import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { RecoveryActions } from './RecoveryActions'
import { captureRecoveryActions, type CaptureCommandResult } from '../../shared/capture-command'

function render(result: CaptureCommandResult, disabled = false): string {
  return renderToStaticMarkup(
    <RecoveryActions
      completion={{ id: 1, mode: 'region', result }}
      platform="macos"
      disabled={disabled}
    />,
  )
}

describe('capture recovery actions', () => {
  it('does not add controls to success or cancellation', () => {
    expect(render({ status: 'success', feedback: 'Copied' })).toBe('')
    expect(render({ status: 'cancelled', feedback: 'Cancelled' })).toBe('')
  })
  it('offers a folder fix without claiming to redeliver the old image', () => {
    const markup = render(
      {
        status: 'partial',
        feedback: 'Copied, not saved',
        notice: {
          tone: 'caution',
          title: 'Save failed',
          detail: 'Choose a folder.',
          recovery: 'folder',
        },
      },
      true,
    )
    expect(markup).toContain('Choose save folder')
    expect(markup).toContain('Capture again')
    expect(markup.match(/disabled=""/g)).toHaveLength(2)
  })
  it('offers platform-appropriate permission recovery and recheck', () => {
    const result: CaptureCommandResult = {
      status: 'failed',
      feedback: 'Permission required',
      notice: {
        tone: 'critical',
        title: 'Permission required',
        detail: 'Allow screen recording.',
        recovery: 'permissions',
      },
    }
    expect(render(result)).toContain('Open System Settings')
    expect(render(result)).toContain('Check again')
    expect(captureRecoveryActions(result, 'windows').map(({ action }) => action)).not.toContain(
      'open-permissions',
    )
  })
})
