import { describe, expect, it, vi } from 'vitest'
import {
  MainWindowPresentation,
  opensWindowOnTrayClick,
  type PresentableWindow,
} from './main-window-presentation'

class FakeWindow implements PresentableWindow {
  private readyListener: (() => void) | null = null
  private readonly closeListeners: ((event: { preventDefault(): void }) => void)[] = []
  public destroyed = false
  public minimized = false
  public readonly restore = vi.fn(() => {
    this.minimized = false
  })
  public readonly show = vi.fn()
  public readonly focus = vi.fn()
  public readonly hide = vi.fn()

  public once(event: 'ready-to-show', listener: () => void): void {
    this.readyListener = listener
  }

  public on(event: 'close', listener: (event: { preventDefault(): void }) => void): void {
    this.closeListeners.push(listener)
  }

  public isDestroyed(): boolean {
    return this.destroyed
  }

  public isMinimized(): boolean {
    return this.minimized
  }

  public emitReady(): void {
    const listener = this.readyListener
    this.readyListener = null
    listener?.()
  }

  public close(): ReturnType<typeof vi.fn> {
    const event = { preventDefault: vi.fn() }
    for (const listener of this.closeListeners) listener(event)
    return event.preventDefault
  }
}

describe('MainWindowPresentation', () => {
  it('keeps startup hidden and reveals a pending request only after the renderer is ready', () => {
    const window = new FakeWindow()
    const becameReady = vi.fn()
    const presentation = new MainWindowPresentation(window, {
      isQuitting: () => false,
      becameReady,
    })

    expect(window.show).not.toHaveBeenCalled()
    presentation.show()
    presentation.show()
    expect(window.show).not.toHaveBeenCalled()

    window.emitReady()

    expect(becameReady).toHaveBeenCalledOnce()
    expect(window.show).toHaveBeenCalledOnce()
    expect(window.focus).toHaveBeenCalledOnce()
  })

  it('restores a minimized ready window before showing and focusing it', () => {
    const window = new FakeWindow()
    const presentation = new MainWindowPresentation(window, {
      isQuitting: () => false,
      becameReady: vi.fn(),
    })
    window.emitReady()
    window.minimized = true

    presentation.show()

    expect(window.restore).toHaveBeenCalledOnce()
    expect(window.show).toHaveBeenCalledOnce()
    expect(window.focus).toHaveBeenCalledOnce()
  })

  it('turns close into hide while resident and allows close while quitting', () => {
    const window = new FakeWindow()
    let quitting = false
    new MainWindowPresentation(window, {
      isQuitting: () => quitting,
      becameReady: vi.fn(),
    })

    expect(window.close()).toHaveBeenCalledOnce()
    expect(window.hide).toHaveBeenCalledOnce()

    quitting = true
    expect(window.close()).not.toHaveBeenCalled()
    expect(window.hide).toHaveBeenCalledOnce()
  })

  it('opens directly from a tray click only on Windows', () => {
    expect(opensWindowOnTrayClick('darwin')).toBe(false)
    expect(opensWindowOnTrayClick('win32')).toBe(true)
  })
})
