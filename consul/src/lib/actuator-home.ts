import { create } from '@bufbuild/protobuf';
import {
  ControlMode,
  MitCommandBatchSchema,
  MitJointCommandSchema,
} from '@/gen/marengo/v1/marengo_pb';
import { postTestingMitCommandBatch } from '@/lib/gateway-api';
import { useActuatorZeroStore } from '@/state/actuatorZeroStore';
import { useTeachStore } from '@/state/teachStore';

/** Position-mode gains: Berthier owns trajectory; match compound playback. */
const HOME_GAINS = { kp: 0, kd: 0, ki: 0, fc: 0 };

export type ActuatorHomeGate = {
  ok: boolean;
  reason: string | null;
};

/**
 * Home is available only after the joint is known zero'd, the row is
 * interactive/live, and motors are ACTIVE (position command).
 */
export function actuatorHomeGate(args: {
  interactive: boolean;
  connected: boolean;
  live: boolean;
  jointName: string;
  operationalMode: string | null;
  zeroed: boolean;
}): ActuatorHomeGate {
  if (!args.interactive) {
    return { ok: false, reason: 'Actuator must be online and configured.' };
  }
  if (!args.connected || !args.live) {
    return { ok: false, reason: 'Live telemetry required for Home.' };
  }
  // READY/ACTIVE means Davout already accepted Verified zeros for the profile.
  const modeImpliesZeroed =
    args.operationalMode === 'READY' || args.operationalMode === 'ACTIVE';
  const zeroed = args.zeroed || modeImpliesZeroed;
  if (!zeroed) {
    return {
      ok: false,
      reason:
        'Unlock zero first: Confirm Set Zero in Limits, or Testing → Home (syncs Verified).',
    };
  }
  if (args.operationalMode !== 'ACTIVE') {
    return {
      ok: false,
      reason: `Enable motors first (Testing: Home → READY → Enable). Current: ${args.operationalMode ?? 'unknown'}.`,
    };
  }
  return { ok: true, reason: null };
}

/** Command this joint to 0 rad under ControlMode.POSITION. */
export async function postActuatorHome(jointName: string): Promise<void> {
  const joint = jointName.trim();
  if (!joint) {
    throw new Error('joint required');
  }
  // Gate already required store mark or READY/ACTIVE; persist unlock after Pi-only zeros.
  useActuatorZeroStore.getState().markZeroed(joint);
  useTeachStore.getState().setGravityArmed(false);
  await postTestingMitCommandBatch(
    create(MitCommandBatchSchema, {
      timestampMs: BigInt(Date.now()),
      mode: ControlMode.POSITION,
      joints: [
        create(MitJointCommandSchema, {
          name: joint,
          kp: HOME_GAINS.kp,
          kd: HOME_GAINS.kd,
          ki: HOME_GAINS.ki,
          fc: HOME_GAINS.fc,
          position: 0,
          velocity: 0,
          torqueFf: 0,
        }),
      ],
    }),
  );
}
