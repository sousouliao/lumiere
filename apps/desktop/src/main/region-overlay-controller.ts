import { BrowserWindow, type Rectangle, type WebContents } from 'electron'
import { join } from 'node:path'
import { captureCommandChannels, type RegionOverlaySnapshot } from '../shared/capture-command'

interface RegionOverlayControllerOptions {
  preloadPath: string
  rendererDirectory: string
  rendererUrl?: string
  onSessionFailure(generation: number): void
  onTiming(stage: string, generation?: number): void
}

/** Owns the single reusable Region overlay renderer and its active generation. */
export class RegionOverlayController {
  private window: BrowserWindow | null = null
  private readyPromise: Promise<BrowserWindow> | null = null
  private resolveReady: ((window: BrowserWindow) => void) | null = null
  private rejectReady: ((error: Error) => void) | null = null
  private activeGeneration: number | null = null

  public constructor(private readonly options: RegionOverlayControllerOptions) {}

  public prewarm(): void {
    void this.ensureReady().catch(() => undefined)
  }

  public ensureReady(): Promise<BrowserWindow> {
    if (this.window && !this.window.isDestroyed() && this.resolveReady === null) {
      return Promise.resolve(this.window)
    }
    if (this.readyPromise) return this.readyPromise

    const window = this.createWindow()
    this.window = window
    this.readyPromise = new Promise<BrowserWindow>((resolve, reject) => {
      this.resolveReady = resolve
      this.rejectReady = reject
    })
    return this.readyPromise
  }

  public rendererBecameReady(sender: WebContents): void {
    const window = this.window
    if (!window || window.isDestroyed() || window.webContents.id !== sender.id) return
    this.options.onTiming('overlay-renderer-loaded')
    this.resolveReady?.(window)
    this.resolveReady = null
    this.rejectReady = null
  }

  public async activate(snapshot: RegionOverlaySnapshot, bounds: Rectangle): Promise<void> {
    const window = await this.ensureReady()
    this.activeGeneration = snapshot.generation
    window.setBounds(bounds, false)
    window.webContents.send(captureCommandChannels.regionOverlayActivated, snapshot)
  }

  public show(generation: number): boolean {
    const window = this.window
    if (this.activeGeneration !== generation || !window || window.isDestroyed()) return false
    window.show()
    window.focus()
    return true
  }

  public reset(generation: number): void {
    if (this.activeGeneration !== generation) return
    this.activeGeneration = null
    const window = this.window
    if (!window || window.isDestroyed()) return
    window.hide()
    window.webContents.send(captureCommandChannels.regionOverlayReset)
  }

  public owns(sender: WebContents): boolean {
    return this.window?.isDestroyed() === false && this.window.webContents.id === sender.id
  }

  public isCurrent(generation: number): boolean {
    return this.activeGeneration === generation
  }

  public dispose(): void {
    this.activeGeneration = null
    const window = this.window
    this.window = null
    this.failReadiness(new Error('Region overlay was disposed.'))
    if (window && !window.isDestroyed()) window.destroy()
  }

  private createWindow(): BrowserWindow {
    const window = new BrowserWindow({
      width: 1,
      height: 1,
      x: -10_000,
      y: -10_000,
      show: false,
      frame: false,
      roundedCorners: false,
      transparent: false,
      backgroundColor: '#000000',
      resizable: false,
      movable: false,
      minimizable: false,
      maximizable: false,
      fullscreenable: false,
      skipTaskbar: true,
      alwaysOnTop: true,
      hasShadow: false,
      enableLargerThanScreen: true,
      webPreferences: {
        preload: this.options.preloadPath,
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: true,
        backgroundThrottling: false,
      },
    })

    window.setContentProtection(true)
    if (process.platform === 'darwin') {
      window.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true })
      window.setAlwaysOnTop(true, 'screen-saver')
    }
    window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
    window.webContents.on('will-navigate', (event) => {
      event.preventDefault()
    })
    window.webContents.once('did-fail-load', () => {
      this.handleWindowFailure(window)
    })
    window.webContents.once('render-process-gone', () => {
      this.handleWindowFailure(window)
    })
    window.once('closed', () => {
      this.handleWindowFailure(window)
    })

    if (this.options.rendererUrl) {
      const rendererUrl = new URL(this.options.rendererUrl)
      rendererUrl.searchParams.set('surface', 'region-overlay')
      void window.loadURL(rendererUrl.toString())
    } else {
      void window.loadFile(join(this.options.rendererDirectory, 'index.html'), {
        query: { surface: 'region-overlay' },
      })
    }
    return window
  }

  private handleWindowFailure(window: BrowserWindow): void {
    if (this.window !== window) return
    const generation = this.activeGeneration
    this.activeGeneration = null
    this.window = null
    this.failReadiness(new Error('Region overlay renderer failed.'))
    if (!window.isDestroyed()) window.destroy()
    if (generation !== null) this.options.onSessionFailure(generation)
  }

  private failReadiness(error: Error): void {
    this.rejectReady?.(error)
    this.resolveReady = null
    this.rejectReady = null
    this.readyPromise = null
  }
}
