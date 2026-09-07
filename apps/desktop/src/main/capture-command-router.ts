import type {
  CaptureCommandResult,
  CaptureNotice,
  CaptureSurfaceSnapshot,
  ProductHdrStatus,
} from '../shared/capture-command'
import type {
  CaptureMode,
  LumierePlatform,
  OutputDelivery,
  DeliveryResult,
  CaptureGeometry,
  LogicalSize,
  PixelSize,
  PlatformCapabilities,
  PlatformFailure,
  PlatformHost,
  PreparedRegionCapture,
} from '../shared/platform-contract'
import { deliveryTargetsFor } from '../shared/platform-contract'

export interface CapturePreferences {
  delivery: OutputDelivery
  saveDirectory?: string
  hdrStatusReminders: boolean
}

export interface CapturePreferencesReader {
  getCapturePreferences(): CapturePreferences
}

export type RegionCapturePreparation =
  | {
      status: 'ready'
      targetSize: LogicalSize
      previewPath: string
      previewPixelSize: PixelSize
      leaseMilliseconds: number
    }
  | { status: 'failed'; result: CaptureCommandResult }

export type RegionCaptureTimingReporter = (stage: 'native-prepared') => void

const defaultPreferences: CapturePreferencesReader = {
  getCapturePreferences: () => ({ delivery: 'both', hdrStatusReminders: true }),
}

export class CaptureCommandRouter {
  private captureInFlight = false
  private preparedRegion: PreparedRegionCapture | null = null

  public constructor(
    private readonly platform: LumierePlatform,
    private readonly host: PlatformHost,
    private readonly preferences: CapturePreferencesReader = defaultPreferences,
  ) {}

  public async getSurfaceSnapshot(): Promise<CaptureSurfaceSnapshot> {
    const { delivery, saveDirectory, hdrStatusReminders } = this.preferences.getCapturePreferences()
    return projectSurfaceSnapshot(
      this.platform,
      delivery,
      saveDirectory,
      hdrStatusReminders,
      await this.host.getCapabilities(),
    )
  }

  public async captureDisplay(): Promise<CaptureCommandResult> {
    if (this.captureInFlight) {
      return failedResult({
        tone: 'caution',
        title: 'A capture is already in progress',
        detail: 'Wait for it to finish, then try again.',
      })
    }

    this.captureInFlight = true
    try {
      const { delivery, saveDirectory } = this.preferences.getCapturePreferences()
      const capabilities = await this.host.getCapabilities()
      const unavailable = captureUnavailableNotice(capabilities, 'display', delivery)
      if (unavailable) {
        return failedResult(unavailable)
      }

      const result = await this.host.captureDisplay({
        delivery,
        ...(delivery !== 'clipboard' && saveDirectory ? { saveDirectory } : {}),
      })

      if (result.status === 'cancelled') {
        return { status: 'cancelled', feedback: 'Capture cancelled' }
      }
      if (result.status === 'failed') {
        return failedResult(noticeForFailure(result.failure))
      }

      return projectDeliveryResult(result.deliveries)
    } catch {
      return failedResult({
        tone: 'critical',
        title: 'Capture failed',
        detail: 'Try again. Restart Lumiere if the issue continues.',
      })
    } finally {
      this.captureInFlight = false
    }
  }

  public async beginRegionCapture(
    reportTiming?: RegionCaptureTimingReporter,
  ): Promise<RegionCapturePreparation> {
    if (this.captureInFlight) {
      return { status: 'failed', result: captureAlreadyInProgress() }
    }

    this.captureInFlight = true
    try {
      const prepared = await this.host.prepareRegion()
      reportTiming?.('native-prepared')
      if (prepared.status === 'failed') {
        this.captureInFlight = false
        return { status: 'failed', result: failedResult(noticeForFailure(prepared.failure)) }
      }
      this.preparedRegion = prepared
      return {
        status: 'ready',
        targetSize: prepared.targetLogicalSize,
        previewPath: prepared.preview.filePath,
        previewPixelSize: prepared.preview.pixelSize,
        leaseMilliseconds: prepared.leaseMilliseconds,
      }
    } catch {
      this.captureInFlight = false
      return { status: 'failed', result: captureFailed() }
    }
  }

