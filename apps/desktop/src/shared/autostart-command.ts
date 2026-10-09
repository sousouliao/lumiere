/** Windows owns login registration; it is not persisted in application settings. */
export type AutostartSnapshot =
  { status: 'enabled' | 'disabled' | 'blocked' } | { status: 'unavailable'; message: string }

export interface LumiereAutostartApi {
  getAutostartSnapshot(): Promise<AutostartSnapshot>
  setAutostartEnabled(enabled: boolean): Promise<AutostartSnapshot>
  openStartupSettings(): Promise<void>
}
