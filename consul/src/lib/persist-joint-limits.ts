import {
  patchConfig,
  type ConfigPatchDto,
  type ConfigPatchResultDto,
} from '@/lib/config-api';
import type { JointRangeBounds } from '@/lib/limit-listen';

/** ADR 0009 hard/soft gap (~27 mrad). */
export const DEFAULT_SOFT_INSET_RAD = 0.027;

export type PersistJointLimitsResult =
  | {
      ok: true;
      lower: number;
      upper: number;
      softLower: number;
      softUpper: number;
      restartRequired: boolean;
      persistStatus: string;
      localSync: 'ok' | 'skipped' | 'failed';
      message: string;
    }
  | { ok: false; message: string };

type PatchConfigFn = (
  patch: ConfigPatchDto,
  init?: { signal?: AbortSignal },
) => Promise<ConfigPatchResultDto | null>;

export type LocalLimitSyncFn = (args: {
  profile: string;
  joint: string;
  lower: number;
  upper: number;
  softLower: number;
  softUpper: number;
}) => Promise<'ok' | 'skipped' | 'failed'>;

const DEFAULT_PATCH_TIMEOUT_MS = 30_000;

export function softLimitsWithInset(
  hardLower: number,
  hardUpper: number,
  inset: number = DEFAULT_SOFT_INSET_RAD,
): { softLower: number; softUpper: number } {
  const span = hardUpper - hardLower;
  if (!Number.isFinite(span) || span <= 0) {
    return { softLower: hardLower, softUpper: hardUpper };
  }
  const clamped = Math.min(Math.max(inset, 0), span * 0.25);
  return {
    softLower: hardLower + clamped,
    softUpper: hardUpper - clamped,
  };
}

/**
 * Persist measured Set Limits bounds to Pi (motors + soft + expand-only URDF).
 * After Durable ACK, best-effort sync the local git checkout via marengo-limit-sync.
 */
export async function persistJointLimits(
  joint: string,
  bounds: JointRangeBounds,
  deps?: {
    patchConfig?: PatchConfigFn;
    timeoutMs?: number;
    profile?: string;
    localSync?: LocalLimitSyncFn;
  },
): Promise<PersistJointLimitsResult> {
  if (
    !Number.isFinite(bounds.lower) ||
    !Number.isFinite(bounds.upper) ||
    bounds.lower >= bounds.upper
  ) {
    return { ok: false, message: 'Invalid limit bounds.' };
  }

  const { softLower, softUpper } = softLimitsWithInset(bounds.lower, bounds.upper);
  const patch = deps?.patchConfig ?? patchConfig;
  const timeoutMs = deps?.timeoutMs ?? DEFAULT_PATCH_TIMEOUT_MS;
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), timeoutMs);
  let result: ConfigPatchResultDto | null;
  try {
    result = await patch(
      {
        joint,
        position_lower_rad: bounds.lower,
        position_upper_rad: bounds.upper,
        position_soft_lower_rad: softLower,
        position_soft_upper_rad: softUpper,
      },
      { signal: controller.signal },
    );
  } finally {
    window.clearTimeout(timer);
  }

  if (!result) {
    return {
      ok: false,
      message:
        'Gateway rejected or timed out the limits patch (is Chappe up, and is VITE_MARENGO_LOG_TOKEN set for /config/patch?).',
    };
  }
  if (!result.ok) {
    return {
      ok: false,
      message:
        result.message ||
        'Limits patch failed (check VITE_MARENGO_LOG_TOKEN matches Pi MARENGO_GATEWAY_LOG_TOKEN).',
    };
  }

  const persistStatus = result.persist_status ?? 'unknown';
  let localSync: 'ok' | 'skipped' | 'failed' = 'skipped';
  if (persistStatus === 'durable') {
    const sync = deps?.localSync ?? defaultLocalLimitSync;
    localSync = await sync({
      profile: deps?.profile ?? 'arm_4dof_right',
      joint,
      lower: bounds.lower,
      upper: bounds.upper,
      softLower,
      softUpper,
    });
  }

  const localNote =
    localSync === 'ok'
      ? ' Local checkout synced.'
      : localSync === 'failed'
        ? ' Local checkout sync failed (Pi durable).'
        : persistStatus === 'durable'
          ? ' Local checkout not synced (marengo-limit-sync unavailable).'
          : '';

  return {
    ok: true,
    lower: bounds.lower,
    upper: bounds.upper,
    softLower,
    softUpper,
    restartRequired: result.restart_required,
    persistStatus,
    localSync,
    message: `${result.message}${localNote}`,
  };
}

async function defaultLocalLimitSync(args: {
  profile: string;
  joint: string;
  lower: number;
  upper: number;
  softLower: number;
  softUpper: number;
}): Promise<'ok' | 'skipped' | 'failed'> {
  const base =
    (import.meta.env.VITE_LIMIT_SYNC_URL as string | undefined)?.trim() ||
    'http://127.0.0.1:8790';
  try {
    const res = await fetch(`${base.replace(/\/$/, '')}/local/limit-patch`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        profile: args.profile,
        joint: args.joint,
        lower: args.lower,
        upper: args.upper,
        soft_lower: args.softLower,
        soft_upper: args.softUpper,
      }),
    });
    if (res.status === 404 || res.status === 0) {
      return 'skipped';
    }
    if (!res.ok) {
      return 'failed';
    }
    return 'ok';
  } catch {
    return 'skipped';
  }
}
