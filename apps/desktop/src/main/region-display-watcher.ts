const DEFAULT_POLL_INTERVAL_MILLISECONDS = 100

interface RegionDisplayWatcherOptions {
  readDisplayId(): number
  onDisplayChanged(displayId: number): void
  pollIntervalMilliseconds?: number
}

/** Watches the global pointer while the Region overlay only covers one display. */
export class RegionDisplayWatcher {
  private timer: NodeJS.Timeout | null = null
  private observedDisplayId: number | null = null

  public constructor(private readonly options: RegionDisplayWatcherOptions) {}

  public start(displayId: number): void {
    this.stop()
    this.observedDisplayId = displayId
    this.timer = setInterval(() => {
      const nextDisplayId = this.options.readDisplayId()
      if (nextDisplayId === this.observedDisplayId) return
      this.observedDisplayId = nextDisplayId
      this.options.onDisplayChanged(nextDisplayId)
    }, this.options.pollIntervalMilliseconds ?? DEFAULT_POLL_INTERVAL_MILLISECONDS)
  }

  public stop(): void {
    if (this.timer) {
      clearInterval(this.timer)
      this.timer = null
    }
  }

  public dispose(): void {
    this.stop()
    this.observedDisplayId = null
  }
}
