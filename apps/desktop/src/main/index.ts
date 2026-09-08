import {
  app,
  BrowserWindow,
  dialog,
  globalShortcut,
  ipcMain,
  protocol,
  screen,
  shell,
} from 'electron'
import { readFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { performance } from 'node:perf_hooks'
import {
  applyMacDockIcon,
  createApplicationTray,
  desktopIconPaths,
  type ApplicationTray,
} from './app-icons'
import type { ApplicationTrayState } from './tray-menu'
import { CaptureSession } from './capture-session'
import { CaptureFailureNotifier } from './capture-failure-notifier'
import { captureRecoveryActions, type CaptureRecoveryAction } from '../shared/capture-command'
import { CaptureCommandRouter } from './capture-command-router'
import { CaptureSurfaceMonitor } from './capture-surface-monitor'
import { MacOSPlatformHost } from './macos-platform-host'
import { macOSHostCandidates, windowsHostCandidates } from './native-host-paths'
import { NativeProcessPlatformHost } from './native-process-platform-host'
import { currentLumierePlatform } from './platform-host'
import { WindowsPlatformHost } from './windows-platform-host'
import { SettingsStore } from './settings-store'
import { ShortcutRegistrationError, ShortcutService } from './shortcut-service'
import { applyAfterCaptureBehavior } from './after-capture'
import { RegionPreviewRegistry, regionPreviewScheme } from './region-preview-registry'
import { RegionDisplayWatcher } from './region-display-watcher'
import { RegionOverlayController } from './region-overlay-controller'
import { resolveRegionTargetWithRetry } from './region-target-resolver'
import { captureCommandChannels, type CaptureCommandResult } from '../shared/capture-command'
import {
  availableOutputDeliveries,
  parseAfterCaptureBehavior,
  parseHdrStatusReminders,
  parseOutputDelivery,
  settingsCommandChannels,
  type SettingsSnapshot,
} from '../shared/settings-command'
import { parseShortcutUpdate } from '../shared/shortcut-command'
import { configureWindowsUpdates } from './windows-updater'
import {
  parseCaptureGeometry,
  type CaptureGeometry,
  type LogicalSize,
  type PlatformHost,
} from '../shared/platform-contract'

protocol.registerSchemesAsPrivileged([
  {
    scheme: regionPreviewScheme,
    privileges: { standard: true, secure: true, supportFetchAPI: true },
  },
])

const regionPreviewDirectory = join(tmpdir(), 'lumiere-region-preview')
const regionPreviewRegistry = new RegionPreviewRegistry(regionPreviewDirectory)

if (process.platform === 'win32') {
  app.setAppUserModelId('io.github.sousouliao.lumiere')
}

let mainWindow: BrowserWindow | null = null
let applicationTray: ApplicationTray | null = null
let platformHost: PlatformHost | null = null
let captureRouter: CaptureCommandRouter | null = null
let captureSurfaceMonitor: CaptureSurfaceMonitor | null = null
let settingsStore: SettingsStore | null = null
let shortcutService: ShortcutService | null = null
let regionOverlaySession: RegionOverlaySession | null = null
let regionOverlayController: RegionOverlayController | null = null
let nextRegionOverlayGeneration = 0
let quitting = false
let lastTrayState: ApplicationTrayState | null = null
const captureSession = new CaptureSession((activity) => {
  if (quitting) return
  if (activity.activeMode) captureFailureNotifier.clear()
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send(captureCommandChannels.activityChanged, activity)
  }
  updateApplicationTray()
})

const captureFailureNotifier = new CaptureFailureNotifier((id) => {
  if (!quitting && captureSession.getSnapshot().lastCompletion?.id === id) showCaptureWindow()
})

interface RegionOverlaySession {
  generation: number
  activeDisplayId: number | null
  desiredDisplayId: number
  targetSize: LogicalSize
  previewToken: string
  previewUrl: string
  leaseTimeout: NodeJS.Timeout
  displayWatcher: RegionDisplayWatcher
  switchPromise: Promise<void> | null
  ready: boolean
  submitted: boolean
  resolve(result: CaptureCommandResult): void
  stopWatchingDisplays(): void
  restoreMainWindow: boolean
  timingStartedAt: number
  timingLastAt: number
}

function reportRegionCaptureTiming(
  stage: string,
  startedAt: number,
  details: Record<string, number> = {},
): void {
  process.stderr.write(
    `${JSON.stringify({
      level: 'info',
      event: 'region-capture-timing',
      stage,
      elapsedMilliseconds: Math.round(performance.now() - startedAt),
      ...details,
    })}\n`,
  )
}