  public async completeRegionCapture(geometry: CaptureGeometry): Promise<CaptureCommandResult> {
    const prepared = this.preparedRegion
    if (!this.captureInFlight || !prepared) {
      return captureFailed()
    }

    this.preparedRegion = null
    try {
      const { delivery, saveDirectory } = this.preferences.getCapturePreferences()
      const result = await this.host.commitRegion({
        sessionId: prepared.sessionId,
        delivery,
        ...(delivery !== 'clipboard' && saveDirectory ? { saveDirectory } : {}),
        geometry,
      })
      return projectCaptureResult(result)
    } catch {
      return captureFailed()
    } finally {
      this.captureInFlight = false
    }
  }

  public async cancelRegionCapture(): Promise<void> {
    const prepared = this.preparedRegion
    this.preparedRegion = null
    this.captureInFlight = false
    if (!prepared) {
      return
    }
    try {
      await this.host.cancelRegion(prepared.sessionId)
    } catch {
      // Host teardown still runs on process disposal; Overlay cancel must remain local.
    }
  }
}

function projectCaptureResult(
  result: Awaited<ReturnType<PlatformHost['captureDisplay']>>,
): CaptureCommandResult {
  if (result.status === 'cancelled') {
    return { status: 'cancelled', feedback: 'Capture cancelled' }
  }
  if (result.status === 'failed') {
    return failedResult(noticeForFailure(result.failure))
  }
  return projectDeliveryResult(result.deliveries)
}

function captureAlreadyInProgress(): CaptureCommandResult {
  return failedResult({
    tone: 'caution',
    title: 'A capture is already in progress',
    detail: 'Wait for it to finish, then try again.',
  })
}

function captureFailed(): CaptureCommandResult {
  return failedResult({
    tone: 'critical',
    title: 'Capture failed',
    detail: 'Try again. Restart Lumiere if the issue continues.',
  })
}

function projectSurfaceSnapshot(
  platform: LumierePlatform,
  delivery: OutputDelivery,
  saveDirectory: string | undefined,
  hdrStatusReminders: boolean,
  capabilities: PlatformCapabilities,
): CaptureSurfaceSnapshot {
  const hostAvailable = capabilities.hostStatus === 'available'
  const hdrStatus = projectHdrStatus(capabilities.hdrCapture)
  return {
    platform,
    hostAvailable,
    captureModes: capabilities.captureModes,
    hdrStatus,
    output: outputSummary(platform, delivery, saveDirectory),
    ...(!hostAvailable
      ? {
          blockingNotice: {
            tone: 'critical' as const,
            title: 'Native capture host is unavailable',
            detail: 'Retry, or restart Lumiere if it does not come back.',
          },
        }
      : {}),
    ...(hostAvailable &&
    capabilities.captureModes.length > 0 &&
    hdrStatusReminders &&
    hdrStatus !== 'ready'
      ? { advisoryNotice: hdrAdvisoryNotice(hdrStatus) }
      : {}),
  }
}

function hdrAdvisoryNotice(status: Exclude<ProductHdrStatus, 'ready'>): CaptureNotice {
  return status === 'unvalidated'
    ? {
        tone: 'caution',
        title: 'This display has not been verified',
        detail: 'The display under the pointer will use sRGB Visual Match.',
      }
    : {
        tone: 'caution',
        title: 'HDR-aware capture is unavailable for this display',
        detail: 'The display under the pointer will use sRGB Visual Match.',
      }
}

function projectHdrStatus(value: PlatformCapabilities['hdrCapture']): ProductHdrStatus {
  return value === 'supported' ? 'ready' : value
}

function outputSummary(
  platform: LumierePlatform,
  delivery: OutputDelivery,
  saveDirectory?: string,
): CaptureSurfaceSnapshot['output'] {
  if (delivery !== 'folder') {
    return {
      delivery,
      label: delivery === 'clipboard' ? 'Clipboard' : 'Clipboard and folder',
      location:
        delivery === 'clipboard' ? 'Ready for paste' : (saveDirectory ?? captureFolder(platform)),
    }
  }

  return {
    delivery,
    label: 'Folder',
    location: saveDirectory ?? captureFolder(platform),
  }
}

