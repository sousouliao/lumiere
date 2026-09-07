import { Notification } from 'electron'
import type { CaptureCompletion } from '../shared/capture-command'

/** System notifications only announce background failures; recovery stays in the main window. */
export class CaptureFailureNotifier {
  private notification: Notification | null = null
  private lastNotifiedId: number | null = null

  public constructor(private readonly openResult: (id: number) => void) {}

  public notify(completion: CaptureCompletion, foreground: boolean): void {
    const { result, id } = completion
    if (foreground || (result.status !== 'failed' && result.status !== 'partial')) {
      this.clear()
      return
    }
    if (this.lastNotifiedId === id) return
    this.clear()
    this.lastNotifiedId = id
    try {
      if (!Notification.isSupported()) return
      const notification = new Notification({
        title: result.status === 'partial' ? 'Screenshot needs attention' : result.notice.title,
        body:
          result.status === 'partial'
            ? `${result.feedback}. Click to open Lumiere.`
            : `${result.notice.detail} Click to open Lumiere.`,
        silent: true,
      })
      this.notification = notification
      notification.once('click', () => {
        if (this.notification !== notification) return
        this.clear()
        this.openResult(id)
      })
      notification.once('failed', (_event, error) => {
        console.warn('operation=CaptureNotification stage=Failed', error)
      })
      notification.show()
    } catch (error) {
      console.warn('operation=CaptureNotification stage=Unavailable', error)
      this.clear()
    }
  }

  public clear(): void {
    const notification = this.notification
    this.notification = null
    try {
      notification?.close()
    } catch (error) {
      console.warn('operation=CaptureNotification stage=CloseFailed', error)
    }
  }
}
