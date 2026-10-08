import type { LumiereRendererApi } from '../../shared/capture-command'

declare global {
  interface Window {
    lumierePlatform: LumiereRendererApi
  }
}

export {}
