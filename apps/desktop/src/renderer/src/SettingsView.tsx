import { useState } from 'react'
import type { CaptureSurfaceSnapshot } from '../../shared/capture-command'
import type { LumierePlatform, OutputDelivery } from '../../shared/platform-contract'
import {
  formatShortcutAccelerator,
  shortcutFromKeyInput,
  type CaptureMode,
  type ShortcutSnapshot,
  type ShortcutUpdate,
} from '../../shared/shortcut-command'
import {
  afterCaptureBehaviorOptions,
  outputDeliveryOptions,
  parseAfterCaptureBehavior,
  parseOutputDelivery,
  type AfterCaptureBehavior,
  type SettingsSnapshot,
} from '../../shared/settings-command'
import { Button } from '@/components/motion/button/base'
import { Dock, DockItem } from '@/components/motion/dock'
import { FolderPickerButton } from '@/components/motion/folder-picker-button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/motion/select'
import { Switch } from '@/components/motion/switch'

const outputDeliveryLabels: Record<OutputDelivery, string> = {
  clipboard: 'Clipboard',
  folder: 'Folder',
  both: 'Clipboard and folder',
}

const afterCaptureBehaviorLabels: Record<AfterCaptureBehavior, string> = {
  'do-nothing': 'Do nothing',
  'show-in-folder': 'Show in folder',
}

export type SettingsSection = 'output' | 'capture' | 'system'

interface SettingsViewProps {
  initialSection?: SettingsSection
  snapshot: SettingsSnapshot | null
  surfaceSnapshot: CaptureSurfaceSnapshot | null
  platform: LumierePlatform
  isSaving: boolean
  savingShortcut: CaptureMode | null
  error: string | null
  onDone: () => void
  onOutputDeliveryChange: (delivery: OutputDelivery) => void
  onChooseSaveDirectory: () => void
  onAfterCaptureBehaviorChange: (behavior: AfterCaptureBehavior) => void
  onHdrStatusRemindersChange: (enabled: boolean) => void
  onShortcutChange: (update: ShortcutUpdate) => Promise<void>
  onShortcutRecordingChange: (recording: boolean) => Promise<void>
}

export function SettingsView({
  initialSection = 'output',
  snapshot,
  surfaceSnapshot,
  platform,
  isSaving,
  savingShortcut,
  error,
  onDone,
  onOutputDeliveryChange,
  onChooseSaveDirectory,
  onAfterCaptureBehaviorChange,
  onHdrStatusRemindersChange,
  onShortcutChange,
  onShortcutRecordingChange,
}: SettingsViewProps): React.JSX.Element {
  const [section, setSection] = useState<SettingsSection>(initialSection)
  const available = snapshot?.availableOutputDeliveries ?? []
  const selectedDelivery = snapshot?.outputDelivery ?? 'both'

  return (
    <main className={`settings-shell settings-shell--${platform}`}>
      <header
        className={`settings-title-bar settings-title-bar--${platform}`}
        aria-label="Lumiere settings window"
      >
        {platform === 'windows' ? (
          <Button
            variant="ghost"
            size="icon"
            hoverScale={1}
            pressScale={0.98}
            className="settings-back"
            aria-label="Back to capture"
            title="Back to capture"
            onClick={onDone}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M13 8H3M7 4L3 8L7 12" />
            </svg>
          </Button>
        ) : null}
        <h1>Settings</h1>
        {platform !== 'windows' ? (
          <Button
            variant="ghost"
            size="sm"
            hoverScale={1}
            pressScale={0.98}
            className="settings-done"
            onClick={onDone}
          >
            Done
          </Button>
        ) : null}
      </header>

      <nav className="settings-dock-slot" aria-label="Settings sections">
        <Dock fill itemHeight={36} className="settings-dock">
          <DockItem
            className="settings-dock-item"
            active={section === 'output'}
            aria-label="Output settings"
            onClick={() => {
              setSection('output')
            }}
          >
            <OutputIcon />
          </DockItem>
          <DockItem
            className="settings-dock-item"
            active={section === 'capture'}
            aria-label="Capture settings"
            onClick={() => {
              setSection('capture')
            }}
          >
            <CaptureIcon />
          </DockItem>
          <DockItem
            className="settings-dock-item"
            active={section === 'system'}
            aria-label="System and about"
            onClick={() => {
              setSection('system')
            }}
          >
            <AboutIcon />
          </DockItem>
        </Dock>
      </nav>

      <section className="settings-content" aria-label={`${section} settings`}>
        {section === 'output' ? (
          <OutputSettings
            snapshot={snapshot}
            surfaceSnapshot={surfaceSnapshot}
            isSaving={isSaving}
            available={available}
            selectedDelivery={selectedDelivery}
            onOutputDeliveryChange={onOutputDeliveryChange}
            onChooseSaveDirectory={onChooseSaveDirectory}
          />
        ) : null}
        {section === 'capture' ? (
          <CaptureSettings
            surfaceSnapshot={surfaceSnapshot}
            afterCaptureBehavior={snapshot?.afterCaptureBehavior ?? 'do-nothing'}
            hdrStatusReminders={snapshot?.hdrStatusReminders ?? true}
            isSaving={isSaving}
            shortcuts={snapshot?.captureShortcuts ?? null}
            platform={platform}
            savingShortcut={savingShortcut}
            onShortcutChange={onShortcutChange}
            onShortcutRecordingChange={onShortcutRecordingChange}
            onAfterCaptureBehaviorChange={onAfterCaptureBehaviorChange}
            onHdrStatusRemindersChange={onHdrStatusRemindersChange}
          />
        ) : null}
        {section === 'system' ? <SystemSettings snapshot={surfaceSnapshot} /> : null}
        {error ? (
          <p className="settings-error" role="alert">
            {error}
          </p>
        ) : null}
      </section>
    </main>
  )
}

