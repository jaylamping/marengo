export type ActuatorCardBadge = {
  id: string;
  /** Short label — shown on text chips; used as tooltip title for icon chips. */
  label: string;
  /** Tailwind text/border tone — keep chroma semantic only. */
  tone: 'ok' | 'accent' | 'fault' | 'muted' | 'warning';
  /**
   * `icon` = dense glyph + tooltip (zero state).
   * Drive / fault stay as text chips — those words are the signal.
   */
  presentation?: 'text' | 'icon';
  /** Extra tooltip body when presentation is `icon`. */
  detail?: string;
};

/**
 * Compact status chips for Testing telemetry cards.
 * Drive mode is robot-wide (Davout); zero/fault are per-joint.
 */
export function resolveActuatorCardBadges(args: {
  operationalMode: string | null;
  zeroed: boolean;
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

  switch (args.operationalMode) {
    case 'ACTIVE':
      badges.push({ id: 'drive', label: 'ENABLED', tone: 'accent' });
      break;
    case 'READY':
      badges.push({ id: 'drive', label: 'READY', tone: 'ok' });
      break;
    case 'DISABLED':
      badges.push({ id: 'drive', label: 'DISABLED', tone: 'muted' });
      break;
    default:
      badges.push({ id: 'drive', label: 'UNKNOWN', tone: 'muted' });
      break;
  }

  if (args.zeroed || args.operationalMode === 'READY' || args.operationalMode === 'ACTIVE') {
    badges.push({
      id: 'zero',
      label: 'ZEROED',
      tone: 'ok',
      presentation: 'icon',
      detail: 'Encoder origin confirmed (set-zero or READY/ACTIVE sync).',
    });
  } else {
    badges.push({
      id: 'zero',
      label: 'UNHOMED',
      tone: 'warning',
      presentation: 'icon',
      detail: 'Set zero before commanding this joint.',
    });
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
