export const PLATFORM_CONTRACT_VERSION = 5 as const

export type LumierePlatform = 'macos' | 'windows'
export type CaptureMode = 'region' | 'display'
export type OutputDelivery = 'clipboard' | 'folder' | 'both'
export type DeliveryTarget = 'clipboard' | 'folder'

export interface PlatformCapabilities {
  contractVersion: typeof PLATFORM_CONTRACT_VERSION
  platform: LumierePlatform
  hostStatus: 'available' | 'unavailable'
  captureModes: readonly CaptureMode[]
  deliveryTargets: readonly DeliveryTarget[]
  hdrCapture: 'supported' | 'unavailable' | 'unvalidated'
  outputProfiles: readonly ['srgb-visual-match']
  activeTarget?: CaptureTarget
  unavailableReason?: PlatformFailure
}

export interface LogicalSize {
  width: number
  height: number
}

export interface CaptureTarget {
  id: string
  logicalSize: LogicalSize
}

export interface PixelSize {
  width: number
  height: number
}

export interface CaptureGeometry {
  coordinateSpace: 'target-logical'
  x: number
  y: number
  width: number
  height: number
}

export interface DisplayCaptureRequest {
  delivery: OutputDelivery
  saveDirectory?: string
}

export interface CommitRegionRequest extends DisplayCaptureRequest {
  sessionId: string
  geometry: CaptureGeometry
}

export interface PreparedRegionCapture {
  status: 'prepared'
  sessionId: string
  targetLogicalSize: LogicalSize
  preview: {
    filePath: string
    mediaType: 'image/png'
    pixelSize: PixelSize
  }
  leaseMilliseconds: number
}

export interface FailedCaptureResult {
  status: 'failed'
  failure: PlatformFailure
}

export type PrepareRegionResult = PreparedRegionCapture | FailedCaptureResult

export interface ReleasedRegionCapture {
  status: 'released'
}

export type DeliveryResult =
  | { target: 'clipboard'; status: 'success' }
  | { target: 'folder'; status: 'success'; filePath: string }
  | { target: DeliveryTarget; status: 'failed'; failure: PlatformFailure }

export type CaptureResult =
  | {
      status: 'completed'
      sourceDynamicRange: 'sdr' | 'hdr'
      outputProfile: 'srgb-visual-match'
      deliveries: readonly DeliveryResult[]
    }
  | { status: 'cancelled' }
  | FailedCaptureResult

export interface ScreenCapturePermissionRequestResult {
  status: 'granted' | 'restart-required' | 'not-granted'
}

export interface PlatformFailure {
  code:
    | 'host-unavailable'
    | 'permission-denied'
    | 'capture-unavailable'
    | 'delivery-unavailable'
    | 'delivery-failed'
    | 'invalid-request'
    | 'unexpected-failure'
  message: string
  retryable: boolean
}

export interface PlatformHost {
  getCapabilities(): Promise<PlatformCapabilities>
  captureDisplay(request: DisplayCaptureRequest): Promise<CaptureResult>
  prepareRegion(targetId: string): Promise<PrepareRegionResult>
  commitRegion(request: CommitRegionRequest): Promise<CaptureResult>
  cancelRegion(sessionId: string): Promise<ReleasedRegionCapture>
}

export interface MacOSScreenCapturePermissionHost {
  requestScreenCapturePermission(): Promise<ScreenCapturePermissionRequestResult>
}

export type HostMethod =
  | 'getCapabilities'
  | 'captureDisplay'
  | 'prepareRegion'
  | 'commitRegion'
  | 'cancelRegion'
  | 'requestScreenCapturePermission'

export type PlatformRequestEnvelope =
  | {
      version: typeof PLATFORM_CONTRACT_VERSION
      id: string
      method: 'getCapabilities'
      params: Record<string, never>
    }
  | {
      version: typeof PLATFORM_CONTRACT_VERSION
      id: string
      method: 'prepareRegion'
      params: { targetId: string }
    }
  | {
      version: typeof PLATFORM_CONTRACT_VERSION
      id: string
      method: 'captureDisplay'
      params: DisplayCaptureRequest
    }
  | {
      version: typeof PLATFORM_CONTRACT_VERSION
      id: string
      method: 'commitRegion'
      params: CommitRegionRequest
    }
  | {
      version: typeof PLATFORM_CONTRACT_VERSION
      id: string
      method: 'cancelRegion'
      params: { sessionId: string }
    }
  | {
      version: typeof PLATFORM_CONTRACT_VERSION
      id: string
      method: 'requestScreenCapturePermission'
      params: Record<string, never>
    }

