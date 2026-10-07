import { app } from 'electron'
import { access } from 'node:fs/promises'
import { join } from 'node:path'
import { WindowsUpdateService, type WindowsUpdateClient } from './windows-update-service'

export async function configureWindowsUpdates(
  changed: ConstructorParameters<typeof WindowsUpdateService>[2],
  captureBusy: () => boolean,
  stopHost: () => Promise<void>,
  resumeHost: () => void,
): Promise<WindowsUpdateService | null> {
  if (process.platform !== 'win32') return null
  let client: WindowsUpdateClient | null = null
  if (app.isPackaged) {
    try {
      await access(join(process.resourcesPath, 'app-update.yml'))
      // electron-updater is CommonJS; Electron's native ESM namespace exposes its default.
      const { default: electronUpdater } = await import('electron-updater')
      const { autoUpdater } = electronUpdater
      autoUpdater.autoDownload = false
      autoUpdater.autoInstallOnAppQuit = false
      autoUpdater.allowPrerelease = false
      autoUpdater.disableDifferentialDownload = true
      autoUpdater.on('error', (error) => {
        console.error('operation=WindowsUpdate stage=Failed', error)
      })
      client = autoUpdater
    } catch (error) {
      console.warn('operation=WindowsUpdate stage=Configure result=disabled', error)
    }
  }
  return new WindowsUpdateService(
    app.getVersion(),
    client,
    changed,
    captureBusy,
    stopHost,
    resumeHost,
  )
}
