import { JointHomingState } from '@/gen/marengo/v1/marengo_pb';

export type ActuatorCardBadge = {
  id: string;
  label: string;
  /** Tailwind text/border tone — keep chroma semantic only. */
  tone: 'ok' | 'accent' | 'fault' | 'muted' | 'warning';
};

/**
 * Compact status chips for Testing telemetry cards.
 * Drive and reference come from the joint's own wire fields (`drive_active`,
 * `homing_state`), never from the robot-wide mode: with a commissioning-scope
 * subset enabled, an undriven or unverified joint must not read ENABLED/ZEROED.
 */
export function resolveActuatorCardBadges(args: {
  operationalMode: string | null;
  driveActive: boolean;
  homingState: JointHomingState;
  fault: number;
}): ActuatorCardBadge[] {
  const badges: ActuatorCardBadge[] = [];

  if (args.fault !== 0) {
    badges.push({
      id: 'fault',
      label: `FAULT 0x${args.fault.toString(16).padStart(4, '0')}`,
      tone: 'fault',
    });
  }

  if (args.operationalMode === null) {
    badges.push({ id: 'drive', label: 'UNKNOWN', tone: 'muted' });
  } else if (args.driveActive) {
    badges.push({ id: 'drive', label: 'ENABLED', tone: 'accent' });
  } else {
    badges.push({ id: 'drive', label: 'DISABLED', tone: 'muted' });
  }

  switch (args.homingState) {
    case JointHomingState.VERIFIED:
      badges.push({ id: 'zero', label: 'ZEROED', tone: 'ok' });
      break;
    case JointHomingState.HOMING:
      badges.push({ id: 'zero', label: 'HOMING', tone: 'warning' });
      break;
    case JointHomingState.FAULTED:
      badges.push({ id: 'zero', label: 'REF FAULT', tone: 'fault' });
      break;
    case JointHomingState.UNHOMED:
      badges.push({ id: 'zero', label: 'UNHOMED', tone: 'warning' });
      break;
    default:
      // UNSPECIFIED: the publisher did not populate the field.
      badges.push({ id: 'zero', label: 'REF ?', tone: 'muted' });
      break;
  }

  return badges;
}

export function badgeToneClass(tone: ActuatorCardBadge['tone']): string {
  switch (tone) {
    case 'ok':
      return 'border-[color:var(--ok)]/40 text-[color:var(--ok)]';
    case 'accent':
      return 'border-accent/50 text-accent';
    case 'fault':
      return 'border-fault/50 text-fault';
    case 'warning':
      return 'border-warning/50 text-warning';
    case 'muted':
      return 'border-border text-muted-foreground';
    default: {
      const _exhaustive: never = tone;
      return _exhaustive;
    }
  }
}
