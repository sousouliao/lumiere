import { beforeEach, describe, expect, it, vi } from 'vitest'
import { captureCommandChannels } from '../shared/capture-command'

const electronState = vi.hoisted(() => {
  const windows: FakeWindow[] = []
  class FakeEmitter {
    private readonly listeners = new Map<string, (() => void)[]>()
    public once(event: string, listener: () => void): void {
      this.listeners.set(event, [...(this.listeners.get(event) ?? []), listener])
    }
    public on(event: string, listener: () => void): void {
      this.once(event, listener)
    }
    public emit(event: string): void {
      const listeners = this.listeners.get(event) ?? []
      this.listeners.delete(event)
      for (const listener of listeners) listener()
    }
  }
  class FakeWebContents extends FakeEmitter {
    public readonly id = windows.length + 1
    public readonly sent: unknown[][] = []
    public setWindowOpenHandler(): void {
      return undefined
    }
    public send(...args: unknown[]): void {
      this.sent.push(args)
    }
  }
  class FakeWindow extends FakeEmitter {
    public readonly webContents = new FakeWebContents()
    public readonly options: Record<string, unknown>
    public destroyed = false
    public visible = false
    public bounds: unknown
    public constructor(options: Record<string, unknown>) {
      super()
      this.options = options
      windows.push(this)
    }
    public setContentProtection(): void {
      return undefined
    }
    public setVisibleOnAllWorkspaces(): void {
      return undefined
    }
    public setAlwaysOnTop(): void {
      return undefined
    }
    public loadURL(): Promise<void> {
      return Promise.resolve()
    }
    public loadFile(): Promise<void> {
      return Promise.resolve()
    }
    public setBounds(bounds: unknown): void {
      this.bounds = bounds
    }
    public show(): void {
      this.visible = true
    }
    public focus(): void {
      return undefined
    }
    public hide(): void {
      this.visible = false
    }
    public isDestroyed(): boolean {
      return this.destroyed
    }
    public destroy(): void {
      this.destroyed = true
    }
  }
  return { windows, FakeWindow }
})

vi.mock('electron', () => ({ BrowserWindow: electronState.FakeWindow }))

import { RegionOverlayController } from './region-overlay-controller'

describe('RegionOverlayController', () => {
  beforeEach(() => electronState.windows.splice(0))

  it('creates the capture overlay without rounded window corners', () => {
    const controller = new RegionOverlayController({
      preloadPath: '/preload.js',
      rendererDirectory: '/renderer',
      onSessionFailure: () => undefined,
      onTiming: () => undefined,
    })

    void controller.ensureReady()

    expect(electronState.windows[0].options).toMatchObject({
      frame: false,
      roundedCorners: false,
    })
  })

  it('prewarms once, reuses the renderer, and scopes activation by generation', async () => {
    const failures: number[] = []
    const controller = new RegionOverlayController({
      preloadPath: '/preload.js',
      rendererDirectory: '/renderer',
      onSessionFailure: (generation) => failures.push(generation),
      onTiming: () => undefined,
    })

    const ready = controller.ensureReady()
    const window = electronState.windows[0]
    controller.rendererBecameReady(window.webContents as never)
    await ready
    await controller.ensureReady()
    expect(electronState.windows).toHaveLength(1)

    const snapshot = {
      generation: 7,
      targetSize: { width: 1512, height: 982 },
      previewPixelSize: { width: 1512, height: 982 },
      previewUrl: 'lumiere-region-preview://token',
    }
    await controller.activate(snapshot, { x: 0, y: 0, width: 1512, height: 982 })
    expect(window.webContents.sent.at(-1)).toEqual([
      captureCommandChannels.regionOverlayActivated,
      snapshot,
    ])
    expect(controller.show(6)).toBe(false)
    expect(controller.show(7)).toBe(true)

    controller.reset(6)
    expect(window.visible).toBe(true)
    controller.reset(7)
    expect(window.visible).toBe(false)
    expect(window.webContents.sent.at(-1)).toEqual([captureCommandChannels.regionOverlayReset])
    expect(failures).toEqual([])
  })

  it('fails the active generation once and rebuilds after a renderer crash', async () => {
    const failures: number[] = []
    const controller = new RegionOverlayController({
      preloadPath: '/preload.js',
      rendererDirectory: '/renderer',
      onSessionFailure: (generation) => failures.push(generation),
      onTiming: () => undefined,
    })
    const ready = controller.ensureReady()
    const first = electronState.windows[0]
    controller.rendererBecameReady(first.webContents as never)
    await ready
    await controller.activate(
      {
        generation: 8,
        targetSize: { width: 1, height: 1 },
        previewPixelSize: { width: 1, height: 1 },
        previewUrl: 'lumiere-region-preview://token',
      },
      { x: 0, y: 0, width: 1, height: 1 },
    )

    first.webContents.emit('render-process-gone')
    first.emit('closed')
    expect(failures).toEqual([8])

    void controller.ensureReady()
    expect(electronState.windows).toHaveLength(2)
  })
})
