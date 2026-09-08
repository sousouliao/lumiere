import type { CaptureTarget } from '../shared/platform-contract'

const DEFAULT_RETRY_DELAY_MILLISECONDS = 50
const DEFAULT_MAX_ATTEMPTS = 3

interface DisplayTarget {
  id: number
  bounds: { x: number; y: number; width: number; height: number }
}

interface RegionTargetResolverOptions {
  expectedDisplayId: number
  readDisplay(): DisplayTarget
  readTarget(): Promise<CaptureTarget | null>
  isActive(): boolean
  matchesTarget(display: DisplayTarget, target: CaptureTarget): boolean
  retryDelayMilliseconds?: number
  maxAttempts?: number
  wait?: (milliseconds: number) => Promise<void>
}

export type RegionTargetResolution =
  | { status: 'ready'; display: DisplayTarget; target: CaptureTarget }
  | { status: 'display-changed'; displayId: number }
  | { status: 'unavailable' }
  | { status: 'cancelled' }

/** Resolves a stable native target without spinning when the Host is temporarily unavailable. */
export async function resolveRegionTargetWithRetry(
  options: RegionTargetResolverOptions,
): Promise<RegionTargetResolution> {
  const maxAttempts = options.maxAttempts ?? DEFAULT_MAX_ATTEMPTS
  const retryDelayMilliseconds = options.retryDelayMilliseconds ?? DEFAULT_RETRY_DELAY_MILLISECONDS
  const configuredWait = options.wait
  const wait = configuredWait ? (milliseconds: number) => configuredWait(milliseconds) : waitFor

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    if (!options.isActive()) return { status: 'cancelled' }
    const display = options.readDisplay()
    if (display.id !== options.expectedDisplayId) {
      return { status: 'display-changed', displayId: display.id }
    }

    const target = await options.readTarget()
    if (!options.isActive()) return { status: 'cancelled' }
    const currentDisplay = options.readDisplay()
    if (currentDisplay.id !== options.expectedDisplayId) {
      return { status: 'display-changed', displayId: currentDisplay.id }
    }
    if (target && options.matchesTarget(currentDisplay, target)) {
      return { status: 'ready', display: currentDisplay, target }
    }
    if (attempt < maxAttempts) await wait(retryDelayMilliseconds)
  }

  return { status: 'unavailable' }
}

function waitFor(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds))
}
