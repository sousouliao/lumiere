import { useCallback, useEffect, useRef, useState } from 'react'
import { Button } from '@/components/motion/button/base'
import type {
  CaptureCommandResult,
  CaptureActivity,
  CaptureCompletion,
  CaptureNotice,
  CaptureSurfaceSnapshot,
} from '../../shared/capture-command'
import type { OutputDelivery } from '../../shared/platform-contract'
import type { CaptureMode, ShortcutUpdate } from '../../shared/shortcut-command'
import type { SettingsSnapshot } from '../../shared/settings-command'
import type { AfterCaptureBehavior } from '../../shared/settings-command'
import type { AutostartSnapshot } from '../../shared/autostart-command'
import { SettingsView, type SettingsSection, type UpdateViewState } from './SettingsView'
import { RecoveryActions } from './RecoveryActions'
import { CAPTURE_LOAD_FAILURE, resolveCaptureNotices } from './capture-notices'
import { WindowControls } from './WindowControls'
import { observeSnapshot } from './observe-snapshot'

export function App(): React.JSX.Element {
  return <ApplicationSurface />
}

function ApplicationSurface(): React.JSX.Element {
  const [view, setView] = useState<'capture' | 'settings'>(() =>
    new URLSearchParams(window.location.search).get('view') === 'settings' ? 'settings' : 'capture',
  )
  const [settingsSection, setSettingsSection] = useState<SettingsSection>('output')
  const [activity, setActivity] = useState<CaptureActivity>({
    activeMode: null,
    lastCompletion: null,
  })
  const [captureResult, setCaptureResult] = useState<CaptureCommandResult | null>(null)
  const [surfaceSnapshot, setSurfaceSnapshot] = useState<CaptureSurfaceSnapshot | null>(null)
  const [surfaceLoadFailed, setSurfaceLoadFailed] = useState(false)

  useEffect(() => {
    const stopSettingsListening = window.lumierePlatform.onShowSettingsRequested(() => {
      setSettingsSection('output')
      setView('settings')
    })
    const stopCaptureViewListening = window.lumierePlatform.onShowCaptureRequested(() => {
      setView('capture')
    })
    const stopCaptureListening = observeSnapshot({
      read: () => window.lumierePlatform.getCaptureActivity(),
      subscribe: (listener) => window.lumierePlatform.onCaptureActivityChanged(listener),
      onSnapshot: (next, source) => {
        setActivity(next)
        if (source === 'initial') return
        setCaptureResult(null)
        if (
          next.lastCompletion?.result.status === 'failed' ||
          next.lastCompletion?.result.status === 'partial'
        )
          setView('capture')
      },
    })
    const stopSurfaceListening = observeSnapshot({
      read: () => window.lumierePlatform.getCaptureSurfaceSnapshot(),
      subscribe: (listener) => window.lumierePlatform.onCaptureSurfaceChanged(listener),
      onSnapshot: (snapshot, source) => {
        setSurfaceSnapshot(snapshot)
        setSurfaceLoadFailed(false)
        if (source === 'event') setCaptureResult(null)
      },
      onError: () => {
        setSurfaceLoadFailed(true)
      },
    })
    return () => {
      stopSettingsListening()
      stopCaptureListening()
      stopCaptureViewListening()
      stopSurfaceListening()
    }
  }, [])

  return view === 'settings' ? (
    <SettingsWindow
      initialSection={settingsSection}
      surfaceSnapshot={surfaceSnapshot}
      onDone={() => {
        setView('capture')
      }}
    />
  ) : (
    <MainWindow
      snapshot={surfaceSnapshot}
      loadFailed={surfaceLoadFailed}
      result={captureResult ?? activity.lastCompletion?.result ?? null}
      activity={activity}
      onResultChange={setCaptureResult}
      onOpenSettings={(section = 'output') => {
        setSettingsSection(section)
        setView('settings')
      }}
    />
  )
}

