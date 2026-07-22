import {
  patchConfig,
  type ConfigPatchDto,
  type ConfigPatchResultDto,
} from '@/lib/config-api';
import type { JointRangeBounds } from '@/lib/limit-listen';

export type PersistJointLimitsResult =
  | {
      ok: true;
      lower: number;
      upper: number;
      restartRequired: boolean;
      message: string;
    }
  | { ok: false; message: string };

type PatchConfigFn = (
  patch: ConfigPatchDto,
  init?: { signal?: AbortSignal },
) => Promise<ConfigPatchResultDto | null>;

const DEFAULT_PATCH_TIMEOUT_MS = 15_000;

/**
 * Persist measured Set Limits bounds to motors.yaml via gateway /config/patch.
 * Pass exact listen bounds — never a display Range string.
 */
export async function persistJointLimits(
  joint: string,
  bounds: JointRangeBounds,
  deps?: { patchConfig?: PatchConfigFn; timeoutMs?: number },
): Promise<PersistJointLimitsResult> {
  if (
    !Number.isFinite(bounds.lower) ||
    !Number.isFinite(bounds.upper) ||
    bounds.lower >= bounds.upper
  ) {
    return { ok: false, message: 'Invalid limit bounds.' };
  }

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
    return { ok: false, message: result.message || 'Limits patch failed.' };
  }

  return {
    ok: true,
    lower: bounds.lower,
    upper: bounds.upper,
    restartRequired: result.restart_required,
    message: result.message,
  };
}