interface OutputSettingsProps {
  snapshot: SettingsSnapshot | null
  surfaceSnapshot: CaptureSurfaceSnapshot | null
  isSaving: boolean
  available: readonly OutputDelivery[]
  selectedDelivery: OutputDelivery
  onOutputDeliveryChange: (delivery: OutputDelivery) => void
  onChooseSaveDirectory: () => void
}

function OutputSettings({
  snapshot,
  surfaceSnapshot,
  isSaving,
  available,
  selectedDelivery,
  onOutputDeliveryChange,
  onChooseSaveDirectory,
}: OutputSettingsProps): React.JSX.Element {
  return (
    <div className="settings-list">
      <div className="settings-row">
        <span className="settings-row-label" id="default-destination-label">
          Default destination
        </span>
        <Select
          value={selectedDelivery}
          disabled={!snapshot || isSaving || available.length === 0}
          onValueChange={(value) => {
            onOutputDeliveryChange(parseOutputDelivery(value))
          }}
          className="settings-select"
        >
          <SelectTrigger
            aria-labelledby="default-destination-label"
            className="settings-control-trigger"
          >
            <SelectValue placeholder={outputDeliveryLabels[selectedDelivery]} />
          </SelectTrigger>
          <SelectContent className="settings-select-content">
            {outputDeliveryOptions.map((delivery) => (
              <SelectItem
                key={delivery}
                value={delivery}
                disabled={!available.includes(delivery)}
                className="settings-select-item"
              >
                {outputDeliveryLabels[delivery]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="settings-row">
        <span className="settings-row-label">Save folder</span>
        <FolderPickerButton
          path={snapshot?.saveDirectory ?? surfaceSnapshot?.output.location ?? '~/Pictures/Lumiere'}
          disabled={!snapshot || isSaving}
          onClick={onChooseSaveDirectory}
        />
      </div>

      <div className="settings-row">
        <span className="settings-row-copy">
          <span className="settings-row-label">File naming</span>
          <span className="settings-row-hint">Lumiere-2026-08-25-162345.png</span>
        </span>
        <span className="settings-row-value settings-row-value--control">Timestamped</span>
      </div>
    </div>
  )
}

function CaptureSettings({
  surfaceSnapshot,
  afterCaptureBehavior,
  hdrStatusReminders,
  isSaving,
  shortcuts,
  platform,
  savingShortcut,
  onShortcutChange,
  onShortcutRecordingChange,
  onAfterCaptureBehaviorChange,
  onHdrStatusRemindersChange,
}: {
  surfaceSnapshot: CaptureSurfaceSnapshot | null
  afterCaptureBehavior: AfterCaptureBehavior
  hdrStatusReminders: boolean
  isSaving: boolean
  shortcuts: SettingsSnapshot['captureShortcuts'] | null
  platform: LumierePlatform
  savingShortcut: CaptureMode | null
  onShortcutChange: (update: ShortcutUpdate) => Promise<void>
  onShortcutRecordingChange: (recording: boolean) => Promise<void>
  onAfterCaptureBehaviorChange: (behavior: AfterCaptureBehavior) => void
  onHdrStatusRemindersChange: (enabled: boolean) => void
}): React.JSX.Element {
  const regionAvailable = surfaceSnapshot?.captureModes.includes('region') === true
  const displayAvailable = surfaceSnapshot?.captureModes.includes('display') === true

  return (
    <div className="settings-list">
      <ShortcutRecorder
        mode="region"
        label="Region shortcut"
        shortcut={shortcuts?.region ?? { accelerator: null, status: 'unconfigured' }}
        platform={platform}
        disabled={!regionAvailable || savingShortcut !== null}
        saving={savingShortcut === 'region'}
        onChange={onShortcutChange}
        onRecordingChange={onShortcutRecordingChange}
      />
      <ShortcutRecorder
        mode="display"
        label="Display shortcut"
        shortcut={shortcuts?.display ?? { accelerator: null, status: 'unconfigured' }}
        platform={platform}
        disabled={!displayAvailable || savingShortcut !== null}
        saving={savingShortcut === 'display'}
        onChange={onShortcutChange}
        onRecordingChange={onShortcutRecordingChange}
      />
      <div className="settings-row">
        <span className="settings-row-label" id="after-capture-label">
          After capture
        </span>
        <Select
          value={afterCaptureBehavior}
          disabled={isSaving}
          onValueChange={(value) => {
            onAfterCaptureBehaviorChange(parseAfterCaptureBehavior(value))
          }}
          className="settings-select"
        >
          <SelectTrigger aria-labelledby="after-capture-label" className="settings-control-trigger">
            <SelectValue placeholder={afterCaptureBehaviorLabels[afterCaptureBehavior]} />
          </SelectTrigger>
          <SelectContent className="settings-select-content">
            {afterCaptureBehaviorOptions.map((behavior) => (
              <SelectItem key={behavior} value={behavior} className="settings-select-item">
                {afterCaptureBehaviorLabels[behavior]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="settings-row">
        <span className="settings-row-copy">
          <span className="settings-row-label">HDR status reminders</span>
          <span className="settings-row-hint">Non-blocking display status alerts</span>
        </span>
        <Switch
          checked={hdrStatusReminders}
          disabled={isSaving}
          ariaLabel="HDR status reminders"
          className="settings-switch"
          onCheckedChange={onHdrStatusRemindersChange}
        />
      </div>
    </div>
  )
}

interface ShortcutRecorderProps {
  mode: CaptureMode
  label: string
  shortcut: ShortcutSnapshot
  platform: LumierePlatform
  disabled: boolean
  saving: boolean
  onChange: (update: ShortcutUpdate) => Promise<void>
  onRecordingChange: (recording: boolean) => Promise<void>
}

function ShortcutRecorder({
  mode,
  label,
  shortcut,
  platform,
  disabled,
  saving,
  onChange,
  onRecordingChange,
}: ShortcutRecorderProps): React.JSX.Element {
  const [recording, setRecording] = useState(false)
  const [inputError, setInputError] = useState<string | null>(null)

  const stopRecording = (): void => {
    if (!recording) return
    setRecording(false)
    void onRecordingChange(false).catch(() => {
      setInputError('Global shortcuts could not be resumed. Restart Lumiere.')
    })
  }

  const commitShortcut = async (update: ShortcutUpdate): Promise<void> => {
    setRecording(false)
    try {
      await onRecordingChange(false)
      await onChange(update)
    } catch {
      setInputError('The shortcut could not be saved. Try again.')
    }
  }

  const startRecording = async (): Promise<void> => {
    if (disabled || recording) return
    setInputError(null)
    try {
      await onRecordingChange(true)
      setRecording(true)
    } catch {
      setInputError('Shortcut recording could not start. Try again.')
    }
  }

  return (
    <div className="shortcut-setting">
      <div className="settings-row">
        <span className="settings-row-copy">
          <span className="settings-row-label">{label}</span>
          {recording ? (
            <span className="settings-row-hint">Press a shortcut · Backspace to clear</span>
          ) : shortcut.status === 'unavailable' ? (
            <span className="settings-row-hint">Could not register this shortcut</span>
          ) : null}
        </span>
        <Button
          variant="ghost"
          size="sm"
          hoverScale={1}
          pressScale={0.98}
          className={`shortcut-recorder${recording ? ' shortcut-recorder--recording' : ''}`}
          disabled={disabled}
          aria-label={`Configure ${label.toLowerCase()}`}
          aria-pressed={recording}
          aria-invalid={shortcut.status === 'unavailable'}
          onClick={() => void startRecording()}
          onBlur={stopRecording}
          onKeyDown={(event) => {
            if (!recording) return
            event.preventDefault()
            event.stopPropagation()
            if (event.key === 'Escape') {
              setInputError(null)
              stopRecording()
              return
            }
            if (event.key === 'Backspace' || event.key === 'Delete') {
              setInputError(null)
              void commitShortcut({ mode, accelerator: null })
              return
            }
            try {
              const accelerator = shortcutFromKeyInput(event, platform)
              setInputError(null)
              void commitShortcut({ mode, accelerator })
            } catch (error) {
              setInputError(error instanceof Error ? error.message : 'Use another shortcut.')
            }
          }}
        >
          {saving
            ? 'Saving…'
            : recording
              ? 'Press keys'
              : formatShortcutAccelerator(shortcut.accelerator, platform)}
        </Button>
      </div>
      {inputError ? (
        <p className="shortcut-setting-error" role="alert">
          {inputError}
        </p>
      ) : null}
    </div>
  )
}

function SystemSettings({
  snapshot,
}: {
  snapshot: CaptureSurfaceSnapshot | null
}): React.JSX.Element {
  const hostAvailable = snapshot?.hostAvailable === true
  const displayAvailable = snapshot?.captureModes.includes('display') === true
  const permissionStatus = !snapshot
    ? 'Checking…'
    : !hostAvailable
      ? 'Unknown'
      : displayAvailable
        ? 'Available'
        : 'Needs attention'

  return (
    <div className="settings-list settings-list--system">
      <SettingsRow
        label="Screen recording permission"
        value={permissionStatus}
        tone={displayAvailable ? 'ready' : 'muted'}
      />
      <SettingsRow
        label="Native capture host"
        value={!snapshot ? 'Checking…' : hostAvailable ? 'Connected' : 'Unavailable'}
        tone={hostAvailable ? 'ready' : 'muted'}
      />
      <SettingsRow label="Version" value="0.2.0-preview.1" />
      <p className="settings-semantics-note">
        Native HDR-aware capture. Everyday output is sRGB Visual Match. Copied and saved mean
        delivered, not certified.
      </p>
    </div>
  )
}

interface SettingsRowProps {
  label: string
  hint?: string
  value: string
  control?: boolean
  tone?: 'ready' | 'muted'
}

function SettingsRow({
  label,
  hint,
  value,
  control = false,
  tone = 'muted',
}: SettingsRowProps): React.JSX.Element {
  return (
    <div className="settings-row">
      <span className="settings-row-copy">
        <span className="settings-row-label">{label}</span>
        {hint ? <span className="settings-row-hint">{hint}</span> : null}
      </span>
      <span className={`settings-row-value settings-row-value--${control ? 'control' : tone}`}>
        {value}
      </span>
    </div>
  )
}

function OutputIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 18 18" aria-hidden="true">
      <rect x="2.5" y="3" width="13" height="12" rx="2" />
      <circle cx="6.25" cy="7" r="1.15" />
      <path d="m4.25 12 3-3 2.25 2.25 1.5-1.5 2.75 2.75" />
    </svg>
  )
}

function CaptureIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 18 18" aria-hidden="true">
      <path d="M6 2.5H3.5a1 1 0 0 0-1 1V6m9.5-3.5h2.5a1 1 0 0 1 1 1V6m0 6v2.5a1 1 0 0 1-1 1H12m-6 0H3.5a1 1 0 0 1-1-1V12" />
    </svg>
  )
}

function AboutIcon(): React.JSX.Element {
  return (
    <svg viewBox="0 0 18 18" aria-hidden="true">
      <circle cx="9" cy="9" r="6.25" />
      <path d="M9 8.25v4M9 5.5h.01" />
    </svg>
  )
}
