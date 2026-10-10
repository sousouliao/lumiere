/** Stop is synchronous even when native listener registration is still pending. */
export type SnapshotSubscription = (() => void) & { ready: Promise<void> }
