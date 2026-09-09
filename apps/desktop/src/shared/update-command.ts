export const updateCommandChannels = {
  getSnapshot: 'update:get-snapshot',
  check: 'update:check',
  openLatestRelease: 'update:open-latest-release',
} as const

export interface UpdateSnapshot {
  currentVersion: string
}

export type UpdateCheckResult =
  | { status: 'up-to-date'; currentVersion: string }
  | { status: 'available'; currentVersion: string; availableVersion: string }
  | { status: 'failed'; currentVersion: string }

export interface LumiereUpdateApi {
  getUpdateSnapshot(): Promise<UpdateSnapshot>
  checkForUpdates(): Promise<UpdateCheckResult>
  openLatestRelease(): Promise<void>
}
