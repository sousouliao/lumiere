import { afterEach, describe, expect, it, vi } from 'vitest'
const mocks = vi.hoisted(() => ({
  app: { isPackaged: false, getVersion: () => '0.6.0' },
  access: vi.fn(() => Promise.resolve()),
  loaded: vi.fn(),
  client: {
    autoDownload: true,
    autoInstallOnAppQuit: true,
    allowPrerelease: true,
    disableDifferentialDownload: false,
    on: vi.fn(),
    checkForUpdates: vi.fn(),
    downloadUpdate: vi.fn(),
    quitAndInstall: vi.fn(),
  },
}))
vi.mock('electron', () => ({ app: mocks.app }))
vi.mock('node:fs/promises', () => ({ access: mocks.access }))
vi.mock('electron-updater', () => {
  mocks.loaded()
  return { default: { autoUpdater: mocks.client } }
})
import { configureWindowsUpdates } from './windows-updater'
import type { WindowsUpdateService } from './windows-update-service'
let service: WindowsUpdateService | null = null
afterEach(() => {
  service?.dispose()
  vi.unstubAllGlobals()
  vi.clearAllMocks()
})
describe('Windows update configuration', () => {
  it('never loads the real updater or accesses packaged metadata in development', async () => {
    vi.stubGlobal('process', { ...process, platform: 'win32', resourcesPath: '/resources' })
    mocks.app.isPackaged = false
    service = await configureWindowsUpdates(
      vi.fn(),
      () => false,
      () => Promise.resolve(),
      vi.fn(),
    )
    expect(service?.getSnapshot().windowsUpdate?.status).toBe('disabled')
    expect(mocks.access).not.toHaveBeenCalled()
    expect(mocks.loaded).not.toHaveBeenCalled()
  })
  it('loads the CommonJS default export and requires explicit download and install', async () => {
    vi.stubGlobal('process', { ...process, platform: 'win32', resourcesPath: '/resources' })
    mocks.app.isPackaged = true
    service = await configureWindowsUpdates(
      vi.fn(),
      () => false,
      () => Promise.resolve(),
      vi.fn(),
    )
    expect(mocks.access).toHaveBeenCalledWith(expect.stringContaining('app-update.yml'))
    expect(mocks.loaded).toHaveBeenCalledOnce()
    expect(mocks.client).toMatchObject({
      autoDownload: false,
      autoInstallOnAppQuit: false,
      allowPrerelease: false,
      disableDifferentialDownload: true,
    })
    expect(service?.getSnapshot().windowsUpdate?.status).toBe('idle')
  })
})
