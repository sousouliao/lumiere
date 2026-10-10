import { useEffect, useRef, useState, type KeyboardEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Button } from '@/components/motion/button/base'

interface CaptureItem {
  label: string
  shortcut: string
  enabled: boolean
}
interface Snapshot {
  revision: number
  region: CaptureItem
  display: CaptureItem
}
const initial: Snapshot = {
  revision: 0,
  region: { label: 'Capture Region', shortcut: '', enabled: false },
  display: { label: 'Capture Display', shortcut: '', enabled: false },
}

export function TrayMenu(): React.JSX.Element {
  const [snapshot, setSnapshot] = useState(initial)
  const [pending, setPending] = useState(false)
  const actionPending = useRef(false)
  const buttons = useRef<(HTMLButtonElement | null)[]>([])

  useEffect(() => {
    let current = true
    let revision = 0
    const apply = (value: Snapshot) => {
      if (current && value.revision >= revision) {
        revision = value.revision
        setSnapshot(value)
      }
    }
    let focusFrame = 0
    const unlisteners: UnlistenFn[] = []
    const register = (unlisten: UnlistenFn) => {
      if (current) unlisteners.push(unlisten)
      else unlisten()
    }
    const subscriptions = Promise.allSettled([
      listen<Snapshot>('tray-menu-changed', ({ payload }) => {
        if (!current) return
        apply(payload)
      }).then(register),
      listen('tray-menu-opened', () => {
        if (!current) return
        actionPending.current = false
        setPending(false)
        cancelAnimationFrame(focusFrame)
        focusFrame = requestAnimationFrame(() => {
          buttons.current.find((button) => button && !button.disabled)?.focus()
        })
      }).then(register),
    ])
    void subscriptions
      .then(async (results) => {
        for (const result of results) {
          if (result.status === 'rejected')
            console.error('Tray menu subscription failed', result.reason)
        }
        const value = await invoke<Snapshot>('get_tray_menu_snapshot')
        apply(value)
      })
      .catch((error: unknown) => {
        console.error('Tray menu initialization failed', error)
      })
      .then(async () => {
        if (current) await invoke('tray_menu_ready')
      })
      .catch((error: unknown) => {
        console.error('Tray menu readiness failed', error)
      })
    return () => {
      current = false
      cancelAnimationFrame(focusFrame)
      unlisteners.forEach((unlisten) => {
        unlisten()
      })
    }
  }, [])

  const activate = (action: string) => {
    if (actionPending.current) return
    actionPending.current = true
    setPending(true)
    void invoke('tray_menu_action', { action }).catch((error: unknown) => {
      console.error('Tray menu action failed', action, error)
      actionPending.current = false
      setPending(false)
    })
  }
  const keyboard = (event: KeyboardEvent) => {
    if (event.key === 'Escape' || event.key === 'Tab') {
      event.preventDefault()
      void getCurrentWindow().close()
      return
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
    event.preventDefault()
    const enabled = buttons.current.filter((button): button is HTMLButtonElement =>
      Boolean(button && !button.disabled),
    )
    if (!enabled.length) return
    const index = enabled.findIndex((button) => button === document.activeElement)
    const next =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? enabled.length - 1
          : index === -1
            ? event.key === 'ArrowDown'
              ? 0
              : enabled.length - 1
            : (index + (event.key === 'ArrowDown' ? 1 : -1) + enabled.length) % enabled.length
    enabled[next]?.focus()
  }
  const items = [
    { action: 'region', ...snapshot.region },
    { action: 'display', ...snapshot.display },
    { action: 'open', label: 'Open Lumiere', shortcut: '', enabled: true },
    { action: 'settings', label: 'Settings…', shortcut: '', enabled: true },
    { action: 'quit', label: 'Quit Lumiere', shortcut: '', enabled: true },
  ]
  return (
    <div
      className="tray-menu-backdrop"
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) void getCurrentWindow().close()
      }}
    >
      <div className="tray-menu" role="menu" aria-label="Lumiere" onKeyDown={keyboard}>
        {items.map((item, index) => (
          <div className="tray-menu-group" key={item.action}>
            {index === 2 || index === 4 ? (
              <div role="separator" className="tray-menu-separator" />
            ) : null}
            <Button
              ref={(element) => {
                buttons.current[index] = element
              }}
              className="tray-menu-item"
              role="menuitem"
              tabIndex={-1}
              variant="ghost"
              hoverScale={1}
              pressScale={1}
              disabled={!item.enabled || pending}
              onClick={() => {
                activate(item.action)
              }}
            >
              <span>{item.label}</span>
              {item.shortcut ? (
                <span className="tray-menu-shortcut" title={item.shortcut}>
                  {item.shortcut}
                </span>
              ) : null}
            </Button>
          </div>
        ))}
      </div>
    </div>
  )
}