export type PlatformResponseEnvelope =
  | {
      version: typeof PLATFORM_CONTRACT_VERSION
      id: string
      result:
        | PlatformCapabilities
        | CaptureResult
        | PreparedRegionCapture
        | ReleasedRegionCapture
        | ScreenCapturePermissionRequestResult
    }
  | {
      version: typeof PLATFORM_CONTRACT_VERSION
      id: string
      error: PlatformFailure
    }

const outputDeliveries: readonly OutputDelivery[] = ['clipboard', 'folder', 'both']

export function parseDisplayCaptureRequest(value: unknown): DisplayCaptureRequest {
  const request = parseDeliveryRequest(value, [])
  return {
    delivery: request.delivery,
    ...(request.saveDirectory ? { saveDirectory: request.saveDirectory } : {}),
  }
}

export function parseCommitRegionRequest(value: unknown): CommitRegionRequest {
  const request = parseDeliveryRequest(value, ['sessionId', 'geometry'])
  if (!isRecord(value) || typeof value.sessionId !== 'string' || value.sessionId.length === 0) {
    throw new PlatformContractError('Region session id must be a non-empty string.')
  }
  return {
    sessionId: value.sessionId,
    delivery: request.delivery,
    ...(request.saveDirectory ? { saveDirectory: request.saveDirectory } : {}),
    geometry: parseCaptureGeometry(value.geometry),
  }
}

export function parseRegionSessionId(value: unknown): string {
  if (
    !isRecord(value) ||
    typeof value.sessionId !== 'string' ||
    value.sessionId.length === 0 ||
    !hasExactKeys(value, ['sessionId'])
  ) {
    throw new PlatformContractError('Region session id must be a non-empty string.')
  }
  return value.sessionId
}

function parseDeliveryRequest(
  value: unknown,
  extraKeys: readonly string[],
): { delivery: OutputDelivery; saveDirectory?: string } {
  if (!isRecord(value)) {
    throw new PlatformContractError('Capture request must be an object.')
  }
  if (!outputDeliveries.includes(value.delivery as OutputDelivery)) {
    throw new PlatformContractError('Capture delivery must be clipboard, folder, or both.')
  }
  const delivery = value.delivery as OutputDelivery
  const saveDirectory = parseSaveDirectory(value.saveDirectory, delivery)
  const expected = saveDirectory
    ? ['delivery', 'saveDirectory', ...extraKeys]
    : ['delivery', ...extraKeys]
  if (!hasExactKeys(value, expected)) {
    throw new PlatformContractError('Capture request contains missing or unknown fields.')
  }
  return { delivery, ...(saveDirectory ? { saveDirectory } : {}) }
}

function parseSaveDirectory(value: unknown, delivery: OutputDelivery): string | undefined {
  if (value === undefined) return undefined
  if (delivery === 'clipboard') {
    throw new PlatformContractError('Clipboard-only capture must not include a save directory.')
  }
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new PlatformContractError('Save directory must be a non-empty string.')
  }
  return value
}

export function deliveryTargetsFor(delivery: OutputDelivery): readonly DeliveryTarget[] {
  return delivery === 'both' ? ['clipboard', 'folder'] : [delivery]
}

export class PlatformContractError extends Error {
  public constructor(message: string) {
    super(message)
    this.name = 'PlatformContractError'
  }
}

export function parseCaptureGeometry(value: unknown): CaptureGeometry {
  if (!isRecord(value)) {
    throw new PlatformContractError('Region geometry must be an object.')
  }
  if (!hasExactKeys(value, ['coordinateSpace', 'x', 'y', 'width', 'height'])) {
    throw new PlatformContractError('Region geometry contains missing or unknown fields.')
  }
  if (value.coordinateSpace !== 'target-logical') {
    throw new PlatformContractError('Region geometry must use target-logical coordinates.')
  }
  if (!isNonNegativeFiniteNumber(value.x) || !isNonNegativeFiniteNumber(value.y)) {
    throw new PlatformContractError('Region origin must contain finite non-negative numbers.')
  }
  if (!isPositiveFiniteNumber(value.width) || !isPositiveFiniteNumber(value.height)) {
    throw new PlatformContractError('Region size must contain finite positive numbers.')
  }
  return {
    coordinateSpace: 'target-logical',
    x: value.x,
    y: value.y,
    width: value.width,
    height: value.height,
  }
}

function isPositiveFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

function isNonNegativeFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Object.keys(value)
  return actual.length === expected.length && expected.every((key) => key in value)
}
