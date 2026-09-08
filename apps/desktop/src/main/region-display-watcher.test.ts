import { afterEach, describe, expect, it, vi } from 'vitest'
import { RegionDisplayWatcher } from './region-display-watcher'

afterEach(() => {
  vi.useRealTimers()
})

describe('RegionDisplayWatcher', () => {
  it('reports when the pointer crosses from the active overlay display to another display', async () => {
    vi.useFakeTimers()
    let displayId = 1
    const onDisplayChanged = vi.fn()
    const watcher = new RegionDisplayWatcher({
      readDisplayId: () => displayId,
      onDisplayChanged,
      pollIntervalMilliseconds: 50,
    })

    watcher.start(1)
    await vi.advanceTimersByTimeAsync(50)
    expect(onDisplayChanged).not.toHaveBeenCalled()

    displayId = 2
    await vi.advanceTimersByTimeAsync(50)

    expect(onDisplayChanged).toHaveBeenCalledOnce()
    expect(onDisplayChanged).toHaveBeenCalledWith(2)
    watcher.dispose()
  })

  it('coalesces repeated polls after reporting the same destination display', async () => {
    vi.useFakeTimers()
    let displayId = 1
    const onDisplayChanged = vi.fn()
    const watcher = new RegionDisplayWatcher({
      readDisplayId: () => displayId,
      onDisplayChanged,
      pollIntervalMilliseconds: 50,
    })

    watcher.start(1)
    displayId = 2
    await vi.advanceTimersByTimeAsync(250)

    expect(onDisplayChanged).toHaveBeenCalledOnce()
    watcher.dispose()
  })

  it('reports each destination during a rapid display round trip', async () => {
    vi.useFakeTimers()
    let displayId = 1
    const onDisplayChanged = vi.fn()
    const watcher = new RegionDisplayWatcher({
      readDisplayId: () => displayId,
      onDisplayChanged,
      pollIntervalMilliseconds: 50,
    })

    watcher.start(1)
    displayId = 2
    await vi.advanceTimersByTimeAsync(50)
    displayId = 1
    await vi.advanceTimersByTimeAsync(50)

    expect(onDisplayChanged.mock.calls).toEqual([[2], [1]])
    watcher.dispose()
  })

  it('stops observing after the Region session ends', async () => {
    vi.useFakeTimers()
    let displayId = 1
    const onDisplayChanged = vi.fn()
    const watcher = new RegionDisplayWatcher({
      readDisplayId: () => displayId,
      onDisplayChanged,
      pollIntervalMilliseconds: 50,
    })

    watcher.start(1)
    watcher.dispose()
    displayId = 2
    await vi.advanceTimersByTimeAsync(100)

    expect(onDisplayChanged).not.toHaveBeenCalled()
  })
})
