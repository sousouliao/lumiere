import type {
  CaptureCommandResult,
  CaptureNotice,
  CaptureSurfaceSnapshot,
} from '../../shared/capture-command'

export const CAPTURE_LOAD_FAILURE: CaptureNotice = {
  tone: 'critical',
  title: 'Capture is unavailable',
  detail: 'Try again, or restart Lumiere if the issue persists.',
}

interface ResolveCaptureNoticesInput {
  snapshot: CaptureSurfaceSnapshot | null
  result: CaptureCommandResult | null
  loadFailed: boolean
}

export function resolveCaptureNotices({
  snapshot,
  result,
  loadFailed,
}: ResolveCaptureNoticesInput): {
  activeNotice: CaptureNotice | undefined
  blockingNotice: CaptureNotice | undefined
  detailNotice: CaptureNotice | undefined
} {
  const resultNotice =
    result?.status === 'failed' || result?.status === 'partial' ? result.notice : undefined
  const blockingNotice = loadFailed ? CAPTURE_LOAD_FAILURE : snapshot?.blockingNotice
  const detailNotice = resultNotice ?? snapshot?.advisoryNotice
  return {
    activeNotice: resultNotice ?? blockingNotice ?? snapshot?.advisoryNotice,
    blockingNotice,
    detailNotice,
  }
}