function MainWindow({
  snapshot,
  loadFailed,
  result,
  activity,
  onResultChange,
  onOpenSettings,
}: {
  snapshot: CaptureSurfaceSnapshot | null
  loadFailed: boolean
  result: CaptureCommandResult | null
  activity: CaptureActivity
  onResultChange: (result: CaptureCommandResult | null) => void
  onOpenSettings: (section?: SettingsSection) => void
}): React.JSX.Element {
  const capturingMode = activity.activeMode
  const [interactionHint, setInteractionHint] = useState<string | null>(null)
  const [detailsOpen, setDetailsOpen] = useState(false)
  const statusSummaryRef = useRef<HTMLButtonElement>(null)

  const captureDisplay = async (): Promise<void> => {
    onResultChange(null)
    setInteractionHint(null)
    try {
      await window.lumierePlatform.captureDisplay()
    } catch {
      onResultChange({
        status: 'failed',
        feedback: CAPTURE_LOAD_FAILURE.title,
        notice: CAPTURE_LOAD_FAILURE,
      })
    }
  }

  const captureRegion = async (): Promise<void> => {
    onResultChange(null)
    setInteractionHint(null)
    try {
      await window.lumierePlatform.captureRegion()
    } catch {
      onResultChange({
        status: 'failed',
        feedback: CAPTURE_LOAD_FAILURE.title,
        notice: CAPTURE_LOAD_FAILURE,
      })
    }
  }

  const supportsRegionCapture =
    snapshot?.hostAvailable === true && snapshot.captureModes.includes('region')
  const supportsDisplayCapture =
    snapshot?.hostAvailable === true && snapshot.captureModes.includes('display')
  const resolvedNotices = resolveCaptureNotices({
    loadFailed,
    result,
    snapshot,
  })
  const { activeNotice, blockingNotice, detailNotice } = resolvedNotices
  const resultCompletion =
    activity.lastCompletion?.result === result ? activity.lastCompletion : null
  const completionNotice =
    resultCompletion?.result.status === 'failed' || resultCompletion?.result.status === 'partial'
      ? resultCompletion.result.notice
      : undefined
  const detailCompletion = detailNotice === completionNotice ? resultCompletion : null
  const captureBlocked = blockingNotice !== undefined

  const closeDetails = useCallback((): void => {
    setDetailsOpen(false)
    requestAnimationFrame(() => statusSummaryRef.current?.focus())
  }, [])

  useEffect(() => {
    if (!detailsOpen) return
    const handleKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      closeDetails()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [closeDetails, detailsOpen])

  return (
    <main className="app-shell">
      <header
        className={`title-bar title-bar--${window.lumierePlatform.platform}`}
        aria-label="Lumiere window"
        data-tauri-drag-region
      >
        <span className="window-title" data-tauri-drag-region>
          Lumiere
        </span>
        <WindowControls />
      </header>

      <section
        className={`capture-panel${
          detailsOpen && detailNotice
            ? ' capture-panel--details'
            : blockingNotice
              ? ' capture-panel--blocking'
              : ''
        }`}
        aria-label="Capture controls"
      >
        {detailsOpen && detailNotice ? (
          <NoticeDetails
            notice={detailNotice}
            completion={detailCompletion}
            disabled={capturingMode !== null}
            onBack={closeDetails}
            onOpenSettings={() => {
              onOpenSettings('capture')
            }}
          />
        ) : (
          <>
            {blockingNotice ? (
              <BlockingRecovery
                notice={blockingNotice}
                completion={blockingNotice === completionNotice ? resultCompletion : null}
                disabled={capturingMode !== null}
              />
            ) : (
              <div className="capture-actions">
                <Button
                  variant="secondary"
                  size="lg"
                  pressScale={0.99}
                  hoverScale={1}
                  className="capture-action capture-action--region"
                  disabled={captureBlocked || !supportsRegionCapture || capturingMode !== null}
                  onClick={() => void captureRegion()}
                  onFocus={() => {
                    setInteractionHint('Drag to select an area')
                  }}
                  onBlur={() => {
                    setInteractionHint(null)
                  }}
                  onPointerEnter={() => {
                    setInteractionHint('Drag to select an area')
                  }}
                  onPointerLeave={() => {
                    setInteractionHint(null)
                  }}
                >
                  <RegionIcon />
                  <span>{capturingMode === 'region' ? 'Capturing region…' : 'Capture region'}</span>
                  {capturingMode === 'region' ? (
                    <span className="capture-pulse" aria-hidden="true" />
                  ) : null}
                </Button>

                <Button
                  variant="secondary"
                  size="lg"
                  pressScale={0.99}
                  hoverScale={1}
                  className="capture-action"
                  disabled={captureBlocked || !supportsDisplayCapture || capturingMode !== null}
                  onClick={() => void captureDisplay()}
                  onFocus={() => {
                    setInteractionHint('Capture the active display')
                  }}
                  onBlur={() => {
                    setInteractionHint(null)
                  }}
                  onPointerEnter={() => {
                    setInteractionHint('Capture the active display')
                  }}
                  onPointerLeave={() => {
                    setInteractionHint(null)
                  }}
                >
                  <DisplayIcon />
                  <span>
                    {capturingMode === 'display' ? 'Capturing display…' : 'Capture display'}
                  </span>
                  {capturingMode === 'display' ? (
                    <span className="capture-pulse" aria-hidden="true" />
                  ) : null}
                </Button>
              </div>
            )}

            <div className="output-summary" aria-label="Current output">
              <span className="output-label">Output</span>
              <span className="output-value">
                {snapshot?.output.label ?? 'Clipboard and folder'}
              </span>
              <span className="output-location">
                {snapshot?.output.location ?? '~/Pictures/Lumiere'}
              </span>
            </div>
          </>
        )}
      </section>

      <footer className="status-bar">
        {detailNotice ? (
          <Button
            ref={statusSummaryRef}
            variant="ghost"
            size="sm"
            hoverScale={1}
            pressScale={0.98}
            className="status-summary"
            aria-expanded={detailsOpen}
            onClick={() => {
              setDetailsOpen(true)
            }}
            disabled={detailsOpen}
          >
            <span className={`status-dot status-dot--${statusTone(snapshot, activeNotice)}`} />
            <span className="status-message" aria-live="polite">
              {statusMessage({
                activeNotice,
                captureBlocked,
                interactionHint,
                capturingMode,
                result,
                snapshot,
              })}
            </span>
            {!detailsOpen ? <ChevronRightIcon /> : null}
          </Button>
        ) : (
          <div className="status-summary status-summary--static">
            <span className={`status-dot status-dot--${statusTone(snapshot, activeNotice)}`} />
            <span className="status-message" aria-live="polite">
              {statusMessage({
                activeNotice,
                captureBlocked,
                interactionHint,
                capturingMode,
                result,
                snapshot,
              })}
            </span>
          </div>
        )}
        <Button
          variant="ghost"
          size="sm"
          hoverScale={1}
          pressScale={0.98}
          className="settings-link"
          onClick={() => {
            onOpenSettings()
          }}
        >
          Settings
        </Button>
      </footer>
    </main>
  )
}

function SettingsWindow({
  initialSection,
  surfaceSnapshot,
  onDone,
}: {
  initialSection: SettingsSection
  surfaceSnapshot: CaptureSurfaceSnapshot | null
  onDone: () => void
}): React.JSX.Element {
  const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null)
  const [isSaving, setIsSaving] = useState(false)
  const [savingShortcut, setSavingShortcut] = useState<CaptureMode | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [updateState, setUpdateState] = useState<UpdateViewState>({ status: 'loading' })
  const [captureBusy, setCaptureBusy] = useState(false)
  const [autostart, setAutostart] = useState<AutostartSnapshot | null>(null)
  const [autostartSaving, setAutostartSaving] = useState(false)
  const [autostartError, setAutostartError] = useState<string | null>(null)
  const autostartRequest = useRef(0)
  const autostartWriting = useRef(false)

  const refreshAutostart = useCallback(async (): Promise<void> => {
    if (autostartWriting.current) return
    const request = ++autostartRequest.current
    try {
      const next = await window.lumierePlatform.getAutostartSnapshot()
      if (request === autostartRequest.current) {
        setAutostart(next)
        setAutostartError(null)
      }
    } catch {
      if (request === autostartRequest.current) {
        setAutostart({
          status: 'unavailable',
          message: 'Unable to read launch at login. Try again.',
        })
      }
    }
  }, [])

  useEffect(() => {
    const requests = autostartRequest
    const refresh = (): void => {
      void refreshAutostart()
    }
    refresh()
    window.addEventListener('focus', refresh)
    return () => {
      window.removeEventListener('focus', refresh)
      ++requests.current
    }
  }, [refreshAutostart])

  const setAutostartEnabled = async (enabled: boolean): Promise<void> => {
    if (autostartWriting.current) return
    autostartWriting.current = true
    const request = ++autostartRequest.current
    setAutostartSaving(true)
    setAutostartError(null)
    try {
      const next = await window.lumierePlatform.setAutostartEnabled(enabled)
      if (request === autostartRequest.current) setAutostart(next)
    } catch {
      if (request === autostartRequest.current) {
        setAutostartError(
          'Unable to update launch at login. Try again or check Windows Startup Apps.',
        )
      }
      // A write may succeed before its readback fails. Re-query rather than showing
      // a stale toggle or claiming the operation was rolled back.
      try {
        const next = await window.lumierePlatform.getAutostartSnapshot()
        if (request === autostartRequest.current) setAutostart(next)
      } catch {
        if (request === autostartRequest.current)
          setAutostart({
            status: 'unavailable',
            message: 'Unable to read launch at login. Try again.',
          })
      }
    } finally {
      autostartWriting.current = false
      if (request === autostartRequest.current) setAutostartSaving(false)
    }
  }

  const openStartupSettings = async (): Promise<void> => {
    setAutostartError(null)
    try {
      await window.lumierePlatform.openStartupSettings()
    } catch {
      setAutostartError('Open Windows Settings → Apps → Startup to enable Lumiere.')
    }
  }

  useEffect(
    () =>
      observeSnapshot({
        read: () => window.lumierePlatform.getSettingsSnapshot(),
        subscribe: (listener) => window.lumierePlatform.onSettingsChanged(listener),
        onSnapshot: setSnapshot,
        onError: () => {
          setError('Settings could not be loaded. Restart Lumiere and try again.')
        },
      }),
    [],
  )

  useEffect(
    () =>
      observeSnapshot({
        read: () => window.lumierePlatform.getUpdateSnapshot(),
        subscribe: (listener) => window.lumierePlatform.onUpdateChanged(listener),
        onSnapshot: (next) => {
          setUpdateState({ status: 'idle', ...next })
        },
        // Keep the control disabled when local version metadata is unavailable.
      }),
    [],
  )

  useEffect(
    () =>
      observeSnapshot({
        read: () => window.lumierePlatform.getCaptureActivity(),
        subscribe: (listener) => window.lumierePlatform.onCaptureActivityChanged(listener),
        onSnapshot: (activity) => {
          setCaptureBusy(activity.activeMode !== null)
        },
        onError: () => {
          setCaptureBusy(true)
        },
      }),
    [],
  )

  const performWindowsUpdate = async (action: 'download' | 'install'): Promise<void> => {
    try {
      const next =
        action === 'download'
          ? await window.lumierePlatform.downloadUpdate()
          : await window.lumierePlatform.installUpdate()
      setUpdateState({ status: 'idle', ...next })
    } catch {
      setError('Unable to complete update. Please try again.')
    }
  }

  const checkForUpdates = async (): Promise<void> => {
    if (updateState.status === 'loading') return
    const { currentVersion } = updateState
    setUpdateState({ status: 'checking', currentVersion, windowsUpdate: { status: 'checking' } })
    setError(null)
    try {
      setUpdateState(await window.lumierePlatform.checkForUpdates())
    } catch {
      setUpdateState({
        status: 'failed',
        currentVersion,
        windowsUpdate: { status: 'failed', message: 'Unable to check for updates', retry: 'check' },
      })
    }
  }

  const setOutputDelivery = async (delivery: OutputDelivery): Promise<void> => {
    setIsSaving(true)
    setError(null)
    try {
      setSnapshot(await window.lumierePlatform.setOutputDelivery(delivery))
    } catch {
      setError('Unable to save output destination. Please try again.')
    } finally {
      setIsSaving(false)
    }
  }

  const chooseSaveDirectory = async (): Promise<void> => {
    setIsSaving(true)
    setError(null)
    try {
      setSnapshot(await window.lumierePlatform.chooseSaveDirectory())
    } catch {
      setError('Unable to change save folder. Please try again.')
    } finally {
      setIsSaving(false)
    }
  }

  const setAfterCaptureBehavior = async (behavior: AfterCaptureBehavior): Promise<void> => {
    setIsSaving(true)
    setError(null)
    try {
      setSnapshot(await window.lumierePlatform.setAfterCaptureBehavior(behavior))
    } catch {
      setError('Unable to save after-capture behavior. Please try again.')
    } finally {
      setIsSaving(false)
    }
  }

  const setHideMainWindowDuringCapture = async (enabled: boolean): Promise<void> => {
    setIsSaving(true)
    setError(null)
    try {
      setSnapshot(await window.lumierePlatform.setHideMainWindowDuringCapture(enabled))
    } catch {
      setError('Unable to save window hiding preference. Please try again.')
    } finally {
      setIsSaving(false)
    }
  }

  const setHdrStatusReminders = async (enabled: boolean): Promise<void> => {
    setIsSaving(true)
    setError(null)
    try {
      setSnapshot(await window.lumierePlatform.setHdrStatusReminders(enabled))
    } catch {
      setError('Unable to save HDR display alerts. Please try again.')
    } finally {
      setIsSaving(false)
    }
  }

  const setCaptureShortcut = async (update: ShortcutUpdate): Promise<void> => {
    setSavingShortcut(update.mode)
    setError(null)
    try {
      const result = await window.lumierePlatform.setCaptureShortcut(update)
      if (result.status === 'failed') {
        setError(result.message)
      } else {
        setSnapshot(result.snapshot)
      }
    } catch {
      setError('Unable to save shortcut. Please try again.')
    } finally {
      setSavingShortcut(null)
    }
  }

  return (
    <SettingsView
      initialSection={initialSection}
      autostart={autostart}
      autostartSaving={autostartSaving}
      autostartError={autostartError}
      onAutostartChange={(enabled) => void setAutostartEnabled(enabled)}
      onAutostartRefresh={() => void refreshAutostart()}
      onOpenStartupSettings={() => void openStartupSettings()}
      snapshot={snapshot}
      surfaceSnapshot={surfaceSnapshot}
      platform={window.lumierePlatform.platform}
      isSaving={isSaving}
      savingShortcut={savingShortcut}
      error={error}
      updateState={updateState}
      onDone={onDone}
      onOutputDeliveryChange={(delivery) => void setOutputDelivery(delivery)}
      onChooseSaveDirectory={() => void chooseSaveDirectory()}
      onAfterCaptureBehaviorChange={(behavior) => void setAfterCaptureBehavior(behavior)}
      onHideMainWindowDuringCaptureChange={(enabled) =>
        void setHideMainWindowDuringCapture(enabled)
      }
      onHdrStatusRemindersChange={(enabled) => void setHdrStatusReminders(enabled)}
      onShortcutChange={setCaptureShortcut}
      onShortcutRecordingChange={(recording) => {
        if (recording) setError(null)
        return window.lumierePlatform.setShortcutRecording(recording)
      }}
      onCheckForUpdates={() => void checkForUpdates()}
      onDownloadUpdate={() => void performWindowsUpdate('download')}
      onInstallUpdate={() => void performWindowsUpdate('install')}
      captureBusy={captureBusy}
    />
  )
}

