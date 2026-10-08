import type { LumiereRendererApi } from './capture-command'
import type { LumiereMacOSPermissionRecoveryApi } from './macos-permission-recovery-command'

/** Transitional omission keeps the former shell reviewable until P6 removes it. */
export type WindowsRendererApi = Omit<
  LumiereRendererApi,
  | keyof LumiereMacOSPermissionRecoveryApi
  | 'onRegionOverlayActivated'
  | 'onRegionOverlayReset'
  | 'regionOverlayHostReady'
  | 'regionOverlayReady'
  | 'cancelRegionOverlay'
  | 'submitRegionSelection'
> & { readonly platform: 'windows' }