function createRendererWindow(): BrowserWindow {
  const window = new BrowserWindow({
    width: 480,
    height: 370,
    minWidth: 440,
    minHeight: 340,
    show: false,
    title: 'Lumiere',
    backgroundColor: '#1f1d1b',
    ...(process.platform === 'darwin'
      ? {
          titleBarStyle: 'hiddenInset' as const,
          trafficLightPosition: { x: 12, y: 15 },
        }
      : {}),
    ...(process.platform === 'win32'
      ? {
          autoHideMenuBar: true,
          icon: desktopIconPaths().appIcon,
          titleBarStyle: 'hidden' as const,
          titleBarOverlay: {
            color: '#1f1d1b',
            symbolColor: '#f0ede6',
            height: 46,
          },
        }
      : {}),
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  })

  window.once('ready-to-show', () => {
    window.show()
    setImmediate(() => regionOverlayController?.prewarm())
  })
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
  window.webContents.on('will-navigate', (event) => {
    event.preventDefault()
  })
  window.once('closed', () => {
    shortcutService?.setRecording(false)
    if (mainWindow === window) {
      mainWindow = null
      if (process.platform === 'win32') regionOverlayController?.dispose()
    }
  })
  bindCaptureSurfaceMonitoring(window)

  if (!app.isPackaged && process.env.ELECTRON_RENDERER_URL) {
    void window.loadURL(process.env.ELECTRON_RENDERER_URL)
  } else {
    void window.loadFile(join(__dirname, '../renderer/index.html'))
  }

  return window
}

function showMainWindow(): BrowserWindow {
  if (!mainWindow || mainWindow.isDestroyed()) {
    mainWindow = createRendererWindow()
  }

  mainWindow.show()
  mainWindow.focus()
  return mainWindow
}

function bindCaptureSurfaceMonitoring(window: BrowserWindow): void {
  if (!captureSurfaceMonitor) return
  window.on('show', () => {
    captureSurfaceMonitor?.start()
  })
  window.on('focus', () => {
    captureFailureNotifier.clear()
    void captureSurfaceMonitor?.refresh()
  })
  window.on('restore', () => {
    captureSurfaceMonitor?.start()
    void captureSurfaceMonitor?.refresh()
  })
  window.on('moved', () => {
    void captureSurfaceMonitor?.refresh()
  })
  window.on('hide', () => {
    captureSurfaceMonitor?.stop()
  })
  window.on('minimize', () => {
    captureSurfaceMonitor?.stop()
  })
  window.on('closed', () => {
    captureSurfaceMonitor?.stop()
  })
}

function refreshCaptureSurfaceForDisplayChange(): void {
  void captureSurfaceMonitor?.invalidate()
}

function registerRegionPreviewProtocol(): void {
  protocol.handle(regionPreviewScheme, async (request) => {
    if (request.method !== 'GET') {
      return new Response(null, { status: 405 })
    }
    const filePath = regionPreviewRegistry.resolve(request.url)
    if (!filePath) {
      return new Response(null, { status: 404 })
    }
    try {
      const readStartedAt = performance.now()
      const data = await readFile(filePath)
      const timingSession = regionOverlaySession
      if (timingSession) {
        reportSessionTiming(timingSession, 'preview-read', {
          readMilliseconds: Math.round(performance.now() - readStartedAt),
          previewBytes: data.byteLength,
        })
      }
      return new Response(data, {
        status: 200,
        headers: {
          'Content-Type': 'image/png',
          'Cache-Control': 'no-store',
        },
      })
    } catch {
      return new Response(null, { status: 410 })
    }
  })
}