function BlockingRecovery({
  notice,
  completion,
  disabled,
}: {
  notice: CaptureNotice
  completion: CaptureCompletion | null
  disabled: boolean
}): React.JSX.Element {
  const [refreshing, setRefreshing] = useState(false)
  const [refreshError, setRefreshError] = useState<string | null>(null)
  const refresh = async (): Promise<void> => {
    setRefreshing(true)
    setRefreshError(null)
    try {
      await window.lumierePlatform.refreshCaptureSurface()
    } catch {
      setRefreshError('Unable to check capture availability. Please try again.')
    } finally {
      setRefreshing(false)
    }
  }
  return (
    <div className="blocking-recovery" role="status">
      <div className="notice-copy">
        <h1>{notice.title}</h1>
        <p>{notice.detail}</p>
      </div>
      {completion &&
      (completion.result.status === 'failed' || completion.result.status === 'partial') ? (
        <RecoveryActions key={completion.id} completion={completion} disabled={disabled} />
      ) : notice.tone === 'critical' ? (
        <Button
          variant="ghost"
          size="sm"
          hoverScale={1}
          pressScale={1}
          className="capture-recovery-action"
          disabled={disabled || refreshing}
          onClick={() => void refresh()}
        >
          {refreshing ? 'Checking…' : 'Check again'}
        </Button>
      ) : null}
      {refreshError ? <p role="alert">{refreshError}</p> : null}
    </div>
  )
}

