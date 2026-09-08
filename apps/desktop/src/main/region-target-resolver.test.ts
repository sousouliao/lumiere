import { describe, expect, it, vi } from 'vitest'
import { resolveRegionTargetWithRetry } from './region-target-resolver'

const display = { id: 2, bounds: { x: 0, y: 0, width: 1512, height: 982 } }
const target = { id: 'target-2', logicalSize: { width: 1512, height: 982 } }

describe('resolveRegionTargetWithRetry', () => {
  it('backs off and stops after a bounded number of unavailable target responses', async () => {
    const readTarget = vi.fn(() => Promise.resolve(null))
    const wait = vi.fn(() => Promise.resolve())

    await expect(
      resolveRegionTargetWithRetry({
        expectedDisplayId: 2,
        readDisplay: () => display,
        readTarget,
        isActive: () => true,
        matchesTarget: () => false,
        maxAttempts: 3,
        retryDelayMilliseconds: 50,
        wait,
      }),
    ).resolves.toEqual({ status: 'unavailable' })

    expect(readTarget).toHaveBeenCalledTimes(3)
    expect(wait.mock.calls).toEqual([[50], [50]])
  })

  it('returns the target when a retry becomes stable', async () => {
    const readTarget = vi.fn().mockResolvedValueOnce(null).mockResolvedValueOnce(target)

    await expect(
      resolveRegionTargetWithRetry({
        expectedDisplayId: 2,
        readDisplay: () => display,
        readTarget,
        isActive: () => true,
        matchesTarget: (currentDisplay, currentTarget) =>
          currentDisplay.bounds.width === currentTarget.logicalSize.width,
        wait: () => Promise.resolve(),
      }),
    ).resolves.toEqual({ status: 'ready', display, target })
  })

  it('redirects immediately when the pointer moves before target resolution', async () => {
    const readTarget = vi.fn(() => Promise.resolve(target))

    await expect(
      resolveRegionTargetWithRetry({
        expectedDisplayId: 2,
        readDisplay: () => ({ ...display, id: 3 }),
        readTarget,
        isActive: () => true,
        matchesTarget: () => true,
      }),
    ).resolves.toEqual({ status: 'display-changed', displayId: 3 })

    expect(readTarget).not.toHaveBeenCalled()
  })
})
