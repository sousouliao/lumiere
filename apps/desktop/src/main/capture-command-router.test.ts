import { describe, expect, it } from 'vitest'
import { CaptureCommandRouter } from './capture-command-router'
import type {
  CaptureResult,
  CommitRegionRequest,
  DisplayCaptureRequest,
  PlatformCapabilities,
  PlatformHost,
  PrepareRegionResult,
} from '../shared/platform-contract'
import { PLATFORM_CONTRACT_VERSION } from '../shared/platform-contract'

describe('CaptureCommandRouter', () => {
  it('projects the current macOS display and both-target default for the renderer', async () => {
    const host = new StubHost({
      contractVersion: PLATFORM_CONTRACT_VERSION,
      platform: 'macos',
      hostStatus: 'available',
      captureModes: ['display'],
      deliveryTargets: ['clipboard', 'folder'],
      hdrCapture: 'unavailable',
      outputProfiles: ['srgb-visual-match'],
    })

    await expect(new CaptureCommandRouter('macos', host).getSurfaceSnapshot()).resolves.toEqual({
      platform: 'macos',
      hostAvailable: true,
      captureModes: ['display'],
      hdrStatus: 'unavailable',
      advisoryNotice: {
        tone: 'caution',
        title: 'HDR-aware capture is unavailable for this display',
        detail: 'The display under the pointer will use sRGB Visual Match.',
      },
      output: {
        delivery: 'both',
        label: 'Clipboard and folder',
        location: '~/Pictures/Lumiere',
      },
    })
  })

  it('suppresses optional HDR status reminders without hiding the target status', async () => {
    const host = new StubHost({
      ...availableCapabilities(),
      hdrCapture: 'unvalidated',
    })

    const snapshot = await new CaptureCommandRouter(
      'macos',
      host,
      preferences('both', false),
    ).getSurfaceSnapshot()

    expect(snapshot).toMatchObject({
      hdrStatus: 'unvalidated',
    })
    expect(snapshot).not.toHaveProperty('advisoryNotice')
  })

  it('does not project an HDR advisory when the Host exposes no capture mode', async () => {
    const host = new StubHost({
      ...availableCapabilities(),
      captureModes: [],
      hdrCapture: 'unavailable',
    })

    const snapshot = await new CaptureCommandRouter('macos', host).getSurfaceSnapshot()

    expect(snapshot).not.toHaveProperty('advisoryNotice')
  })

  it('routes display capture to the clipboard-and-folder default', async () => {
    const host = new StubHost(availableCapabilities(), {
      status: 'completed',
      sourceDynamicRange: 'hdr',
      outputProfile: 'srgb-visual-match',
      deliveries: [
        { target: 'clipboard', status: 'success' },
        { target: 'folder', status: 'success', filePath: '/tmp/lumiere.png' },
      ],
    })

    await expect(new CaptureCommandRouter('macos', host).captureDisplay()).resolves.toEqual({
      status: 'success',
      feedback: 'Copied and saved to “tmp”',
      filePath: '/tmp/lumiere.png',
    })
    expect(host.requests).toEqual([{ operation: 'display', delivery: 'both' }])
  })

  it('routes a custom save directory only to file-capable capture requests', async () => {
    const host = new StubHost(availableCapabilities(), {
      status: 'completed',
      sourceDynamicRange: 'sdr',
      outputProfile: 'srgb-visual-match',
      deliveries: [{ target: 'folder', status: 'success', filePath: '/tmp/custom/lumiere.png' }],
    })
    const router = new CaptureCommandRouter('macos', host, {
      getCapturePreferences: () => ({
        delivery: 'folder',
        saveDirectory: '/tmp/custom',
        hdrStatusReminders: true,
      }),
    })

    await expect(router.captureDisplay()).resolves.toMatchObject({ feedback: 'Saved to “custom”' })

    expect(host.requests).toEqual([
      { operation: 'display', delivery: 'folder', saveDirectory: '/tmp/custom' },
    ])
    await expect(router.getSurfaceSnapshot()).resolves.toMatchObject({
      output: { location: '/tmp/custom' },
    })
  })

  it('prepares and completes a target-local region capture', async () => {
    const host = new StubHost(
      {
        ...availableCapabilities(),
        captureModes: ['region', 'display'],
      },
      {
        status: 'completed',
        sourceDynamicRange: 'hdr',
        outputProfile: 'srgb-visual-match',
        deliveries: [
          { target: 'clipboard', status: 'success' },
          { target: 'folder', status: 'success', filePath: '/tmp/region.png' },
        ],
      },
    )
    const router = new CaptureCommandRouter('macos', host)
    const timingStages: string[] = []

    await expect(router.beginRegionCapture((stage) => timingStages.push(stage))).resolves.toEqual({
      status: 'ready',
      targetSize: { width: 1512, height: 982 },
      previewPath: '/tmp/frozen-region.png',
      previewPixelSize: { width: 1512, height: 982 },
      leaseMilliseconds: 60_000,
    })
    expect(timingStages).toEqual(['native-prepared'])
    expect(host.capabilitiesRequests).toBe(0)
    await expect(
      router.completeRegionCapture({
        coordinateSpace: 'target-logical',
        x: 12.5,
        y: 20,
        width: 640,
        height: 360,
      }),
    ).resolves.toEqual({
      status: 'success',
      feedback: 'Copied and saved to “tmp”',
      filePath: '/tmp/region.png',
    })
    expect(host.requests).toEqual([
      {
        operation: 'commit-region',
        delivery: 'both',
        sessionId: 'region-session-17',
        geometry: {
          coordinateSpace: 'target-logical',
          x: 12.5,
          y: 20,
          width: 640,
          height: 360,
        },
      },
    ])
  })

  it('cancels a region before dispatch without calling the native host', async () => {
    const host = new StubHost({
      ...availableCapabilities(),
      captureModes: ['region', 'display'],
    })
    const router = new CaptureCommandRouter('macos', host)

    await expect(router.beginRegionCapture()).resolves.toMatchObject({ status: 'ready' })
    await router.cancelRegionCapture()
    await expect(router.captureDisplay()).resolves.toMatchObject({ status: 'cancelled' })
    expect(host.requests).toEqual([
      { operation: 'cancel-region', sessionId: 'region-session-17' },
      { operation: 'display', delivery: 'both' },
    ])
  })

  it('does not open a region overlay without an active target', async () => {
    const host = new StubHost(
      { ...availableCapabilities(), captureModes: ['region', 'display'] },
      { status: 'cancelled' },
      {
        status: 'failed',
        failure: {
          code: 'capture-unavailable',
          message: 'No target is available.',
          retryable: true,
        },
      },
    )

    await expect(
      new CaptureCommandRouter('macos', host).beginRegionCapture(),
    ).resolves.toMatchObject({
      status: 'failed',
      result: { status: 'failed', feedback: 'Capture failed' },
    })
  })

  it('maps native failures to product copy instead of exposing host diagnostics', async () => {
    const host = new StubHost(availableCapabilities(), {
      status: 'failed',
      failure: {
        code: 'permission-denied',
        message: 'CGPreflightScreenCaptureAccess returned false for pid 4812.',
        retryable: false,
      },
    })

    const result = await new CaptureCommandRouter('macos', host).captureDisplay()

    expect(result).toMatchObject({
      status: 'failed',
      feedback: 'Screen recording permission is required',
      notice: {
        detail: 'Allow screen recording in System Settings, then try again.',
      },
    })
    expect(JSON.stringify(result)).not.toContain('CGPreflightScreenCaptureAccess')
  })

  it('does not start a second native capture while one is in progress', async () => {
    let finishCapture: ((result: CaptureResult) => void) | undefined
    const capturePending = new Promise<CaptureResult>((resolve) => {
      finishCapture = resolve
    })
    const host = new StubHost(availableCapabilities(), capturePending)
    const router = new CaptureCommandRouter('macos', host)

    const firstCapture = router.captureDisplay()
    await Promise.resolve()
    const secondCapture = await router.captureDisplay()

    expect(secondCapture).toMatchObject({
      status: 'failed',
      feedback: 'A capture is already in progress',
    })
    expect(host.requests).toHaveLength(1)

    finishCapture?.({
      status: 'completed',
      sourceDynamicRange: 'sdr',
      outputProfile: 'srgb-visual-match',
      deliveries: [
        { target: 'clipboard', status: 'success' },
        { target: 'folder', status: 'success', filePath: '/tmp/first.png' },
      ],
    })
    await firstCapture
  })

  it('preserves a successful target when the second delivery fails', async () => {
    const host = new StubHost(
      { ...availableCapabilities(), deliveryTargets: ['clipboard', 'folder'] },
      {
        status: 'completed',
        sourceDynamicRange: 'hdr',
        outputProfile: 'srgb-visual-match',
        deliveries: [
          { target: 'clipboard', status: 'success' },
          {
            target: 'folder',
            status: 'failed',
            failure: {
              code: 'delivery-failed',
              message: 'The configured folder is not writable.',
              retryable: true,
            },
          },
        ],
      },
    )

    await expect(
      new CaptureCommandRouter('macos', host, preferences('both')).captureDisplay(),
    ).resolves.toMatchObject({
      status: 'partial',
      feedback: 'Copied to clipboard, but couldn’t save the file',
    })
  })

  it('projects two target failures as delivery failure without losing either host fact', async () => {
    const failed = (target: 'clipboard' | 'folder') => ({
      target,
      status: 'failed' as const,
      failure: {
        code: 'delivery-failed' as const,
        message: `${target} failed`,
        retryable: true,
      },
    })
    const host = new StubHost(availableCapabilities(), {
      status: 'completed',
      sourceDynamicRange: 'sdr',
      outputProfile: 'srgb-visual-match',
      deliveries: [failed('clipboard'), failed('folder')],
    })

    await expect(new CaptureCommandRouter('macos', host).captureDisplay()).resolves.toMatchObject({
      status: 'failed',
      feedback: 'Couldn’t deliver capture',
    })
  })

  it('reads the latest output preference for the next capture', async () => {
    const host = new StubHost(availableCapabilities(), {
      status: 'completed',
      sourceDynamicRange: 'sdr',
      outputProfile: 'srgb-visual-match',
      deliveries: [{ target: 'clipboard', status: 'success' }],
    })
    let delivery: 'clipboard' | 'folder' | 'both' = 'both'
    const router = new CaptureCommandRouter('macos', host, {
      getCapturePreferences: () => ({ delivery, hdrStatusReminders: true }),
    })

    delivery = 'clipboard'
    await expect(router.getSurfaceSnapshot()).resolves.toMatchObject({
      output: { delivery: 'clipboard', label: 'Clipboard' },
    })
    await router.captureDisplay()

    expect(host.requests).toEqual([{ operation: 'display', delivery: 'clipboard' }])
  })
})

