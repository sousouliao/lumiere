import { app, Menu, nativeImage, Tray } from 'electron'
import { resolveDesktopIconPaths, type DesktopIconPlatform } from './icon-paths'
import {
  applicationTrayMenuTemplate,
  type ApplicationTrayCommands,
  type ApplicationTrayState,
} from './tray-menu'
import { opensWindowOnTrayClick } from './main-window-presentation'

function currentIconPlatform(): DesktopIconPlatform {
  if (process.platform === 'darwin' || process.platform === 'win32') {
    return process.platform
  }

  throw new Error(`Unsupported desktop icon platform: ${process.platform}`)
}

export function desktopIconPaths() {
  return resolveDesktopIconPaths({
    appPath: app.getAppPath(),
    isPackaged: app.isPackaged,
    platform: currentIconPlatform(),
    resourcesPath: process.resourcesPath,
  })
}

export interface ApplicationTray {
  update(state: ApplicationTrayState): void
  destroy(): void
}

export function createApplicationTray(
  state: ApplicationTrayState,
  commands: ApplicationTrayCommands,
): ApplicationTray {
  const icon = nativeImage.createFromPath(desktopIconPaths().trayIcon)
  if (icon.isEmpty()) {
    throw new Error('The application tray icon could not be loaded.')
  }
  if (process.platform === 'darwin') {
    icon.setTemplateImage(true)
  }

  const tray = new Tray(icon)
  tray.setToolTip('Lumiere')
  const update = (nextState: ApplicationTrayState): void => {
    tray.setContextMenu(Menu.buildFromTemplate(applicationTrayMenuTemplate(nextState, commands)))
  }
  update(state)
  if (opensWindowOnTrayClick(process.platform)) {
    tray.on('click', () => {
      commands.showWindow()
    })
  }
  return {
    update,
    destroy: () => {
      tray.destroy()
    },
  }
}
