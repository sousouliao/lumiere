import { describe, expect, it } from 'vitest'
import type { CaptureCommandResult, CaptureSurfaceSnapshot } from '../../shared/capture-command'
import { resolveCaptureNotices } from './capture-notices'

const readySnapshot: CaptureSurfaceSnapshot = {
  platform: 'windows',
  hostAvailable: true,
  captureModes: ['region', 'display'],
  hdrStatus: 'ready',
  output: {
    delivery: 'both',
    label: 'Clipboard and folder',
    location: '~/Pictures/Lumiere',
  },
}

describe('capture notice placement', () => {
  it('keeps a non-blocking HDR reminder available as footer details', () => {
    const advisoryNotice = {
      tone: 'caution' as const,
      title: 'This display has not been verified',
      detail: 'The display under the pointer will use sRGB Visual Match.',
    }

    expect(
      resolveCaptureNotices({
        snapshot: { ...readySnapshot, advisoryNotice },
        result: null,
        loadFailed: false,
      }),
    ).toEqual({
      activeNotice: advisoryNotice,
      blockingNotice: undefined,
      detailNotice: advisoryNotice,
    })
  })

  it('replaces capture actions for a blocking surface failure', () => {
    const blockingNotice = {
      tone: 'critical' as const,
      title: 'Native capture host is unavailable',
      detail: 'Retry, or restart Lumiere if it does not come back.',
    }

    expect(
      resolveCaptureNotices({
        snapshot: { ...readySnapshot, hostAvailable: false, captureModes: [], blockingNotice },
        result: null,
        loadFailed: false,
      }),
    ).toEqual({ activeNotice: blockingNotice, blockingNotice, detailNotice: undefined })
  })

  it('places partial delivery recovery in details', () => {
    const partialResult: CaptureCommandResult = {
      status: 'partial',
      feedback: 'Copied, but not saved',
      notice: {
        tone: 'caution',
        title: 'Copied, but the file was not saved',
        detail: 'Choose a writable folder for your next capture.',
        recovery: 'folder',
      },
    }

    expect(
      resolveCaptureNotices({ snapshot: readySnapshot, result: partialResult, loadFailed: false }),
    ).toMatchObject({
      blockingNotice: undefined,
      detailNotice: partialResult.notice,
    })
  })
})
