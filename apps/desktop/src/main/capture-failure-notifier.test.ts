import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { CaptureCompletion } from '../shared/capture-command'

const fake = vi.hoisted(() => {
  class Notification {
    public static all: Notification[] = []
    public static isSupported = vi.fn(() => true)
    public static failOnShow = false
    public callbacks = new Map<string, () => void>()
    public show = vi.fn(() => {
      if (Notification.failOnShow) throw new Error('notifications disabled')
    })
    public close = vi.fn()
    public once(event: string, callback: () => void): void {
      this.callbacks.set(event, callback)
    }
    public constructor(public readonly options: { title: string; body: string; silent: boolean }) {
      Notification.all.push(this)
    }
  }
  return { Notification }
})
vi.mock('electron', () => ({ Notification: fake.Notification }))
import { CaptureFailureNotifier } from './capture-failure-notifier'

const failure: CaptureCompletion = {
  id: 1,
  mode: 'region',
  result: {
    status: 'failed',
    feedback: 'Capture failed',
    notice: { tone: 'critical', title: 'Capture failed', detail: 'Try again.' },
  },
}

describe('background capture failure notifications', () => {
  beforeEach(() => {
    fake.Notification.all = []
    fake.Notification.isSupported.mockReturnValue(true)
    fake.Notification.failOnShow = false
  })

  it('keeps success, cancellation, and foreground errors silent', () => {
    const notifier = new CaptureFailureNotifier(vi.fn())
    notifier.notify({ ...failure, result: { status: 'success', feedback: 'Copied' } }, false)
    notifier.notify({ ...failure, result: { status: 'cancelled', feedback: 'Cancelled' } }, false)
    notifier.notify(failure, true)
    notifier.notify(
      {
        ...failure,
        result: {
          status: 'partial',
          feedback: 'Copied, not saved',
          notice: { tone: 'caution', title: 'Save failed', detail: 'Choose a folder.' },
        },
      },
      true,
    )
    expect(fake.Notification.all).toHaveLength(0)
  })

  it('announces background partial success once, silently, and opens its result on click', () => {
    const open = vi.fn()
    const notifier = new CaptureFailureNotifier(open)
    const partial: CaptureCompletion = {
      ...failure,
      result: {
        status: 'partial',
        feedback: 'Copied to clipboard, but couldn’t save the file',
        notice: { tone: 'caution', title: 'Save failed', detail: 'Choose a folder.' },
      },
    }
    notifier.notify(partial, false)
    notifier.notify(partial, false)
    const notification = fake.Notification.all[0]
    expect(fake.Notification.all).toHaveLength(1)
    expect(notification.options.silent).toBe(true)
    expect(notification.options.body).toContain('Copied to clipboard, but couldn’t save the file')
    notification.callbacks.get('click')?.()
    expect(open).toHaveBeenCalledExactlyOnceWith(1)
    expect(notification.close).toHaveBeenCalledOnce()
  })

  it('clears old notifications before another operation and rejects their late clicks', () => {
    const open = vi.fn()
    const notifier = new CaptureFailureNotifier(open)
    notifier.notify(failure, false)
    const old = fake.Notification.all[0]
    notifier.clear()
    notifier.notify({ ...failure, id: 2 }, false)
    old.callbacks.get('click')?.()
    expect(open).not.toHaveBeenCalled()
    expect(old.close).toHaveBeenCalledOnce()
    fake.Notification.all[1].callbacks.get('click')?.()
    expect(open).toHaveBeenCalledExactlyOnceWith(2)
  })

  it('preserves capture truth when system notifications are unavailable', () => {
    const notifier = new CaptureFailureNotifier(vi.fn())
    fake.Notification.isSupported.mockReturnValue(false)
    notifier.notify(failure, false)
    expect(fake.Notification.all).toHaveLength(0)
    fake.Notification.isSupported.mockReturnValue(true)
    fake.Notification.failOnShow = true
    const log = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    try {
      expect(() => {
        notifier.notify({ ...failure, id: 2 }, false)
      }).not.toThrow()
      expect(failure.result.status).toBe('failed')
    } finally {
      log.mockRestore()
    }
  })
})