function registerIpc(): void {
  platformHost ??= createPlatformHost()
  settingsStore ??= new SettingsStore(join(app.getPath('userData'), 'settings.json'))
  const router = (captureRouter ??= new CaptureCommandRouter(
    currentLumierePlatform(),
    platformHost,
    settingsStore,
  ))
  regionOverlayController ??= new RegionOverlayController({
    preloadPath: join(__dirname, '../preload/index.js'),
    rendererDirectory: join(__dirname, '../renderer'),
    ...(!app.isPackaged && process.env.ELECTRON_RENDERER_URL
      ? { rendererUrl: process.env.ELECTRON_RENDERER_URL }
      : {}),
    onSessionFailure: (generation) => {
      if (regionOverlaySession?.generation === generation) {
        void failRegionOverlay(router, captureFailedResult())
      }
    },
    onTiming: (stage) => {
      const session = regionOverlaySession
      if (session) reportSessionTiming(session, stage)
    },
  })
  captureSurfaceMonitor ??= new CaptureSurfaceMonitor({
    readTargetId: () => screen.getDisplayNearestPoint(screen.getCursorScreenPoint()).id,
    readSnapshot: () => router.getSurfaceSnapshot(),
    publish: (snapshot) => {
      if (mainWindow && !mainWindow.isDestroyed()) {
        mainWindow.webContents.send(captureCommandChannels.surfaceChanged, snapshot)
      }
    },
  })
  if (mainWindow) {
    bindCaptureSurfaceMonitoring(mainWindow)
  }

  ipcMain.removeHandler(captureCommandChannels.getSurfaceSnapshot)
  ipcMain.removeHandler(captureCommandChannels.captureDisplay)
  ipcMain.removeHandler(captureCommandChannels.captureRegion)
  ipcMain.removeAllListeners(captureCommandChannels.regionOverlayHostReady)
  ipcMain.removeAllListeners(captureCommandChannels.regionOverlayReady)
  ipcMain.removeAllListeners(captureCommandChannels.cancelRegionOverlay)
  ipcMain.removeAllListeners(captureCommandChannels.submitRegionSelection)
  ipcMain.removeHandler(settingsCommandChannels.getSnapshot)
  ipcMain.removeHandler(settingsCommandChannels.chooseSaveDirectory)
  ipcMain.removeHandler(settingsCommandChannels.setAfterCaptureBehavior)
  ipcMain.removeHandler(settingsCommandChannels.setCaptureShortcut)
  ipcMain.removeHandler(settingsCommandChannels.setHdrStatusReminders)
  ipcMain.removeHandler(settingsCommandChannels.setOutputDelivery)
  ipcMain.removeHandler(settingsCommandChannels.setShortcutRecording)

  const assertTrustedWindow = (
    event: Electron.IpcMainEvent | Electron.IpcMainInvokeEvent,
    trustedWindow: BrowserWindow | null,
  ): void => {
    if (
      !trustedWindow ||
      trustedWindow.isDestroyed() ||
      trustedWindow.webContents.id !== event.sender.id ||
      event.senderFrame !== event.sender.mainFrame
    ) {
      throw new Error('Rejected IPC from an untrusted renderer.')
    }
  }

  const assertNoArguments = (args: readonly unknown[]): void => {
    if (args.length !== 0) {
      throw new Error('Rejected unexpected IPC arguments.')
    }
  }

  ipcMain.handle(captureCommandChannels.getSurfaceSnapshot, (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    assertNoArguments(args)
    return captureSurfaceMonitor?.getSnapshot() ?? router.getSurfaceSnapshot()
  })

  ipcMain.handle(captureCommandChannels.captureDisplay, async (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    assertNoArguments(args)
    return runCapture('display')
  })

  ipcMain.handle(captureCommandChannels.captureRegion, async (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    assertNoArguments(args)
    return runCapture('region')
  })

  ipcMain.handle(captureCommandChannels.getActivity, (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    assertNoArguments(args)
    return captureSession.getSnapshot()
  })
  ipcMain.handle(captureCommandChannels.refreshSurface, async (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    assertNoArguments(args)
    await captureSurfaceMonitor?.invalidate()
    await refreshApplicationTray()
    return captureSurfaceMonitor?.getSnapshot() ?? router.getSurfaceSnapshot()
  })
  ipcMain.handle(captureCommandChannels.recover, async (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    if (args.length !== 2) throw new Error('Expected a result ID and recovery action.')
    const { activeMode, lastCompletion } = captureSession.getSnapshot()
    if (activeMode || !lastCompletion || lastCompletion.id !== args[0]) return
    const action = captureRecoveryActions(lastCompletion.result, currentLumierePlatform()).find(
      ({ action }) => action === args[1],
    )?.action
    if (!action) throw new Error('Recovery action is unavailable for this result.')
    await recoverCapture(lastCompletion.id, lastCompletion.mode, action)
  })
  ipcMain.on(captureCommandChannels.regionOverlayHostReady, (event, ...args) => {
    if (args.length !== 0 || !regionOverlayController?.owns(event.sender)) return
    regionOverlayController.rendererBecameReady(event.sender)
  })

  ipcMain.on(captureCommandChannels.regionOverlayReady, (event, ...args) => {
    if (!regionOverlayController?.owns(event.sender) || args.length !== 1) return
    const generation = parseGeneration(args[0])
    const session = regionOverlaySession
    if (!session) return
    if (generation !== session.generation || session.ready) return
    session.ready = true
    reportSessionTiming(session, 'overlay-ready')
    if (regionOverlayController.show(generation)) reportSessionTiming(session, 'overlay-shown')
  })

  ipcMain.on(captureCommandChannels.cancelRegionOverlay, (event, ...args) => {
    if (!regionOverlayController?.owns(event.sender) || args.length !== 1) return
    const generation = parseGeneration(args[0])
    if (generation !== regionOverlaySession?.generation) return
    void cancelRegionOverlay(router)
  })

  ipcMain.on(captureCommandChannels.submitRegionSelection, (event, ...args) => {
    try {
      if (!regionOverlayController?.owns(event.sender) || args.length !== 2) return
      const generation = parseGeneration(args[0])
      if (generation !== regionOverlaySession?.generation) return
      const geometry = parseCaptureGeometry(args[1])
      submitRegionSelection(router, generation, geometry)
    } catch {
      return
    }
  })

  ipcMain.handle(settingsCommandChannels.getSnapshot, (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    assertNoArguments(args)
    return getSettingsSnapshot()
  })

  ipcMain.handle(settingsCommandChannels.chooseSaveDirectory, async (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    assertNoArguments(args)
    return chooseSaveDirectory()
  })

  ipcMain.handle(settingsCommandChannels.setOutputDelivery, async (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    if (args.length !== 1) {
      throw new Error('Expected one output-delivery argument.')
    }
    const delivery = parseOutputDelivery(args[0])
    const snapshot = await getSettingsSnapshot()
    if (!snapshot.availableOutputDeliveries.includes(delivery)) {
      throw new Error('The selected output destination is unavailable.')
    }

    if (!settingsStore) {
      throw new Error('Settings are not ready.')
    }
    await settingsStore.setOutputDelivery(delivery)
    const nextSnapshot = await getSettingsSnapshot()
    broadcastSettingsChanged(nextSnapshot)
    return nextSnapshot
  })

  ipcMain.handle(settingsCommandChannels.setAfterCaptureBehavior, async (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    if (args.length !== 1) {
      throw new Error('Expected one after-capture behavior argument.')
    }
    const behavior = parseAfterCaptureBehavior(args[0])
    if (!settingsStore) {
      throw new Error('Settings are not ready.')
    }
    await settingsStore.setAfterCaptureBehavior(behavior)
    const nextSnapshot = await getSettingsSnapshot()
    broadcastSettingsChanged(nextSnapshot)
    return nextSnapshot
  })

  ipcMain.handle(settingsCommandChannels.setHdrStatusReminders, async (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    if (args.length !== 1) {
      throw new Error('Expected one HDR status reminder argument.')
    }
    const enabled = parseHdrStatusReminders(args[0])
    if (!settingsStore) {
      throw new Error('Settings are not ready.')
    }
    await settingsStore.setHdrStatusReminders(enabled)
    const nextSnapshot = await getSettingsSnapshot()
    broadcastSettingsChanged(nextSnapshot)
    return nextSnapshot
  })

  ipcMain.handle(settingsCommandChannels.setCaptureShortcut, async (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    if (args.length !== 1) {
      throw new Error('Expected one shortcut update.')
    }
    const update = parseShortcutUpdate(args[0])
    if (!shortcutService) {
      throw new Error('Shortcut settings are not ready.')
    }
    try {
      await shortcutService.setShortcut(update.mode, update.accelerator)
    } catch (error) {
      return {
        status: 'failed' as const,
        message:
          error instanceof ShortcutRegistrationError
            ? error.message
            : 'The shortcut could not be saved. Try again.',
      }
    }
    const nextSnapshot = await getSettingsSnapshot()
    broadcastSettingsChanged(nextSnapshot)
    await refreshApplicationTray()
    return { status: 'success' as const, snapshot: nextSnapshot }
  })

  ipcMain.handle(settingsCommandChannels.setShortcutRecording, (event, ...args) => {
    assertTrustedWindow(event, mainWindow)
    if (args.length !== 1 || typeof args[0] !== 'boolean') {
      throw new Error('Expected one shortcut-recording state.')
    }
    shortcutService?.setRecording(args[0])
  })
}

