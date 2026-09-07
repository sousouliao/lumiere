import type {
  CaptureActivity,
  CaptureCommandResult,
  CaptureCompletion,
} from '../shared/capture-command'
import type { CaptureMode } from '../shared/platform-contract'

/** Application-level capture lifetime, including Overlay preparation and result delivery. */
export class CaptureSession {
  private activity: CaptureActivity = { activeMode: null, lastCompletion: null }
  private nextId = 0

  public constructor(private readonly changed: (activity: CaptureActivity) => void) {}

  public getSnapshot(): CaptureActivity {
    return this.activity
  }

  public clearCompletion(id: number): void {
    if (!this.activity.activeMode && this.activity.lastCompletion?.id === id) {
      this.publish({ activeMode: null, lastCompletion: null })
    }
  }

  public async run(
    mode: CaptureMode,
    capture: () => Promise<CaptureCommandResult>,
    completed: (completion: CaptureCompletion) => void,
  ): Promise<CaptureCommandResult> {
    if (this.activity.activeMode) {
      return {
        status: 'failed',
        feedback: 'A capture is already in progress',
        notice: {
          tone: 'caution',
          title: 'A capture is already in progress',
          detail: 'Wait for it to finish, then try again.',
        },
      }
    }
    this.publish({ activeMode: mode, lastCompletion: null })
    let result: CaptureCommandResult
    try {
      result = await capture()
    } catch (error) {
      console.error('operation=Capture stage=Failed', error)
      result = {
        status: 'failed',
        feedback: 'Capture failed',
        notice: {
          tone: 'critical',
          title: 'Capture failed',
          detail: 'Start a new capture. Restart Lumiere if the issue continues.',
        },
      }
    }
    const completion = { id: ++this.nextId, mode, result }
    this.publish({ activeMode: null, lastCompletion: completion })
    completed(completion)
    return result
  }

  private publish(activity: CaptureActivity): void {
    this.activity = activity
    this.changed(activity)
  }
}
