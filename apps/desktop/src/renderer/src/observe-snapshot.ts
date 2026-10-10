import type { SnapshotSubscription } from '../../shared/snapshot-command'

/** Own the initial-read/event race and the lifetime of one snapshot subscription. */
export function observeSnapshot<T>({
  read,
  subscribe,
  onSnapshot,
  onError = () => undefined,
}: {
  read: () => Promise<T>
  subscribe: (listener: (snapshot: T) => void) => SnapshotSubscription
  onSnapshot: (snapshot: T, source: 'initial' | 'event') => void
  onError?: (error: unknown) => void
}): () => void {
  const state = { current: true, receivedEvent: false }
  const acceptsInitial = () => state.current && !state.receivedEvent
  const stop = subscribe((snapshot) => {
    if (!state.current) return
    state.receivedEvent = true
    onSnapshot(snapshot, 'event')
  })
  void stop.ready
    .then(async () => {
      if (!acceptsInitial()) return
      const snapshot = await read()
      if (acceptsInitial()) onSnapshot(snapshot, 'initial')
    })
    .catch((error: unknown) => {
      if (acceptsInitial()) onError(error)
    })
  return () => {
    state.current = false
    stop()
  }
}