async function captureRegion(router: CaptureCommandRouter): Promise<CaptureCommandResult> {
  const timingStartedAt = performance.now()
  let timingLastAt = timingStartedAt
  const generation = ++nextRegionOverlayGeneration
  const reportTiming = (stage: string, details: Record<string, number> = {}): void => {
    const now = performance.now()
    reportRegionCaptureTiming(stage, timingStartedAt, {
      generation,
      stageMilliseconds: Math.round(now - timingLastAt),
      ...details,
    })
    timingLastAt = now
  }
  reportTiming('command-received')
  const restoreMainWindow = mainWindow?.isVisible() === true && !mainWindow.isMinimized()
  const initialDisplay = screen.getDisplayNearestPoint(screen.getCursorScreenPoint())
  mainWindow?.hide()
  reportTiming('main-window-hidden')
  const overlay = regionOverlayController
  if (!overlay) {
    if (restoreMainWindow) showMainWindow()
    return captureFailedResult()
  }
  const overlayReadyPromise = overlay.ensureReady().then(
    () => true,
    () => false,
  )
  const target = await router.resolveRegionTarget()
  if (!target || !displayMatchesTarget(initialDisplay.bounds, target.logicalSize)) {
    if (restoreMainWindow) showMainWindow()
    return displayChangedResult()
  }
  const [preparation, overlayReady] = await Promise.all([
    router.beginRegionCapture(target.id, (stage) => {
      reportTiming(stage)
    }),
    overlayReadyPromise,
  ])
  if (preparation.status === 'failed') {
    if (restoreMainWindow) showMainWindow()
    return preparation.result
  }
  if (!overlayReady) {
    await router.cancelRegionCapture()
    if (restoreMainWindow) showMainWindow()
    return captureFailedResult()
  }
  reportTiming('overlay-controller-ready')

  const display = screen.getDisplayNearestPoint(screen.getCursorScreenPoint())
  if (
    display.id !== initialDisplay.id ||
    !displayMatchesTarget(display.bounds, preparation.targetSize)
  ) {
    await router.cancelRegionCapture()
    if (restoreMainWindow) showMainWindow()
    return displayChangedResult()
  }

  let preview: { token: string; url: string }
  try {
    preview = regionPreviewRegistry.grant(preparation.previewPath)
  } catch {
    await router.cancelRegionCapture()
    if (restoreMainWindow) showMainWindow()
    return captureFailedResult()
  }
  const result = new Promise<CaptureCommandResult>((resolve) => {
    const handleDisplayChange = (): void => {
      void failRegionOverlay(router, displayChangedResult())
    }
    screen.on('display-added', handleDisplayChange)
    screen.on('display-removed', handleDisplayChange)
    screen.on('display-metrics-changed', handleDisplayChange)
    const stopWatchingDisplays = (): void => {
      screen.removeListener('display-added', handleDisplayChange)
      screen.removeListener('display-removed', handleDisplayChange)
      screen.removeListener('display-metrics-changed', handleDisplayChange)
    }

    const displayWatcher = new RegionDisplayWatcher({
      readDisplayId: () => screen.getDisplayNearestPoint(screen.getCursorScreenPoint()).id,
      onDisplayChanged: (displayId) => {
        const session = regionOverlaySession
        if (!session || session.submitted) return
        session.desiredDisplayId = displayId
        queueRegionDisplaySwitch(router, session)
      },
    })
    regionOverlaySession = {
      generation,
      activeDisplayId: display.id,
      desiredDisplayId: display.id,
      targetSize: preparation.targetSize,
      previewToken: preview.token,
      previewUrl: preview.url,
      leaseTimeout: setTimeout(() => {
        void failRegionOverlay(router, {
          status: 'failed',
          feedback: 'Capture timed out',
          notice: {
            tone: 'caution',
            title: 'Capture timed out',
            detail: 'Start a new capture and select a region sooner.',
          },
        })
      }, preparation.leaseMilliseconds),
      displayWatcher,
      switchPromise: null,
      ready: false,
      submitted: false,
      resolve,
      stopWatchingDisplays,
      restoreMainWindow,
      timingStartedAt,
      timingLastAt,
    }
    displayWatcher.start(display.id)
  })
  try {
    await overlay.activate(
      {
        generation,
        targetSize: preparation.targetSize,
        previewPixelSize: preparation.previewPixelSize,
        previewUrl: preview.url,
      },
      display.bounds,
    )
    const session = regionOverlaySession
    if (session) {
      reportSessionTiming(session, 'overlay-activated', {
        targetWidth: Math.round(preparation.targetSize.width),
        targetHeight: Math.round(preparation.targetSize.height),
        previewWidth: Math.round(preparation.previewPixelSize.width),
        previewHeight: Math.round(preparation.previewPixelSize.height),
      })
    }
  } catch {
    await failRegionOverlay(router, captureFailedResult())
  }
  return result
}

