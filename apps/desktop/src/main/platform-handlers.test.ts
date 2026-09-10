import { describe, expect, it } from 'vitest'
import type {
  CaptureResult,
  DisplayCaptureRequest,
  PlatformCapabilities,
  PlatformHost,
} from '../shared/platform-contract'
import { PLATFORM_CONTRACT_VERSION } from '../shared/platform-contract'
import { createPlatformHandlers } from './platform-handlers'
import { UnavailablePlatformHost } from './platform-host'

describe('platform handlers', () => {
  it('rejects an unknown capture mode before crossing the platform seam', async () => {
    const host = new RecordingPlatformHost()
    const handlers = createPlatformHandlers(host)

    const result = await handlers.captureDisplay({ mode: 'window', delivery: 'folder' })

    expect(result).toMatchObject({
      status: 'failed',
      failure: { code: 'invalid-request', retryable: false },
    })
    expect(host.requests).toEqual([])
  })

  it('passes a valid request through the narrow host interface', async () => {
    const host = new RecordingPlatformHost()
    const handlers = createPlatformHandlers(host)

    await handlers.captureDisplay({
      delivery: 'both',
      saveDirectory: '/tmp/captures',
    })

    expect(host.requests).toEqual([
      {
        delivery: 'both',
        saveDirectory: '/tmp/captures',
      },
    ])
  })

  it('reports an unavailable native host without falling back to Electron capture', async () => {
    const handlers = createPlatformHandlers(new UnavailablePlatformHost('macos'))

    const capabilities = await handlers.getCapabilities()
    const result = await handlers.captureDisplay({ delivery: 'clipboard' })

    expect(capabilities).toMatchObject({
      contractVersion: 5,
      platform: 'macos',
      hostStatus: 'unavailable',
      hdrCapture: 'unavailable',
      outputProfiles: ['srgb-visual-match'],
    })
    expect(result).toMatchObject({
      status: 'failed',
      failure: { code: 'host-unavailable' },
    })
  })
})

class RecordingPlatformHost implements PlatformHost {
  public readonly requests: DisplayCaptureRequest[] = []

  public getCapabilities(): Promise<PlatformCapabilities> {
    return Promise.resolve({
      contractVersion: PLATFORM_CONTRACT_VERSION,
      platform: 'macos',
      hostStatus: 'available',
      captureModes: ['region', 'display'],
      deliveryTargets: ['clipboard', 'folder'],
      hdrCapture: 'supported',
      outputProfiles: ['srgb-visual-match'],
    })
  }

  public captureDisplay(request: DisplayCaptureRequest): Promise<CaptureResult> {
    this.requests.push(request)
    return Promise.resolve({
      status: 'completed',
      sourceDynamicRange: 'hdr',
      outputProfile: 'srgb-visual-match',
      deliveries:
        request.delivery === 'both'
          ? [
              { target: 'clipboard', status: 'success' },
              { target: 'folder', status: 'success', filePath: '/tmp/lumiere.png' },
            ]
          : request.delivery === 'clipboard'
            ? [{ target: 'clipboard', status: 'success' }]
            : [{ target: 'folder', status: 'success', filePath: '/tmp/lumiere.png' }],
    })
  }

  public prepareRegion(targetId: string): Promise<{
    status: 'failed'
    failure: { code: 'capture-unavailable'; message: string; retryable: true }
  }> {
    void targetId
    return Promise.resolve({
      status: 'failed',
      failure: { code: 'capture-unavailable', message: 'Unavailable', retryable: true },
    })
  }

  public commitRegion(): Promise<CaptureResult> {
    return Promise.resolve({ status: 'cancelled' })
  }

  public cancelRegion(): Promise<{ status: 'released' }> {
    return Promise.resolve({ status: 'released' })
  }
}
