/**
 * Soft-invalidate taught overlays after set-zero / calibration change.
 *
 * Home / enable alone must NOT bump the epoch (operators re-home often).
 * Bump only when the operator marks a set-zero (or a future Verified-drop detector).
 * Overlay stays in storage; Wave with overlay is blocked until Acknowledge or Reset.
 */

import type { TeachSession } from '@/lib/teach-transit';

export interface TeachCalibrationGate {
  /** Monotonic; bumps when calibration of teach joints may have changed. */
  liveCalibrationEpoch: number;
  /** Last epoch the operator acknowledged for this overlay (or Apply time). */
  ackedAtEpoch: number;
}

/** True when Wave must not use the overlay until Ack or Reset. */
export function overlayNeedsCalibrationAck(
  session: TeachSession | undefined,
  gate: TeachCalibrationGate | undefined
): boolean {
  if (!session || !gate) return false;
  const baseline = gate.ackedAtEpoch;
  return gate.liveCalibrationEpoch > baseline;
}

export function initialAckEpoch(liveEpoch: number): number {
  return liveEpoch;
}
