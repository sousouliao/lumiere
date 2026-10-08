import type { WindowsRendererApi } from '../../shared/windows-renderer-api'

declare global {
  interface Window {
    lumierePlatform: WindowsRendererApi
  }
}

export {}
