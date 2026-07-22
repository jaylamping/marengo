import {
  patchConfig,
  type ConfigPatchDto,
  type ConfigPatchResultDto,
} from '@/lib/config-api';
import { parseJointRange } from '@/lib/limit-listen';

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
) => Promise<ConfigPatchResultDto | null>;

/**
 * Persist Set Limits bounds to motors.yaml via gateway /config/patch.
 * Display-only localStorage overrides are not enough across refresh.
 */
export async function persistJointLimits(
  joint: string,
  range: string,
  deps?: { patchConfig?: PatchConfigFn },
): Promise<PersistJointLimitsResult> {
  const bounds = parseJointRange(range);
  if (!bounds) {
    return { ok: false, message: 'Could not parse proposed range.' };
  }

  const patch = deps?.patchConfig ?? patchConfig;
  const result = await patch({
    joint,
    position_lower_rad: bounds.lower,
    position_upper_rad: bounds.upper,
  });

  if (!result) {
    return {
      ok: false,
      message:
        'Gateway rejected the limits patch (is Chappe up, and is VITE_MARENGO_LOG_TOKEN set for /config/patch?).',
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
