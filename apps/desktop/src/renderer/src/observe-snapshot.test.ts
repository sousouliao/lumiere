import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { observeSnapshot } from './observe-snapshot'
import { windowsApi } from './tauri-bridge'

const { invoke, listen } = vi.hoisted(() => ({
  invoke: vi.fn<() => Promise<unknown>>(),
  listen:
    vi.fn<
      (event: string, callback: (event: { payload: unknown }) => void) => Promise<() => void>
    >(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: Error) => void
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept
    reject = fail
  })
  return { promise, resolve, reject }
}

const channels = [
  [
    () => windowsApi.getSettingsSnapshot(),
    (listener: (value: unknown) => void) => windowsApi.onSettingsChanged(listener),
  ],
  [
    () => windowsApi.getUpdateSnapshot(),
    (listener: (value: unknown) => void) => windowsApi.onUpdateChanged(listener),
  ],
  [
    () => windowsApi.getCaptureActivity(),
    (listener: (value: unknown) => void) => windowsApi.onCaptureActivityChanged(listener),
  ],
  [
    () => windowsApi.getCaptureSurfaceSnapshot(),
    (listener: (value: unknown) => void) => windowsApi.onCaptureSurfaceChanged(listener),
  ],
] as const

describe('snapshot observation through the Tauri adapter', () => {
  beforeEach(() => vi.resetAllMocks())
  afterEach(() => vi.restoreAllMocks())

  it.each(channels)('applies events ahead of late initial snapshots', async (read, subscribe) => {
    const registration = deferred<() => void>()
    const initial = deferred<unknown>()
    const stopNative = vi.fn()
    listen.mockReturnValue(registration.promise)
    invoke.mockReturnValue(initial.promise)
    const onSnapshot = vi.fn()
    const stop = observeSnapshot<unknown>({ read, subscribe, onSnapshot })
    expect(invoke).not.toHaveBeenCalled()
    registration.resolve(stopNative)
    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledOnce()
    })
    listen.mock.calls[0]?.[1]({ payload: { state: 'new' } })
    initial.resolve({ state: 'old' })
    await initial.promise
    await Promise.resolve()
    expect(onSnapshot.mock.calls).toEqual([[{ state: 'new' }, 'event']])
    stop()
    expect(stopNative).toHaveBeenCalledOnce()
  })

  it('applies an initial snapshot after registration and continues listening', async () => {
    listen.mockResolvedValue(vi.fn())
    invoke.mockResolvedValue({ state: 'initial' })
    const onSnapshot = vi.fn()
    const stop = observeSnapshot({
      read: () => windowsApi.getSettingsSnapshot(),
      subscribe: (listener) => windowsApi.onSettingsChanged(listener),
      onSnapshot,
    })
    await vi.waitFor(() => {
      expect(onSnapshot).toHaveBeenCalledWith({ state: 'initial' }, 'initial')
    })
    listen.mock.calls[0]?.[1]({ payload: { state: 'changed' } })
    expect(onSnapshot).toHaveBeenLastCalledWith({ state: 'changed' }, 'event')
    stop()
  })

  it('releases a pending registration after unmount without reading or applying events', async () => {
    const registration = deferred<() => void>()
    const stopNative = vi.fn()
    listen.mockReturnValue(registration.promise)
    const onSnapshot = vi.fn()
    const stop = observeSnapshot({
      read: () => windowsApi.getSettingsSnapshot(),
      subscribe: (listener) => windowsApi.onSettingsChanged(listener),
      onSnapshot,
    })
    stop()
    registration.resolve(stopNative)
    await vi.waitFor(() => {
      expect(stopNative).toHaveBeenCalledOnce()
    })
    listen.mock.calls[0]?.[1]({ payload: { state: 'late' } })
    expect(invoke).not.toHaveBeenCalled()
    expect(onSnapshot).not.toHaveBeenCalled()
  })

  it('ignores a read failure after a live snapshot', async () => {
    const initial = deferred<unknown>()
    listen.mockResolvedValue(vi.fn())
    invoke.mockReturnValue(initial.promise)
    const onError = vi.fn()
    const stop = observeSnapshot({
      read: () => windowsApi.getSettingsSnapshot(),
      subscribe: (listener) => windowsApi.onSettingsChanged(listener),
      onSnapshot: vi.fn(),
      onError,
    })
    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledOnce()
    })
    listen.mock.calls[0]?.[1]({ payload: { state: 'new' } })
    initial.reject(new Error('stale read failed'))
    await initial.promise.catch(() => undefined)
    await Promise.resolve()
    expect(onError).not.toHaveBeenCalled()
    stop()
  })

  it('ignores an in-flight snapshot after unmount', async () => {
    const initial = deferred<unknown>()
    const stopNative = vi.fn()
    listen.mockResolvedValue(stopNative)
    invoke.mockReturnValue(initial.promise)
    const onSnapshot = vi.fn()
    const stop = observeSnapshot({
      read: () => windowsApi.getSettingsSnapshot(),
      subscribe: (listener) => windowsApi.onSettingsChanged(listener),
      onSnapshot,
    })
    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledOnce()
    })
    stop()
    initial.resolve({ state: 'late' })
    await initial.promise
    await Promise.resolve()
    expect(onSnapshot).not.toHaveBeenCalled()
    expect(stopNative).toHaveBeenCalledOnce()
  })

  it('reports a subscription failure without issuing a snapshot request', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const error = new Error('registration failed')
    listen.mockRejectedValue(error)
    const onError = vi.fn()
    observeSnapshot({
      read: () => windowsApi.getSettingsSnapshot(),
      subscribe: (listener) => windowsApi.onSettingsChanged(listener),
      onSnapshot: vi.fn(),
      onError,
    })
    await vi.waitFor(() => {
      expect(onError).toHaveBeenCalledWith(error)
    })
    expect(invoke).not.toHaveBeenCalled()
  })

  it('reports an initial read failure and still accepts later events', async () => {
    const error = new Error('read failed')
    listen.mockResolvedValue(vi.fn())
    invoke.mockRejectedValue(error)
    const onSnapshot = vi.fn()
    const onError = vi.fn()
    const stop = observeSnapshot({
      read: () => windowsApi.getSettingsSnapshot(),
      subscribe: (listener) => windowsApi.onSettingsChanged(listener),
      onSnapshot,
      onError,
    })
    await vi.waitFor(() => {
      expect(onError).toHaveBeenCalledWith(error)
    })
    listen.mock.calls[0]?.[1]({ payload: { state: 'recovered' } })
    expect(onSnapshot).toHaveBeenCalledWith({ state: 'recovered' }, 'event')
    stop()
  })
})
