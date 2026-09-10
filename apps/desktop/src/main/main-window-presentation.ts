export interface PresentableWindow {
  once(event: 'ready-to-show', listener: () => void): void
  on(event: 'close', listener: (event: { preventDefault(): void }) => void): void
  isDestroyed(): boolean
  isMinimized(): boolean
  restore(): void
  show(): void
  focus(): void
  hide(): void
}

interface MainWindowPresentationOptions {
  isQuitting(): boolean
  becameReady(): void
}

export class MainWindowPresentation {
  private ready = false
  private showPending = false

  public constructor(
    private readonly window: PresentableWindow,
    private readonly options: MainWindowPresentationOptions,
  ) {
    window.once('ready-to-show', () => {
      this.ready = true
      options.becameReady()
      if (this.showPending) this.reveal()
    })
    window.on('close', (event) => {
      if (options.isQuitting()) return
      event.preventDefault()
      window.hide()
    })
  }

  public show(): void {
    if (this.window.isDestroyed()) return
    this.showPending = true
    if (this.ready) this.reveal()
  }

  private reveal(): void {
    if (this.window.isDestroyed()) return
    this.showPending = false
    if (this.window.isMinimized()) this.window.restore()
    this.window.show()
    this.window.focus()
  }
}

export function opensWindowOnTrayClick(platform: NodeJS.Platform): boolean {
  return platform === 'win32'
}
