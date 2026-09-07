import type {
  CaptureGeometry,
  CaptureMode,
  LogicalSize,
  LumierePlatform,
  OutputDelivery,
} from './platform-contract'
import type { LumiereSettingsApi } from './settings-command'

export const captureCommandChannels = {
  captureDisplay: 'capture:display',
  captureRegion: 'capture:region',
  getActivity: 'capture:get-activity',
  activityChanged: 'capture:activity-changed',
  refreshSurface: 'capture:refresh-surface',
  recover: 'capture:recover',
  showRequested: 'capture:show-requested',
  getSurfaceSnapshot: 'capture:get-surface-snapshot',
  surfaceChanged: 'capture:surface-changed',
  regionOverlayHostReady: 'region-overlay:host-ready',
  regionOverlayActivated: 'region-overlay:activated',
  regionOverlayReset: 'region-overlay:reset',
  regionOverlayReady: 'region-overlay:ready',
  cancelRegionOverlay: 'region-overlay:cancel',
  submitRegionSelection: 'region-overlay:submit-selection',
} as const

export interface RegionOverlaySnapshot {
  generation: number
  targetSize: LogicalSize
  previewPixelSize: LogicalSize
  previewUrl: string
}

export type ProductHdrStatus = 'ready' | 'unavailable' | 'unvalidated'

export interface CaptureNotice {
  tone: 'critical' | 'caution'
  title: string
  detail: string
  recovery?: 'permissions' | 'folder' | 'output'
}

export interface CaptureCompletion {
  id: number
  mode: CaptureMode
  result: CaptureCommandResult
}

export interface CaptureActivity {
  activeMode: CaptureMode | null
  lastCompletion: CaptureCompletion | null
}

export type CaptureRecoveryAction =
  'capture-again' | 'open-permissions' | 'choose-folder' | 'open-settings' | 'refresh'

export function captureRecoveryActions(
  result: CaptureCommandResult,
  platform: LumierePlatform,
): { action: CaptureRecoveryAction; label: string }[] {
  if (result.status !== 'failed' && result.status !== 'partial') return []
  switch (result.notice.recovery) {
    case 'permissions':
      return [
        {
          action: platform === 'macos' ? 'open-permissions' : 'open-settings',
          label: platform === 'macos' ? 'Open System Settings' : 'Open settings',
        },
        { action: 'refresh', label: 'Check again' },
      ]
    case 'folder':
      return [
        { action: 'choose-folder', label: 'Choose save folder' },
        { action: 'capture-again', label: 'Capture again' },
      ]
    case 'output':
      return [
        { action: 'open-settings', label: 'Output settings' },
        { action: 'capture-again', label: 'Capture again' },
      ]
    default:
      return [{ action: 'capture-again', label: 'Capture again' }]
  }
}

export interface CaptureOutputSummary {
  delivery: OutputDelivery
  label: string
  location: string
}

export interface CaptureSurfaceSnapshot {
  platform: LumierePlatform
  hostAvailable: boolean
  captureModes: readonly CaptureMode[]
  hdrStatus: ProductHdrStatus
  output: CaptureOutputSummary
  blockingNotice?: CaptureNotice
  advisoryNotice?: CaptureNotice
}

export type CaptureCommandResult =
  | {
      status: 'success'
      feedback: string
      filePath?: string
    }
  | {
      status: 'partial'
      feedback: string
      notice: CaptureNotice
      filePath?: string
    }
  | {
      status: 'cancelled'
      feedback: string
    }
  | {
      status: 'failed'
      feedback: string
      notice: CaptureNotice
    }

export interface LumiereRendererApi extends LumiereSettingsApi {
  readonly platform: LumierePlatform
  getCaptureSurfaceSnapshot(): Promise<CaptureSurfaceSnapshot>
  onCaptureSurfaceChanged(listener: (snapshot: CaptureSurfaceSnapshot) => void): () => void
  captureDisplay(): Promise<CaptureCommandResult>
  captureRegion(): Promise<CaptureCommandResult>
  getCaptureActivity(): Promise<CaptureActivity>
  onCaptureActivityChanged(listener: (activity: CaptureActivity) => void): () => void
  refreshCaptureSurface(): Promise<CaptureSurfaceSnapshot>
  recoverCapture(id: number, action: CaptureRecoveryAction): Promise<void>
  onShowCaptureRequested(listener: () => void): () => void
  onRegionOverlayActivated(listener: (snapshot: RegionOverlaySnapshot) => void): () => void
  onRegionOverlayReset(listener: () => void): () => void
  regionOverlayHostReady(): void
  regionOverlayReady(generation: number): void
  cancelRegionOverlay(generation: number): void
  submitRegionSelection(generation: number, geometry: CaptureGeometry): void
}