function queueRegionDisplaySwitch(
  router: CaptureCommandRouter,
  session: RegionOverlaySession,
): void {
  if (session.switchPromise || !isActiveRegionSession(session)) return
  const switching = switchRegionDisplay(router, session)
  session.switchPromise = switching
  const finish = (): void => {
    if (session.switchPromise !== switching) return
    session.switchPromise = null
    if (isActiveRegionSession(session) && session.desiredDisplayId !== session.activeDisplayId) {
      queueRegionDisplaySwitch(router, session)
    }
  }
  void switching.then(finish, finish)
}

async function switchRegionDisplay(
  router: CaptureCommandRouter,
  session: RegionOverlaySession,
): Promise<void> {
  while (
    regionOverlaySession === session &&
    !session.submitted &&
    session.desiredDisplayId !== session.activeDisplayId
  ) {
    regionOverlayController?.reset(session.generation)
    clearTimeout(session.leaseTimeout)
    regionPreviewRegistry.revoke(session.previewToken)
    session.activeDisplayId = null
    await router.cancelRegionCapture()
    if (!isActiveRegionSession(session)) return

    const targetDisplayId = session.desiredDisplayId
    const resolution = await resolveRegionTargetWithRetry({
      expectedDisplayId: targetDisplayId,
      readDisplay: () => screen.getDisplayNearestPoint(screen.getCursorScreenPoint()),
      readTarget: () => router.resolveRegionTarget(),
      isActive: () => isActiveRegionSession(session),
      matchesTarget: (display, target) => displayMatchesTarget(display.bounds, target.logicalSize),
    })
    if (resolution.status === 'cancelled') return
    if (resolution.status === 'display-changed') {
      session.desiredDisplayId = resolution.displayId
      continue
    }
    if (resolution.status === 'unavailable') {
      session.submitted = true
      finishRegionOverlay(session, displayChangedResult())
      return
    }
    const preparation = await router.beginRegionCapture(resolution.target.id)
    if (!isActiveRegionSession(session)) {
      if (preparation.status === 'ready') await router.cancelRegionCapture()
      return
    }
    if (preparation.status === 'failed') {
      session.submitted = true
      finishRegionOverlay(session, preparation.result)
      return
    }

    const display = screen.getDisplayNearestPoint(screen.getCursorScreenPoint())
    if (
      display.id !== targetDisplayId ||
      !displayMatchesTarget(display.bounds, preparation.targetSize)
    ) {
      await router.cancelRegionCapture()
      session.desiredDisplayId = display.id
      continue
    }

    let preview: { token: string; url: string }
    try {
      preview = regionPreviewRegistry.grant(preparation.previewPath)
    } catch {
      await router.cancelRegionCapture()
      session.submitted = true
      finishRegionOverlay(session, captureFailedResult())
      return
    }

    session.generation = ++nextRegionOverlayGeneration
    session.activeDisplayId = display.id
    session.targetSize = preparation.targetSize
    session.previewToken = preview.token
    session.previewUrl = preview.url
    session.ready = false
    session.leaseTimeout = createRegionLeaseTimeout(router, preparation.leaseMilliseconds)
    try {
      await regionOverlayController?.activate(
        {
          generation: session.generation,
          targetSize: preparation.targetSize,
          previewPixelSize: preparation.previewPixelSize,
          previewUrl: preview.url,
        },
        display.bounds,
      )
    } catch {
      await router.cancelRegionCapture()
      session.submitted = true
      finishRegionOverlay(session, captureFailedResult())
      return
    }
  }
}

