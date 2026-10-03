import {
  patchConfig,
  type ConfigPatchDto,
  type ConfigPatchResultDto,
} from '@/lib/config-api';
import type { JointRangeBounds } from '@/lib/limit-listen';
import { localLimitSyncCredential } from '@/lib/local-limit-sync-session';

/** ADR 0009 hard/soft gap (~27 mrad). */
export const DEFAULT_SOFT_INSET_RAD = 0.027;

export type LocalLimitSyncStatus = 'ok' | 'skipped' | 'failed';

export type PersistJointLimitsResult =
  | {
      ok: true;
      lower: number;
      upper: number;
      softLower: number;
      softUpper: number;
      restartRequired: boolean;
      persistStatus: string;
      localSync: LocalLimitSyncStatus;
      message: string;
    }
  | { ok: false; message: string };

type PatchConfigFn = (
  patch: ConfigPatchDto,
  init?: { signal?: AbortSignal },
) => Promise<ConfigPatchResultDto | null>;

export type LocalLimitSyncFn = (args: {
  joint: string;
  lower: number;
  upper: number;
  softLower: number;
  softUpper: number;
}) => Promise<LocalLimitSyncStatus>;

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
    expectedRevision?: string;
    patchConfig?: PatchConfigFn;
    timeoutMs?: number;
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
  if (!deps?.expectedRevision) {
    return { ok: false, message: 'Config revision unavailable; refresh before applying limits.' };
  }

  // Persist taught hard as SoT (what the operator swept). Enable-at-stop jitter
  // is covered by Davout `position_limit_measured_fault_slack_rad` (~30 mrad),
  // not by silently widening motors.yaml hard on every Apply.
  const hardLower = bounds.lower;
  const hardUpper = bounds.upper;
  const { softLower, softUpper } = softLimitsWithInset(hardLower, hardUpper);
  const patch = deps?.patchConfig ?? patchConfig;
  const timeoutMs = deps?.timeoutMs ?? DEFAULT_PATCH_TIMEOUT_MS;
  const controller = new AbortController();
  const timer = window.setTimeout(() => controller.abort(), timeoutMs);
  let result: ConfigPatchResultDto | null;
  try {
    result = await patch(
      {
        joint,
        expected_revision: deps.expectedRevision,
        position_lower_rad: hardLower,
        position_upper_rad: hardUpper,
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
        'Gateway rejected or timed out the limits patch. Check connectivity and your Robot access credential.',
    };
  }
  if (!result.ok) {
    return {
      ok: false,
      message:
        result.message ||
        'Limits patch failed. Check your configuration credential in Robot access.',
    };
  }

  const persistStatus = result.persist_status ?? 'unknown';
  let localSync: LocalLimitSyncStatus = 'skipped';
  if (persistStatus === 'durable') {
    const sync = deps?.localSync ?? defaultLocalLimitSync;
    localSync = await sync({
      joint,
      lower: hardLower,
      upper: hardUpper,
      softLower,
      softUpper,
    });
  }

  const localNote =
    localSync === 'ok'
      ? ' Local checkout synced.'
      : localSync === 'failed'
        ? ' Local checkout sync failed — is just limit-sync-serve running?'
        : '';

  return {
    ok: true,
    lower: hardLower,
    upper: hardUpper,
    softLower,
    softUpper,
    restartRequired: result.restart_required,
    persistStatus,
    localSync,
    message: `${result.message}${localNote}`,
  };
}

async function defaultLocalLimitSync(args: {
  joint: string;
  lower: number;
  upper: number;
  softLower: number;
  softUpper: number;
}): Promise<LocalLimitSyncStatus> {
  const base = (
    import.meta.env.VITE_LIMIT_SYNC_URL as string | undefined
  )?.trim();
  if (!base) {
    return 'skipped';
  }
  const credential = localLimitSyncCredential();
  if (!credential) {
    return 'failed';
  }
  try {
    const res = await fetch(`${base.replace(/\/$/, '')}/local/limit-patch`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${credential}`,
      },
      signal: AbortSignal.timeout(12_000),
      body: JSON.stringify({
        joint: args.joint,
        lower: args.lower,
        upper: args.upper,
        soft_lower: args.softLower,
        soft_upper: args.softUpper,
      }),
    });
    if (!res.ok) {
      return 'failed';
    }
    return 'ok';
  } catch {
    return 'failed';
  }
}
