import type { InventoryItem } from '@/data/robot-inventory';

/**
 * Online + configured: motion/telemetry panels unlock.
 * Offline / unassigned rows stay open but dithered except identity edits.
 * This is inventory gating only — not a claim of live motor feedback.
 */
export function isSubsystemInteractive(item: InventoryItem): boolean {
  if (item.status === 'Offline') {
    return false;
  }
  if (item.kind === 'actuator' && item.preset === 'unassigned') {
    return false;
  }
  return true;
}
