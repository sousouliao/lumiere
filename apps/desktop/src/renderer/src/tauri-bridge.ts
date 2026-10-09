import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { LumiereRendererApi } from '../../shared/capture-command'

// Registration is asynchronous. An unmount before it resolves still releases the
// native listener, including React StrictMode's setup/cleanup/setup sequence.
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- T also constrains listen's native payload.
export function subscribe<T>(event: string, callback: (value: T) => void): () => void {
  let stopped = false
  let unlisten: (() => void) | undefined
  void listen<T>(event, ({ payload }) => {
    if (!stopped) callback(payload)
  }).then((stop) => {
    if (stopped) stop()
    else unlisten = stop
  })
  return () => {
    stopped = true
    unlisten?.()
  }
}

export const windowsApi: LumiereRendererApi = {
  platform: 'windows',
  getCaptureSurfaceSnapshot: () => invoke('get_capture_surface_snapshot'),
  onCaptureSurfaceChanged: (callback) => subscribe('capture-surface-changed', callback),
  captureDisplay: () => invoke('capture_display'),
  captureRegion: () => invoke('capture_region'),
  getCaptureActivity: () => invoke('get_capture_activity'),
  onCaptureActivityChanged: (callback) => subscribe('capture-activity-changed', callback),
  refreshCaptureSurface: () => invoke('refresh_capture_surface'),
  recoverCapture: (id, action) => invoke('recover_capture', { id, action }),
  onShowCaptureRequested: (callback) => subscribe('show-capture-requested', callback),
  getSettingsSnapshot: () => invoke('get_settings_snapshot'),
  getAutostartSnapshot: () => invoke('get_autostart_snapshot'),
  setAutostartEnabled: (enabled) => invoke('set_autostart_enabled', { enabled }),
  openStartupSettings: () => invoke('open_startup_settings'),
  chooseSaveDirectory: () => invoke('choose_save_directory'),
  setOutputDelivery: (delivery) => invoke('set_output_delivery', { delivery }),
  setAfterCaptureBehavior: (behavior) => invoke('set_after_capture_behavior', { behavior }),
  setHdrStatusReminders: (enabled) => invoke('set_hdr_status_reminders', { enabled }),
  setCaptureShortcut: (update) => invoke('set_capture_shortcut', { update }),
  setShortcutRecording: (recording) => invoke('set_shortcut_recording', { recording }),
  onSettingsChanged: (callback) => subscribe('settings-changed', callback),
  onShowSettingsRequested: (callback) => subscribe('show-settings-requested', callback),
  getUpdateSnapshot: () => invoke('get_update_snapshot'),
  checkForUpdates: () => invoke('check_for_updates'),
  downloadUpdate: () => invoke('download_update'),
  installUpdate: () => invoke('install_update'),
  onUpdateChanged: (callback) => subscribe('update-changed', callback),
}
