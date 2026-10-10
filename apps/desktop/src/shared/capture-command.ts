import type { CaptureMode, LumierePlatform, OutputDelivery } from './platform-contract'
import type { LumiereSettingsApi } from './settings-command'
import type { LumiereUpdateApi } from './update-command'
import type { LumiereAutostartApi } from './autostart-command'
import type { SnapshotSubscription } from './snapshot-command'

export type ProductHdrStatus = 'ready' | 'unavailable' | 'unvalidated'

export interface CaptureNotice {
  tone: 'critical' | 'caution'
  title: string
  detail: string
  recovery?: 'folder' | 'output'
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

export type CaptureRecoveryAction = 'capture-again' | 'choose-folder' | 'open-settings' | 'refresh'

export function captureRecoveryActions(
  result: CaptureCommandResult,
): { action: CaptureRecoveryAction; label: string }[] {
  if (result.status !== 'failed' && result.status !== 'partial') return []
  switch (result.notice.recovery) {
    case 'folder':
      return [
        { action: 'choose-folder', label: 'Choose save folder…' },
        { action: 'capture-again', label: 'Capture again' },
      ]
    case 'output':
      return [
        { action: 'open-settings', label: 'Output settings…' },
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

export interface LumiereRendererApi
  extends LumiereSettingsApi, LumiereUpdateApi, LumiereAutostartApi {
  readonly platform: LumierePlatform
  getCaptureSurfaceSnapshot(): Promise<CaptureSurfaceSnapshot>
  onCaptureSurfaceChanged(
    listener: (snapshot: CaptureSurfaceSnapshot) => void,
  ): SnapshotSubscription
  captureDisplay(): Promise<CaptureCommandResult>
  captureRegion(): Promise<CaptureCommandResult>
  getCaptureActivity(): Promise<CaptureActivity>
  onCaptureActivityChanged(listener: (activity: CaptureActivity) => void): SnapshotSubscription
  refreshCaptureSurface(): Promise<CaptureSurfaceSnapshot>
  recoverCapture(id: number, action: CaptureRecoveryAction): Promise<void>
  onShowCaptureRequested(listener: () => void): () => void
}
