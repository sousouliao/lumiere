import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { SettingsView, type UpdateViewState } from './SettingsView'

function renderSystemSettings(platform: 'macos' | 'windows', updateState: UpdateViewState): string {
  return renderToStaticMarkup(
    <SettingsView
      initialSection="system"
      snapshot={null}
      surfaceSnapshot={null}
      platform={platform}
      isSaving={false}
      savingShortcut={null}
      error={null}
      updateState={updateState}
      onDone={() => undefined}
      onOutputDeliveryChange={() => undefined}
      onChooseSaveDirectory={() => undefined}
      onAfterCaptureBehaviorChange={() => undefined}
      onHdrStatusRemindersChange={() => undefined}
      onShortcutChange={() => Promise.resolve()}
      onShortcutRecordingChange={() => Promise.resolve()}
      onCheckForUpdates={() => undefined}
      onOpenLatestRelease={() => undefined}
    />,
  )
}

describe('SettingsView', () => {
  it.each(['macos', 'windows'] as const)(
    'renders accessible settings controls on %s',
    (platform) => {
      const markup = renderToStaticMarkup(
        <SettingsView
          snapshot={{
            outputDelivery: 'clipboard',
            availableOutputDeliveries: ['clipboard'],
            saveDirectory: '/Users/example/Pictures/Screenshots',
            captureShortcuts: {
              region: { accelerator: null, status: 'unconfigured' },
              display: { accelerator: null, status: 'unconfigured' },
            },
            afterCaptureBehavior: 'do-nothing',
            hdrStatusReminders: true,
          }}
          surfaceSnapshot={{
            platform: 'macos',
            hostAvailable: true,
            captureModes: ['display'],
            hdrStatus: 'ready',
            output: {
              delivery: 'clipboard',
              label: 'Clipboard',
              location: '~/Pictures/Lumiere',
            },
          }}
          platform={platform}
          isSaving={false}
          savingShortcut={null}
          error={null}
          updateState={{ status: 'idle', currentVersion: '0.2.0' }}
          onDone={() => undefined}
          onOutputDeliveryChange={() => undefined}
          onChooseSaveDirectory={() => undefined}
          onAfterCaptureBehaviorChange={() => undefined}
          onHdrStatusRemindersChange={() => undefined}
          onShortcutChange={() => Promise.resolve()}
          onShortcutRecordingChange={() => Promise.resolve()}
          onCheckForUpdates={() => undefined}
          onOpenLatestRelease={() => undefined}
        />,
      )

      if (platform === 'windows') {
        expect(markup).toContain('aria-label="Back to capture"')
        expect(markup).toContain('title="Back to capture"')
        expect(markup).not.toContain('settings-done')
      } else {
        expect(markup).toContain('settings-done')
        expect(markup).not.toContain('settings-back')
      }
      expect(markup).toContain('Default destination')
      expect(markup).toContain('aria-haspopup="listbox"')
      expect(markup).toContain('Clipboard and folder')
      const options = markup.match(/<button\b[^>]*role="option"[^>]*>[\s\S]*?<\/button>/g) ?? []
      expect(options).toHaveLength(3)
      expect(options[0]).toContain('>Clipboard<')
      expect(options[0]).toContain('aria-selected="true"')
      expect(options[0]).toContain('tabindex="-1"')
      expect(options[0]).not.toContain('disabled=""')
      expect(options[1]).toContain('>Folder<')
      expect(options[2]).toContain('>Clipboard and folder<')
      for (const option of options.slice(1)) {
        expect(option).toContain('aria-selected="false"')
        expect(option).toContain('disabled=""')
      }
      expect(markup).not.toContain('<select')
      expect(markup).toContain('aria-label="Output settings" aria-pressed="true"')
      expect(markup).toContain('aria-label="Capture settings" aria-pressed="false"')
      expect(markup).toContain('Save folder')
      expect(markup).toContain(
        'aria-label="Choose save folder. Current folder: /Users/example/Pictures/Screenshots"',
      )
      expect(markup).toContain('/Users/example/Pictures/Screenshots')
      expect(markup).toContain('…/Pictures/Screenshots')
      expect(markup).toContain('m9 18 6-6-6-6')
      expect(markup).toContain('File naming')
      expect(markup).toContain('Lumiere-2026-08-25-162345.png')
    },
  )

  it('offers the Ardot-designed manual update action only on macOS', () => {
    const markup = renderSystemSettings('macos', {
      status: 'available',
      currentVersion: '0.2.0',
      availableVersion: '0.3.0',
    })

    expect(markup).toContain('0.2.0 · 0.3.0 available')
    expect(markup).toContain('View update')
    expect(markup).toContain('aria-live="polite"')
  })

  it('shows the real version without update actions on Windows', () => {
    const markup = renderSystemSettings('windows', {
      status: 'idle',
      currentVersion: '0.2.0',
    })

    expect(markup).toContain('settings-row-value--muted">0.2.0')
    expect(markup).not.toContain('Check for updates')
  })

  it.each([
    [{ status: 'idle', currentVersion: '0.2.0' }, '0.2.0', 'Check for updates'],
    [{ status: 'checking', currentVersion: '0.2.0' }, '0.2.0 · Checking…', 'Checking…'],
    [{ status: 'up-to-date', currentVersion: '0.2.0' }, '0.2.0 · Up to date', 'Check again'],
    [{ status: 'failed', currentVersion: '0.2.0' }, '0.2.0 · Couldn’t check', 'Try again'],
  ] as const)('renders the macOS %s update state', (updateState, statusText, actionText) => {
    const markup = renderSystemSettings('macos', updateState)

    expect(markup).toContain(statusText)
    expect(markup).toContain(actionText)
  })
})
