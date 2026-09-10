import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { MacOSPermissionRecovery } from './MacOSPermissionRecovery'
import { macOSPermissionRecoveryContent } from './macos-permission-recovery-content'

describe('macOS permission recovery surface', () => {
  it.each([
    ['reset-required', 'Reset and restart', 'Not now', 'Capture paused for update'],
    ['grant-required', 'Open System Settings', 'Check again', 'Waiting for permission'],
    ['restart-required', 'Restart now', 'Check again', 'Restart required'],
    ['reset-failed', 'Copy Terminal command', 'Try again', 'Permission reset failed'],
  ] as const)(
    'renders the %s step with its two bounded actions',
    (phase, first, second, status) => {
      const markup = renderToStaticMarkup(
        <MacOSPermissionRecovery snapshot={{ phase }} disabled={false} />,
      )

      expect(markup).toContain(first)
      expect(markup).toContain(second)
      expect(macOSPermissionRecoveryContent(phase)?.status).toBe(status)
    },
  )

  it('renders nothing when recovery is inactive', () => {
    expect(
      renderToStaticMarkup(
        <MacOSPermissionRecovery snapshot={{ phase: 'inactive' }} disabled={false} />,
      ),
    ).toBe('')
  })
})