function NoticeDetails({
  notice,
  completion,
  disabled,
  onBack,
  onOpenSettings,
}: {
  notice: CaptureNotice
  completion: CaptureCompletion | null
  disabled: boolean
  onBack: () => void
  onOpenSettings: () => void
}): React.JSX.Element {
  return (
    <div className="notice-details">
      <Button
        variant="ghost"
        size="sm"
        hoverScale={1}
        pressScale={0.98}
        className="notice-details-back"
        onClick={onBack}
      >
        <ChevronLeftIcon />
        Back
      </Button>
      <div className="notice-copy">
        <h1>{notice.title}</h1>
        <p>{notice.detail}</p>
      </div>
      {completion ? (
        <RecoveryActions completion={completion} disabled={disabled} />
      ) : (
        <Button
          variant="ghost"
          size="sm"
          hoverScale={1}
          pressScale={0.98}
          className="notice-details-action"
          onClick={onOpenSettings}
        >
          Alert settings
        </Button>
      )}
    </div>
  )
}

interface StatusMessageInput {
  snapshot: CaptureSurfaceSnapshot | null
  result: CaptureCommandResult | null
  activeNotice: CaptureNotice | undefined
  captureBlocked: boolean
  interactionHint: string | null
  capturingMode: 'region' | 'display' | null
}

