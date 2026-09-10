import { NativeProcessPlatformHost, type SpawnHost } from './native-process-platform-host'
import type { MacOSScreenCapturePermissionHost } from '../shared/platform-contract'

export class MacOSPlatformHost
  extends NativeProcessPlatformHost
  implements MacOSScreenCapturePermissionHost
{
  public constructor(executableCandidates: readonly string[], spawnHost?: SpawnHost) {
    super('macos', executableCandidates, spawnHost)
  }
}