function isActiveRegionSession(session: RegionOverlaySession): boolean {
  return regionOverlaySession === session && !session.submitted
}

function createRegionLeaseTimeout(
  router: CaptureCommandRouter,
  leaseMilliseconds: number,
): NodeJS.Timeout {
  return setTimeout(() => {
    void failRegionOverlay(router, {
      status: 'failed',
      feedback: 'Capture timed out',
      notice: {
        tone: 'caution',
        title: 'Capture timed out',
        detail: 'Start a new capture and select a region sooner.',
      },
    })
  }, leaseMilliseconds)
}

function submitRegionSelection(
  router: CaptureCommandRouter,
  generation: number,
  geometry: CaptureGeometry,
): void {
  const session = regionOverlaySession
  if (
    !session ||
    session.submitted ||
    session.switchPromise ||
    session.generation !== generation ||
    session.activeDisplayId === null
  ) {
    return
  }
  if (!geometryFitsTarget(geometry, session.targetSize)) {
    void failRegionOverlay(router, displayChangedResult())
    return
  }

  session.submitted = true
  regionOverlayController?.reset(session.generation)
  void router.completeRegionCapture(geometry).then((result) => {
    finishRegionOverlay(session, result)
  })
}

async function cancelRegionOverlay(router: CaptureCommandRouter): Promise<void> {
  const session = regionOverlaySession
  if (!session || session.submitted) {
    return
  }
  session.submitted = true
  await session.switchPromise
  await router.cancelRegionCapture()
  finishRegionOverlay(session, { status: 'cancelled', feedback: 'Capture cancelled' })
}

async function failRegionOverlay(
  router: CaptureCommandRouter,
  result: CaptureCommandResult,
): Promise<void> {
  const session = regionOverlaySession
  if (!session || session.submitted) {
    return
  }
  session.submitted = true
  await session.switchPromise
  await router.cancelRegionCapture()
  finishRegionOverlay(session, result)
}

function finishRegionOverlay(session: RegionOverlaySession, result: CaptureCommandResult): void {
  if (regionOverlaySession !== session) {
    return
  }
  regionOverlaySession = null
  clearTimeout(session.leaseTimeout)
  regionPreviewRegistry.revoke(session.previewToken)
  session.displayWatcher.dispose()
  session.stopWatchingDisplays()
  regionOverlayController?.reset(session.generation)
  if (session.restoreMainWindow) showMainWindow()
  session.resolve(result)
}

function displayMatchesTarget(bounds: Electron.Rectangle, targetSize: LogicalSize): boolean {
  return (
    Math.abs(bounds.width - targetSize.width) <= 1 &&
    Math.abs(bounds.height - targetSize.height) <= 1
  )
}

function reportSessionTiming(
  session: RegionOverlaySession,
  stage: string,
  details: Record<string, number> = {},
): void {
  const now = performance.now()
  reportRegionCaptureTiming(stage, session.timingStartedAt, {
    generation: session.generation,
    stageMilliseconds: Math.round(now - session.timingLastAt),
    ...details,
  })
  session.timingLastAt = now
}

