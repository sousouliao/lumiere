import { describe, expect, it } from 'vitest'
import {
  formatShortcutAccelerator,
  parseShortcutUpdate,
  shortcutFromKeyInput,
  ShortcutContractError,
} from './shortcut-command'

describe('shortcut command contract', () => {
  it('normalizes supported Windows key combinations', () => {
    expect(
      shortcutFromKeyInput({
        key: '4',
        metaKey: true,
        ctrlKey: false,
        altKey: false,
        shiftKey: true,
      }),
    ).toBe('Command+Shift+4')
    expect(
      shortcutFromKeyInput({
        key: 'l',
        metaKey: false,
        ctrlKey: true,
        altKey: false,
        shiftKey: true,
      }),
    ).toBe('Control+Shift+L')
  })

  it.each([
    { key: 'l', metaKey: false, ctrlKey: false, altKey: false, shiftKey: true },
    { key: 'Shift', metaKey: true, ctrlKey: false, altKey: false, shiftKey: true },
    { key: '`', metaKey: true, ctrlKey: false, altKey: false, shiftKey: false },
  ])('rejects unsafe or unsupported key input', (input) => {
    expect(() => shortcutFromKeyInput(input)).toThrow(ShortcutContractError)
  })

  it('parses a narrow shortcut update payload and supports clearing', () => {
    expect(parseShortcutUpdate({ mode: 'region', accelerator: 'Command+Shift+L' })).toEqual({
      mode: 'region',
      accelerator: 'Command+Shift+L',
    })
    expect(parseShortcutUpdate({ mode: 'display', accelerator: null })).toEqual({
      mode: 'display',
      accelerator: null,
    })
    expect(() => parseShortcutUpdate({ mode: 'window', accelerator: 'Command+Shift+L' })).toThrow(
      ShortcutContractError,
    )
  })

  it('formats registered accelerators with platform conventions', () => {
    expect(formatShortcutAccelerator('Command+Alt+Shift+L')).toBe('Super+Alt+Shift+L')
    expect(formatShortcutAccelerator('Control+Shift+L')).toBe('Ctrl+Shift+L')
    expect(formatShortcutAccelerator(null)).toBe('Not configured')
  })
})
