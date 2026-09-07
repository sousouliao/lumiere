import { describe, expect, it, vi } from 'vitest'
import { CaptureSession } from './capture-session'
import type { CaptureCommandResult } from '../shared/capture-command'

describe('capture session', () => {
  it('publishes busy before preparation, ignores competing commands, and retains the result for reopening', async () => {
    const changed = vi.fn()
    const finished = vi.fn()
    const session = new CaptureSession(changed)
    let resolve!: (result: CaptureCommandResult) => void
    const pending = session.run(
      'region',
      () =>
        new Promise((done) => {
          resolve = done
        }),
      finished,
    )
    expect(session.getSnapshot()).toEqual({ activeMode: 'region', lastCompletion: null })
    const competing = vi.fn()
    await session.run('display', competing, finished)
    expect(competing).not.toHaveBeenCalled()
    expect(finished).not.toHaveBeenCalled()
    const result: CaptureCommandResult = { status: 'success', feedback: 'Copied to clipboard' }
    resolve(result)
    await pending
    expect(session.getSnapshot()).toEqual({
      activeMode: null,
      lastCompletion: { id: 1, mode: 'region', result },
    })
    expect(changed).toHaveBeenCalledTimes(2)
    expect(finished).toHaveBeenCalledTimes(1)
  })

  it('clears old feedback at start, releases busy after cancellation, and rejects stale recovery clearing', async () => {
    const session = new CaptureSession(() => undefined)
    await session.run(
      'display',
      () => Promise.resolve({ status: 'success', feedback: 'Copied' }),
      () => undefined,
    )
    await session.run(
      'region',
      () => {
        expect(session.getSnapshot()).toEqual({ activeMode: 'region', lastCompletion: null })
        return Promise.resolve({ status: 'cancelled', feedback: 'Capture cancelled' })
      },
      () => undefined,
    )
    session.clearCompletion(1)
    expect(session.getSnapshot().lastCompletion?.id).toBe(2)
    session.clearCompletion(2)
    expect(session.getSnapshot()).toEqual({ activeMode: null, lastCompletion: null })
  })

  it('recovers from a thrown orchestration failure and accepts another capture', async () => {
    const log = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    try {
      const session = new CaptureSession(() => undefined)
      await expect(
        session.run(
          'region',
          () => Promise.reject(new Error('overlay unavailable')),
          () => undefined,
        ),
      ).resolves.toMatchObject({ status: 'failed' })
      expect(session.getSnapshot().activeMode).toBeNull()
      await expect(
        session.run(
          'display',
          () => Promise.resolve({ status: 'success', feedback: 'Copied' }),
          () => undefined,
        ),
      ).resolves.toMatchObject({ status: 'success' })
    } finally {
      log.mockRestore()
    }
  })
})
