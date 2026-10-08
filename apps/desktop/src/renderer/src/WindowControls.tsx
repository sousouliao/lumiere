import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Button } from '@/components/motion/button/base'
import { useEffect, useState } from 'react'

// Electron supplied these buttons outside the renderer. Tauri's frameless window
// uses the same 46px Windows caption slots while retaining the existing title row.
export function WindowControls(): React.JSX.Element | null {
  return isTauri() ? <TauriWindowControls /> : null
}

function TauriWindowControls(): React.JSX.Element {
  const [maximized, setMaximized] = useState(false)
  useEffect(() => {
    let active = true
    let stop: (() => void) | undefined
    const update = (): void => {
      void getCurrentWindow()
        .isMaximized()
        .then((value) => {
          if (active) setMaximized(value)
        })
    }
    update()
    void getCurrentWindow()
      .onResized(update)
      .then((unlisten) => {
        if (active) stop = unlisten
        else unlisten()
      })
    return () => {
      active = false
      stop?.()
    }
  }, [])
  return (
    <div className="window-controls">
      <Button
        variant="ghost"
        size="icon"
        className="window-control"
        hoverScale={1}
        pressScale={1}
        aria-label="Minimize"
        onClick={() => void getCurrentWindow().minimize()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 5.5h10" />
        </svg>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="window-control"
        hoverScale={1}
        pressScale={1}
        aria-label={maximized ? 'Restore' : 'Maximize'}
        onClick={() => void getCurrentWindow().toggleMaximize()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d={maximized ? 'M.5 2.5h7v7h-7z M2.5 2.5v-2h7v7h-2' : 'M.5.5h9v9h-9z'} />
        </svg>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="window-control window-control--close"
        hoverScale={1}
        pressScale={1}
        aria-label="Close"
        onClick={() => void getCurrentWindow().close()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="m.5.5 9 9m0-9-9 9" />
        </svg>
      </Button>
    </div>
  )
}
