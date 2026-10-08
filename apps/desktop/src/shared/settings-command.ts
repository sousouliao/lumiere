import { deliveryTargetsFor, type DeliveryTarget, type OutputDelivery } from './platform-contract'
import type { CaptureShortcutSnapshot, ShortcutUpdate } from './shortcut-command'

export const outputDeliveryOptions: readonly OutputDelivery[] = ['clipboard', 'folder', 'both']
export const afterCaptureBehaviorOptions = ['do-nothing', 'show-in-folder'] as const
export type AfterCaptureBehavior = (typeof afterCaptureBehaviorOptions)[number]

export interface SettingsSnapshot {
  outputDelivery: OutputDelivery
  availableOutputDeliveries: readonly OutputDelivery[]
  saveDirectory: string
  captureShortcuts: CaptureShortcutSnapshot
  afterCaptureBehavior: AfterCaptureBehavior
  hdrStatusReminders: boolean
}

export type ShortcutUpdateResult =
  { status: 'success'; snapshot: SettingsSnapshot } | { status: 'failed'; message: string }

export interface LumiereSettingsApi {
  getSettingsSnapshot(): Promise<SettingsSnapshot>
  chooseSaveDirectory(): Promise<SettingsSnapshot>
  setOutputDelivery(delivery: OutputDelivery): Promise<SettingsSnapshot>
  setAfterCaptureBehavior(behavior: AfterCaptureBehavior): Promise<SettingsSnapshot>
  setHdrStatusReminders(enabled: boolean): Promise<SettingsSnapshot>
  setCaptureShortcut(update: ShortcutUpdate): Promise<ShortcutUpdateResult>
  setShortcutRecording(recording: boolean): Promise<void>
  onSettingsChanged(listener: (snapshot: SettingsSnapshot) => void): () => void
  onShowSettingsRequested(listener: () => void): () => void
}

export function parseOutputDelivery(value: unknown): OutputDelivery {
  if (!outputDeliveryOptions.includes(value as OutputDelivery)) {
    throw new SettingsContractError('Output delivery must be clipboard, folder, or both.')
  }
  return value as OutputDelivery
}

export function parseAfterCaptureBehavior(value: unknown): AfterCaptureBehavior {
  if (!afterCaptureBehaviorOptions.includes(value as AfterCaptureBehavior)) {
    throw new SettingsContractError('After-capture behavior must be do-nothing or show-in-folder.')
  }
  return value as AfterCaptureBehavior
}

export function parseHdrStatusReminders(value: unknown): boolean {
  if (typeof value !== 'boolean') {
    throw new SettingsContractError('HDR status reminders must be enabled or disabled.')
  }
  return value
}

export function availableOutputDeliveries(
  deliveryTargets: readonly DeliveryTarget[],
): readonly OutputDelivery[] {
  return outputDeliveryOptions.filter((delivery) =>
    deliveryTargetsFor(delivery).every((target) => deliveryTargets.includes(target)),
  )
}

export class SettingsContractError extends Error {
  public constructor(message: string) {
    super(message)
    this.name = 'SettingsContractError'
  }
}
