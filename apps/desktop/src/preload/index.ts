import { contextBridge, ipcRenderer } from 'electron'
import { captureCommandChannels, type LumiereRendererApi } from '../shared/capture-command'
import { settingsCommandChannels, type SettingsSnapshot } from '../shared/settings-command'
import type { LumierePlatform } from '../shared/platform-contract'

const platform: LumierePlatform = process.platform === 'darwin' ? 'macos' : 'windows'

const platformApi: LumiereRendererApi = {
  platform,
  getCaptureSurfaceSnapshot: () => ipcRenderer.invoke(captureCommandChannels.getSurfaceSnapshot),
  onCaptureSurfaceChanged: (listener) => {
    const handleChanged = (
      _event: Electron.IpcRendererEvent,
      snapshot: Parameters<typeof listener>[0],
    ): void => {
      listener(snapshot)
    }
    ipcRenderer.on(captureCommandChannels.surfaceChanged, handleChanged)
    return () => {
      ipcRenderer.removeListener(captureCommandChannels.surfaceChanged, handleChanged)
    }
  },
  captureDisplay: () => ipcRenderer.invoke(captureCommandChannels.captureDisplay),
  captureRegion: () => ipcRenderer.invoke(captureCommandChannels.captureRegion),
  getCaptureActivity: () => ipcRenderer.invoke(captureCommandChannels.getActivity),
  onCaptureActivityChanged: (listener) => {
    const handle = (
      _event: Electron.IpcRendererEvent,
      activity: Parameters<typeof listener>[0],
    ): void => {
      listener(activity)
    }
    ipcRenderer.on(captureCommandChannels.activityChanged, handle)
    return () => {
      ipcRenderer.removeListener(captureCommandChannels.activityChanged, handle)
    }
  },
  refreshCaptureSurface: () => ipcRenderer.invoke(captureCommandChannels.refreshSurface),
  recoverCapture: (id, action) => ipcRenderer.invoke(captureCommandChannels.recover, id, action),
  fitCaptureContent: (height) => {
    ipcRenderer.send(captureCommandChannels.fitContent, height)
  },
  onShowCaptureRequested: (listener) => {
    const handle = (): void => {
      listener()
    }
    ipcRenderer.on(captureCommandChannels.showRequested, handle)
    return () => {
      ipcRenderer.removeListener(captureCommandChannels.showRequested, handle)
    }
  },
  onRegionOverlayActivated: (listener) => {
    const handleActivated = (
      _event: Electron.IpcRendererEvent,
      snapshot: Parameters<typeof listener>[0],
    ): void => {
      listener(snapshot)
    }
    ipcRenderer.on(captureCommandChannels.regionOverlayActivated, handleActivated)
    return () => {
      ipcRenderer.removeListener(captureCommandChannels.regionOverlayActivated, handleActivated)
    }
  },
  onRegionOverlayReset: (listener) => {
    const handleReset = (): void => {
      listener()
    }
    ipcRenderer.on(captureCommandChannels.regionOverlayReset, handleReset)
    return () => {
      ipcRenderer.removeListener(captureCommandChannels.regionOverlayReset, handleReset)
    }
  },
  regionOverlayHostReady: () => {
    ipcRenderer.send(captureCommandChannels.regionOverlayHostReady)
  },
  regionOverlayReady: (generation) => {
    ipcRenderer.send(captureCommandChannels.regionOverlayReady, generation)
  },
  cancelRegionOverlay: (generation) => {
    ipcRenderer.send(captureCommandChannels.cancelRegionOverlay, generation)
  },
  submitRegionSelection: (generation, geometry) => {
    ipcRenderer.send(captureCommandChannels.submitRegionSelection, generation, geometry)
  },
  getSettingsSnapshot: () => ipcRenderer.invoke(settingsCommandChannels.getSnapshot),
  chooseSaveDirectory: () => ipcRenderer.invoke(settingsCommandChannels.chooseSaveDirectory),
  setAfterCaptureBehavior: (behavior) =>
    ipcRenderer.invoke(settingsCommandChannels.setAfterCaptureBehavior, behavior),
  setHdrStatusReminders: (enabled) =>
    ipcRenderer.invoke(settingsCommandChannels.setHdrStatusReminders, enabled),
  setOutputDelivery: (delivery) =>
    ipcRenderer.invoke(settingsCommandChannels.setOutputDelivery, delivery),
  setCaptureShortcut: (update) =>
    ipcRenderer.invoke(settingsCommandChannels.setCaptureShortcut, update),
  setShortcutRecording: (recording) =>
    ipcRenderer.invoke(settingsCommandChannels.setShortcutRecording, recording),
  onSettingsChanged: (listener) => {
    const handleChanged = (_event: Electron.IpcRendererEvent, snapshot: SettingsSnapshot): void => {
      listener(snapshot)
    }
    ipcRenderer.on(settingsCommandChannels.changed, handleChanged)
    return () => {
      ipcRenderer.removeListener(settingsCommandChannels.changed, handleChanged)
    }
  },
  onShowSettingsRequested: (listener) => {
    const handleShowRequested = (): void => {
      listener()
    }
    ipcRenderer.on(settingsCommandChannels.showRequested, handleShowRequested)
    return () => {
      ipcRenderer.removeListener(settingsCommandChannels.showRequested, handleShowRequested)
    }
  },
}

contextBridge.exposeInMainWorld('lumierePlatform', platformApi)