function captureFolder(platform: LumierePlatform): string {
  return platform === 'macos' ? '~/Pictures/Lumiere' : '%USERPROFILE%\\Pictures\\Lumiere'
}

function captureUnavailableNotice(
  capabilities: PlatformCapabilities,
  mode: CaptureMode,
  delivery: OutputDelivery,
): CaptureNotice | null {
  if (capabilities.hostStatus !== 'available') {
    return {
      tone: 'critical',
      title: 'Native capture host is unavailable',
      detail: 'Retry, or restart Lumiere if it does not come back.',
    }
  }
  if (!capabilities.captureModes.includes(mode)) {
    return {
      tone: 'caution',
      title:
        mode === 'region' ? 'Region capture is not available yet' : 'Display capture unavailable',
      detail: 'Choose an available capture action and try again.',
    }
  }
  if (
    !deliveryTargetsFor(delivery).every((target) => capabilities.deliveryTargets.includes(target))
  ) {
    return {
      tone: 'caution',
      title: 'Current output is unavailable',
      recovery: 'output',
      detail: 'Choose an available output destination and try again.',
    }
  }
  return null
}

function noticeForFailure(failure: PlatformFailure): CaptureNotice {
  switch (failure.code) {
    case 'permission-denied':
      return {
        tone: 'critical',
        title: 'Screen recording permission is required',
        recovery: 'permissions',
        detail: 'Allow screen recording in System Settings, then try again.',
      }
    case 'host-unavailable':
      return {
        tone: 'critical',
        title: 'Native capture host is unavailable',
        detail: 'Retry, or restart Lumiere if it does not come back.',
      }
    case 'capture-unavailable':
      return {
        tone: 'caution',
        title: 'Capture failed',
        detail: 'The display may have changed. Try again.',
      }
    case 'delivery-unavailable':
    case 'delivery-failed':
      return {
        tone: 'caution',
        title: 'Couldn’t deliver capture',
        recovery: 'output',
        detail: 'Check the output destination, then try again.',
      }
    case 'invalid-request':
    case 'unexpected-failure':
      return {
        tone: 'critical',
        title: 'Capture failed',
        detail: 'Try again. Restart Lumiere if the issue continues.',
      }
  }
}

function projectDeliveryResult(deliveries: readonly DeliveryResult[]): CaptureCommandResult {
  const clipboard = deliveries.find(({ target }) => target === 'clipboard')
  const folder = deliveries.find(({ target }) => target === 'folder')
  const clipboardSucceeded = clipboard?.status === 'success'
  const folderSucceeded = folder?.status === 'success'
  const filePath =
    folder?.target === 'folder' && folder.status === 'success' ? folder.filePath : undefined

  const folderName = filePath?.split(/[\\/]/).at(-2) ?? 'Lumiere'

  if (deliveries.every(({ status }) => status === 'success')) {
    const feedback =
      clipboardSucceeded && folderSucceeded
        ? `Copied and saved to “${folderName}”`
        : clipboardSucceeded
          ? 'Copied to clipboard'
          : `Saved to “${folderName}”`
    return { status: 'success', feedback, ...(filePath ? { filePath } : {}) }
  }

  if (deliveries.some(({ status }) => status === 'success')) {
    const feedback = clipboardSucceeded
      ? 'Copied to clipboard, but couldn’t save the file'
      : 'Saved the file, but couldn’t copy it'
    return {
      status: 'partial',
      feedback,
      notice: {
        tone: 'caution',
        title: feedback,
        detail: clipboardSucceeded
          ? 'Choose a writable folder, then take a new capture.'
          : 'Take a new capture to try copying again. Your saved file is unchanged.',
        recovery: clipboardSucceeded ? 'folder' : undefined,
      },
      ...(filePath ? { filePath } : {}),
    }
  }

  return failedResult({
    tone: 'caution',
    title: 'Couldn’t deliver capture',
    recovery: 'output',
    detail: 'Check the output destination, then try again.',
  })
}

function failedResult(notice: CaptureNotice): CaptureCommandResult {
  return {
    status: 'failed',
    feedback: notice.title,
    notice,
  }
}