function parseGeneration(value: unknown): number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0 ? value : -1
}

function captureFailedResult(): CaptureCommandResult {
  return {
    status: 'failed',
    feedback: 'Capture failed',
    notice: {
      tone: 'critical',
      title: 'Capture failed',
      detail: 'Try again. Restart Lumiere if the issue continues.',
    },
  }
}

function geometryFitsTarget(geometry: CaptureGeometry, targetSize: LogicalSize): boolean {
  return (
    geometry.x + geometry.width <= targetSize.width + Number.EPSILON &&
    geometry.y + geometry.height <= targetSize.height + Number.EPSILON
  )
}

function displayChangedResult(): CaptureCommandResult {
  return {
    status: 'failed',
    feedback: 'Capture failed',
    notice: {
      tone: 'caution',
      title: 'Capture failed',
      detail: 'The display may have changed. Try again.',
    },
  }
}

function disposeRegionOverlay(router: CaptureCommandRouter): void {
  const session = regionOverlaySession
  if (!session) {
    return
  }
  regionOverlaySession = null
  clearTimeout(session.leaseTimeout)
  regionPreviewRegistry.revoke(session.previewToken)
  session.displayWatcher.dispose()
  void router.cancelRegionCapture()
  session.stopWatchingDisplays()
  regionOverlayController?.reset(session.generation)
}

async function getSettingsSnapshot(): Promise<SettingsSnapshot> {
  if (!platformHost || !settingsStore) {
    throw new Error('Settings are not ready.')
  }
  const capabilities = await platformHost.getCapabilities()
  return {
    outputDelivery: settingsStore.getOutputDelivery(),
    availableOutputDeliveries: availableOutputDeliveries(capabilities.deliveryTargets),
    saveDirectory: settingsStore.getSaveDirectory() ?? defaultCaptureDirectory(),
    afterCaptureBehavior: settingsStore.getAfterCaptureBehavior(),
    hdrStatusReminders: settingsStore.getHdrStatusReminders(),
    captureShortcuts: shortcutService?.getSnapshot() ?? {
      region: { accelerator: null, status: 'unconfigured' },
      display: { accelerator: null, status: 'unconfigured' },
    },
  }
}

function defaultCaptureDirectory(): string {
  return join(app.getPath('pictures'), 'Lumiere')
}

function broadcastSettingsChanged(snapshot: SettingsSnapshot): void {
  for (const window of [mainWindow]) {
    if (window && !window.isDestroyed()) {
      window.webContents.send(settingsCommandChannels.changed, snapshot)
    }
  }
  void captureSurfaceMonitor?.invalidate()
}

async function runCapture(mode: 'region' | 'display'): Promise<CaptureCommandResult> {
  const router = captureRouter
  if (!router || quitting) return captureFailedResult()
  return captureSession.run(
    mode,
    async () => {
      return completeCapture(
        mode === 'region' ? await captureRegion(router) : await router.captureDisplay(),
      )
    },
    (completion) => {
      if (quitting) return
      captureFailureNotifier.notify(completion, mainWindow?.isFocused() === true)
      void captureSurfaceMonitor?.invalidate()
      void refreshApplicationTray()
    },
  )
}

async function runExternalCapture(mode: 'region' | 'display'): Promise<void> {
  await runCapture(mode)
}

async function chooseSaveDirectory(): Promise<SettingsSnapshot> {
  if (!settingsStore) throw new Error('Settings are not ready.')
  const window = showMainWindow()
  const result = await dialog.showOpenDialog(window, {
    title: 'Choose save folder',
    defaultPath: settingsStore.getSaveDirectory() ?? defaultCaptureDirectory(),
    buttonLabel: 'Choose',
    properties: ['openDirectory', 'createDirectory'],
  })
  if (!result.canceled && result.filePaths.length === 1) {
    await settingsStore.setSaveDirectory(result.filePaths[0])
  }
  const snapshot = await getSettingsSnapshot()
  broadcastSettingsChanged(snapshot)
  return snapshot
}

async function recoverCapture(
  id: number,
  mode: 'region' | 'display',
  action: CaptureRecoveryAction,
): Promise<void> {
  switch (action) {
    case 'capture-again':
      await runCapture(mode)
      return
    case 'open-permissions':
      await shell.openExternal(
        'x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture',
      )
      return
    case 'choose-folder':
      captureFailureNotifier.clear()
      await chooseSaveDirectory()
      return
    case 'open-settings':
      captureFailureNotifier.clear()
      showSettingsWindow()
      return
    case 'refresh':
      captureFailureNotifier.clear()
      showMainWindow()
      await captureSurfaceMonitor?.invalidate()
      await captureSurfaceMonitor?.getSnapshot()
      captureSession.clearCompletion(id)
      await refreshApplicationTray()
  }
}

