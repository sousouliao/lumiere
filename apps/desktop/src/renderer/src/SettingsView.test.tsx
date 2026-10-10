import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { SettingsView, type UpdateViewState } from './SettingsView'
import type { AutostartSnapshot } from '../../shared/autostart-command'

const autostartProps = {
  autostart: null,
  autostartSaving: false,
  autostartError: null,
  onAutostartChange: () => undefined,
  onAutostartRefresh: () => undefined,
  onOpenStartupSettings: () => undefined,
}

function renderSystemSettings(
  platform: 'windows',
  updateState: UpdateViewState,
  autostart: AutostartSnapshot | null = null,
  saving = false,
  error: string | null = null,
): string {
  return renderToStaticMarkup(
    <SettingsView
      {...autostartProps}
      autostart={autostart}
      autostartSaving={saving}
      autostartError={error}
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
    />,
  )
}

describe('SettingsView', () => {
  it.each([
    [null, false, false, 'Checking…'],
    [{ status: 'disabled' }, false, true, 'Start in the system tray'],
    [{ status: 'enabled' }, true, true, 'Start in the system tray'],
    [{ status: 'blocked' }, false, false, 'Disabled in Windows'],
    [
      { status: 'unavailable', message: 'Could not read startup status' },
      false,
      false,
      'Could not read startup status',
    ],
  ] as const)(
    'renders the actual login registration state %s',
    (snapshot, checked, enabled, hint) => {
      const markup = renderSystemSettings(
        'windows',
        { status: 'idle', currentVersion: '0.7.1' },
        snapshot,
      )
      const control = /<button[^>]*aria-label="Launch at login"[^>]*>/.exec(markup)?.[0]
      expect(control).toBeDefined()
      expect(control).toContain(`aria-checked="${String(checked)}"`)
      expect(control?.includes('disabled=""')).toBe(!enabled)
      expect(markup).toContain(hint)
      if (snapshot?.status === 'blocked') expect(markup).toContain('Windows settings')
      if (snapshot?.status === 'unavailable') expect(markup).toContain('Try again')
    },
  )

  it('disables login registration while saving and reports a failure separately', () => {
    const markup = renderSystemSettings(
      'windows',
      { status: 'idle', currentVersion: '0.7.1' },
      { status: 'disabled' },
      true,
      'Launch at login could not be changed.',
    )
    expect(/<button[^>]*aria-label="Launch at login"[^>]*>/.exec(markup)?.[0]).toContain(
      'disabled=""',
    )
    expect(markup).toContain('Saving…')
    expect(markup).toContain('role="alert">Launch at login could not be changed.')
  })

  it('renders accessible Windows settings controls', () => {
    const markup = renderToStaticMarkup(
      <SettingsView
        {...autostartProps}
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
          platform: 'windows',
          hostAvailable: true,
          captureModes: ['display'],
          hdrStatus: 'ready',
          output: {
            delivery: 'clipboard',
            label: 'Clipboard',
            location: '~/Pictures/Lumiere',
          },
        }}
        platform="windows"
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
      />,
    )

    expect(markup).toContain('aria-label="Back to capture"')
    expect(markup).toContain('title="Back to capture"')
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
  })

  it('shows the real version without update actions when Windows updates are disabled', () => {
    const markup = renderSystemSettings('windows', {
      status: 'idle',
      currentVersion: '0.2.0',
    })

    expect(markup).toContain('settings-row-value--muted">0.2.0')
    expect(markup).not.toContain('Check for updates')
  })

  it.each([
    [{ status: 'idle' }, 'Check for updates'],
    [{ status: 'checking' }, 'Checking…'],
    [{ status: 'up-to-date' }, 'Check again'],
    [{ status: 'failed', retry: 'check', message: 'Unable to check for updates' }, 'Try again'],
    [{ status: 'available', availableVersion: '0.6.1' }, 'Download update'],
    [{ status: 'downloading', availableVersion: '0.6.1', percent: 42 }, 'Downloading 42%'],
    [{ status: 'ready', availableVersion: '0.6.1' }, 'Restart to update'],
    [
      { status: 'failed', retry: 'download', message: 'Unable to download or verify the update' },
      'Try again',
    ],
  ] as const)(
    'shows the Windows %s update action in the existing version row',
    (windowsUpdate, text) => {
      const markup = renderSystemSettings('windows', {
        status: 'idle',
        currentVersion: '0.6.0',
        windowsUpdate,
      })
      expect(markup).toContain(text)
      expect(markup).toContain('aria-live="polite"')
      expect(markup).not.toContain('View update')
    },
  )
})