function statusMessage({
  activeNotice,
  captureBlocked,
  interactionHint,
  capturingMode,
  result,
  snapshot,
}: StatusMessageInput): string {
  if (capturingMode) {
    return capturingMode === 'region' ? 'Capturing region…' : 'Capturing display…'
  }
  if (result && result.status !== 'cancelled') {
    return result.feedback
  }
  if (captureBlocked) {
    return 'Capture unavailable'
  }
  if (activeNotice) {
    return activeNotice.title
  }
  if (interactionHint) {
    return interactionHint
  }
  if (!snapshot) {
    return 'Checking…'
  }
  if (snapshot.hdrStatus === 'ready') {
    return 'HDR-aware capture ready'
  }
  return 'Capture ready'
}

function statusTone(
  snapshot: CaptureSurfaceSnapshot | null,
  activeNotice: CaptureNotice | undefined,
): 'ready' | 'caution' | 'critical' {
  if (activeNotice?.tone === 'critical') {
    return 'critical'
  }
  if (!snapshot || activeNotice) {
    return 'caution'
  }
  return 'ready'
}

function RegionIcon(): React.JSX.Element {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16">
      <path d="M2.5 6V3.5a1 1 0 0 1 1-1H6M10 2.5h2.5a1 1 0 0 1 1 1V6M13.5 10v2.5a1 1 0 0 1-1 1H10M6 13.5H3.5a1 1 0 0 1-1-1V10" />
    </svg>
  )
}

function DisplayIcon(): React.JSX.Element {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16">
      <rect x="2.25" y="2.75" width="11.5" height="8.25" rx="1.25" />
      <path d="M6 13.25h4M8 11v2.25" />
    </svg>
  )
}

function ChevronLeftIcon(): React.JSX.Element {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16">
      <path d="m9.5 4-4 4 4 4" />
    </svg>
  )
}

function ChevronRightIcon(): React.JSX.Element {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16">
      <path d="m6.5 4 4 4-4 4" />
    </svg>
  )
}