function completeCapture(result: CaptureCommandResult): CaptureCommandResult {
  return settingsStore
    ? applyAfterCaptureBehavior(result, settingsStore, (filePath) => {
        shell.showItemInFolder(filePath)
      })
    : result
}

function showCaptureWindow(): void {
  const window = showMainWindow()
  // Newly created windows start on Capture; an existing Settings view needs switching.
  if (!window.webContents.isLoadingMainFrame()) {
    window.webContents.send(captureCommandChannels.showRequested)
  }
}

function showSettingsWindow(): void {
  const window = showMainWindow()
  const send = (): void => {
    if (!window.isDestroyed()) {
      window.webContents.send(settingsCommandChannels.showRequested)
    }
  }
  if (window.webContents.isLoadingMainFrame()) {
    window.webContents.once('did-finish-load', send)
  } else {
    send()
  }
}

async function getApplicationTrayState() {
  const shortcuts = shortcutService?.getSnapshot() ?? {
    region: { accelerator: null, status: 'unconfigured' as const },
    display: { accelerator: null, status: 'unconfigured' as const },
  }
  try {
    const snapshot = await captureRouter?.getSurfaceSnapshot()
    return {
      regionAvailable: snapshot?.hostAvailable === true && snapshot.captureModes.includes('region'),
      displayAvailable:
        snapshot?.hostAvailable === true && snapshot.captureModes.includes('display'),
      shortcuts,
    }
  } catch {
    return { regionAvailable: false, displayAvailable: false, shortcuts }
  }
}

function updateApplicationTray(): void {
  if (!lastTrayState || quitting) return
  const busy = captureSession.getSnapshot().activeMode !== null
  applicationTray?.update({
    ...lastTrayState,
    regionAvailable: !busy && lastTrayState.regionAvailable,
    displayAvailable: !busy && lastTrayState.displayAvailable,
  })
}

async function refreshApplicationTray(): Promise<void> {
  lastTrayState = await getApplicationTrayState()
  updateApplicationTray()
}

function createPlatformHost(): PlatformHost {
  const platform = currentLumierePlatform()
  if (platform === 'macos') {
    return new MacOSPlatformHost(
      macOSHostCandidates({
        appPath: app.getAppPath(),
        isPackaged: app.isPackaged,
        resourcesPath: process.resourcesPath,
        overridePath: process.env.LUMIERE_MAC_HOST_PATH,
      }),
    )
  }

  return new WindowsPlatformHost(
    windowsHostCandidates({
      appPath: app.getAppPath(),
      isPackaged: app.isPackaged,
      resourcesPath: process.resourcesPath,
      overridePath: process.env.LUMIERE_WINDOWS_HOST_PATH,
    }),
  )
}

void app.whenReady().then(async () => {
  registerRegionPreviewProtocol()
  platformHost = createPlatformHost()
  settingsStore = new SettingsStore(join(app.getPath('userData'), 'settings.json'))
  await settingsStore.load()
  mainWindow = createRendererWindow()
  registerIpc()
  screen.on('display-added', refreshCaptureSurfaceForDisplayChange)
  screen.on('display-removed', refreshCaptureSurfaceForDisplayChange)
  screen.on('display-metrics-changed', refreshCaptureSurfaceForDisplayChange)
  if (!captureRouter) {
    throw new Error('Capture commands are not ready.')
  }
  shortcutService = new ShortcutService(settingsStore, globalShortcut, {
    region: () => runExternalCapture('region'),
    display: () => runExternalCapture('display'),
  })
  shortcutService.initialize()
  applyMacDockIcon()
  lastTrayState = await getApplicationTrayState()
  applicationTray = createApplicationTray(lastTrayState, {
    captureRegion: () => {
      void runExternalCapture('region')
    },
    captureDisplay: () => {
      void runExternalCapture('display')
    },
    showWindow: showMainWindow,
    showSettings: showSettingsWindow,
    quit: () => {
      app.quit()
    },
  })
  void configureWindowsUpdates()

  app.on('activate', () => {
    showMainWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    applicationTray?.destroy()
    app.quit()
  }
})

app.on('before-quit', () => {
  quitting = true
  captureFailureNotifier.clear()
  shortcutService?.dispose()
  screen.removeListener('display-added', refreshCaptureSurfaceForDisplayChange)
  screen.removeListener('display-removed', refreshCaptureSurfaceForDisplayChange)
  screen.removeListener('display-metrics-changed', refreshCaptureSurfaceForDisplayChange)
  captureSurfaceMonitor?.dispose()
  if (captureRouter) {
    disposeRegionOverlay(captureRouter)
  }
  regionOverlayController?.dispose()
  if (platformHost instanceof NativeProcessPlatformHost) {
    platformHost.dispose()
  }
  regionPreviewRegistry.clear()
})
