/** Renderer vocabulary. The native process protocol is owned by capture-contract. */
export type LumierePlatform = 'windows'
export type CaptureMode = 'region' | 'display'
export type OutputDelivery = 'clipboard' | 'folder' | 'both'
export type DeliveryTarget = 'clipboard' | 'folder'

export function deliveryTargetsFor(delivery: OutputDelivery): readonly DeliveryTarget[] {
  return delivery === 'both' ? ['clipboard', 'folder'] : [delivery]
}
