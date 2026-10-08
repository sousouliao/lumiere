import type { CaptureMode as PlatformCaptureMode } from './platform-contract'

export type CaptureMode = PlatformCaptureMode
export type CaptureShortcuts = Record<CaptureMode, string | null>
export type ShortcutRegistrationStatus = 'unconfigured' | 'registered' | 'unavailable'

export interface ShortcutSnapshot {
  accelerator: string | null
  status: ShortcutRegistrationStatus
}

export type CaptureShortcutSnapshot = Record<CaptureMode, ShortcutSnapshot>

export interface ShortcutUpdate {
  mode: CaptureMode
  accelerator: string | null
}

export interface ShortcutKeyInput {
  key: string
  metaKey: boolean
  ctrlKey: boolean
  altKey: boolean
  shiftKey: boolean
}

const MODIFIERS = ['Command', 'Control', 'Alt', 'Shift'] as const
const PRIMARY_MODIFIERS = new Set<string>(['Command', 'Control', 'Alt'])
const SPECIAL_KEYS: Readonly<Record<string, string>> = {
  ' ': 'Space',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  ArrowUp: 'Up',
}

export function shortcutFromKeyInput(input: ShortcutKeyInput): string {
  const key = normalizeKey(input.key)
  const modifiers: string[] = []
  if (input.metaKey) modifiers.push('Command')
  if (input.ctrlKey) modifiers.push('Control')
  if (input.altKey) modifiers.push('Alt')
  if (input.shiftKey) modifiers.push('Shift')
  if (!modifiers.some((modifier) => PRIMARY_MODIFIERS.has(modifier))) {
    throw new ShortcutContractError('Include Super, Control, or Alt in the shortcut.')
  }
  return parseShortcutAccelerator([...modifiers, key].join('+'))
}

export function parseShortcutUpdate(value: unknown): ShortcutUpdate {
  if (
    typeof value !== 'object' ||
    value === null ||
    Array.isArray(value) ||
    Object.keys(value).length !== 2 ||
    !('mode' in value) ||
    !('accelerator' in value) ||
    (value.mode !== 'region' && value.mode !== 'display')
  ) {
    throw new ShortcutContractError('Expected one region or display shortcut update.')
  }
  return {
    mode: value.mode,
    accelerator: value.accelerator === null ? null : parseShortcutAccelerator(value.accelerator),
  }
}

export function parseShortcutAccelerator(value: unknown): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 80) {
    throw new ShortcutContractError('Shortcut accelerator must be a short string.')
  }
  const parts = value.split('+')
  if (parts.length < 2) {
    throw new ShortcutContractError('Shortcut accelerator requires a modifier and a key.')
  }
  const key = parts.at(-1)
  const modifiers = parts.slice(0, -1)
  if (
    !key ||
    !isSupportedKey(key) ||
    modifiers.some((modifier) => !MODIFIERS.includes(modifier as (typeof MODIFIERS)[number])) ||
    new Set(modifiers).size !== modifiers.length ||
    !modifiers.some((modifier) => PRIMARY_MODIFIERS.has(modifier))
  ) {
    throw new ShortcutContractError('Shortcut accelerator is unsupported.')
  }
  return [...MODIFIERS.filter((modifier) => modifiers.includes(modifier)), key].join('+')
}

export function formatShortcutAccelerator(accelerator: string | null): string {
  if (!accelerator) return 'Not configured'
  const parts = parseShortcutAccelerator(accelerator).split('+')
  const labels: Readonly<Record<string, string>> = {
    Command: 'Super',
    Control: 'Ctrl',
  }
  return parts.map((part) => labels[part] ?? part).join('+')
}

function normalizeKey(key: string): string {
  const special = SPECIAL_KEYS[key]
  if (special) return special
  if (/^[a-z]$/i.test(key)) return key.toUpperCase()
  if (/^[0-9]$/.test(key) || /^F(?:[1-9]|1[0-9]|2[0-4])$/.test(key)) return key.toUpperCase()
  throw new ShortcutContractError('Use a letter, number, function, arrow, or Space key.')
}

function isSupportedKey(key: string): boolean {
  return (
    /^[A-Z0-9]$/.test(key) ||
    /^F(?:[1-9]|1[0-9]|2[0-4])$/.test(key) ||
    ['Space', 'Up', 'Down', 'Left', 'Right'].includes(key)
  )
}

export class ShortcutContractError extends Error {
  public constructor(message: string) {
    super(message)
    this.name = 'ShortcutContractError'
  }
}
