import type {
  CaptureResult,
  LumierePlatform,
  PlatformCapabilities,
  PlatformHost,
  PrepareRegionResult,
  ReleasedRegionCapture,
} from '../shared/platform-contract'
import { PLATFORM_CONTRACT_VERSION } from '../shared/platform-contract'

export class UnavailablePlatformHost implements PlatformHost {
  public constructor(private readonly platform: LumierePlatform) {}

  public getCapabilities(): Promise<PlatformCapabilities> {
    return Promise.resolve({
      contractVersion: PLATFORM_CONTRACT_VERSION,
      platform: this.platform,
      hostStatus: 'unavailable',
      captureModes: [],
      deliveryTargets: [],
      hdrCapture: 'unavailable',
      outputProfiles: ['srgb-visual-match'],
      unavailableReason: {
        code: 'host-unavailable',
        message: `The ${this.platform} native capture host has not been connected yet.`,
        retryable: false,
      },
    })
  }

  public captureDisplay(): Promise<CaptureResult> {
    return Promise.resolve(this.unavailableResult())
  }

  public prepareRegion(targetId: string): Promise<PrepareRegionResult> {
    void targetId
    return Promise.resolve(this.unavailableResult())
  }

  public commitRegion(): Promise<CaptureResult> {
    return Promise.resolve(this.unavailableResult())
  }

  public cancelRegion(): Promise<ReleasedRegionCapture> {
    return Promise.resolve({ status: 'released' })
  }

  private unavailableResult(): Extract<CaptureResult, { status: 'failed' }> {
    return {
      status: 'failed',
      failure: {
        code: 'host-unavailable',
        message: `The ${this.platform} native capture host has not been connected yet.`,
        retryable: false,
      },
    }
  }
}

export function currentLumierePlatform(): LumierePlatform {
  if (process.platform === 'darwin') {
    return 'macos'
  }

  if (process.platform === 'win32') {
    return 'windows'
  }

  throw new Error(`Lumiere does not support ${process.platform}.`)
}