class StubHost implements PlatformHost {
  public capabilitiesRequests = 0
  public readonly requests: (
    | ({ operation: 'display' } & DisplayCaptureRequest)
    | ({ operation: 'commit-region' } & CommitRegionRequest)
    | { operation: 'cancel-region'; sessionId: string }
  )[] = []

  public constructor(
    private readonly capabilities: PlatformCapabilities,
    private readonly result: CaptureResult | Promise<CaptureResult> = {
      status: 'cancelled',
    },
    private readonly prepared: PrepareRegionResult = {
      status: 'prepared',
      sessionId: 'region-session-17',
      targetLogicalSize: { width: 1512, height: 982 },
      preview: {
        filePath: '/tmp/frozen-region.png',
        mediaType: 'image/png',
        pixelSize: { width: 1512, height: 982 },
      },
      leaseMilliseconds: 60_000,
    },
  ) {}

  public getCapabilities(): Promise<PlatformCapabilities> {
    this.capabilitiesRequests += 1
    return Promise.resolve(this.capabilities)
  }

  public captureDisplay(request: DisplayCaptureRequest): Promise<CaptureResult> {
    this.requests.push({ operation: 'display', ...request })
    return Promise.resolve(this.result)
  }

  public prepareRegion(): Promise<PrepareRegionResult> {
    return Promise.resolve(this.prepared)
  }

  public commitRegion(request: CommitRegionRequest): Promise<CaptureResult> {
    this.requests.push({ operation: 'commit-region', ...request })
    return Promise.resolve(this.result)
  }

  public cancelRegion(sessionId: string): Promise<{ status: 'released' }> {
    this.requests.push({ operation: 'cancel-region', sessionId })
    return Promise.resolve({ status: 'released' })
  }
}

function availableCapabilities(): PlatformCapabilities {
  return {
    contractVersion: PLATFORM_CONTRACT_VERSION,
    platform: 'macos',
    hostStatus: 'available',
    captureModes: ['display'],
    deliveryTargets: ['clipboard', 'folder'],
    hdrCapture: 'supported',
    outputProfiles: ['srgb-visual-match'],
  }
}

function preferences(delivery: 'clipboard' | 'folder' | 'both', hdrStatusReminders = true) {
  return { getCapturePreferences: () => ({ delivery, hdrStatusReminders }) }
}
