import { getChappeEndpoints } from '@/lib/chappe-config';

export type MotorBenchLimitsDto = {
  position_lower_rad: number;
  position_upper_rad: number;
  torque_limit_nm: number;
};

export type MotorConfigEntryDto = {
  joint: string;
  can_interface: string;
  device_id: number;
  direction: number;
  motor_type: string;
  bench: MotorBenchLimitsDto;
};

export type ConfigSnapshotDto = {
  profile: string;
  config_dir: string;
  joints: string[];
  motors: MotorConfigEntryDto[];
  control_limits: {
    joint: string;
    position_soft_lower_rad?: number;
    position_soft_upper_rad?: number;
    velocity_max_rad_s?: number;
  }[];
};

export type ConfigPatchDto = {
  joint: string;
  device_id?: number;
  can_interface?: string;
  direction?: number;
  position_lower_rad?: number;
  position_upper_rad?: number;
  torque_limit_nm?: number;
  position_soft_lower_rad?: number;
  position_soft_upper_rad?: number;
  velocity_max_rad_s?: number;
  operator_id?: string;
};

export type ConfigPatchResultDto = {
  ok: boolean;
  message: string;
  restart_required: boolean;
};

function baseUrl(): string | null {
  return getChappeEndpoints()?.httpUrl ?? null;
}

function authHeaders(): Record<string, string> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };
  const token = import.meta.env.VITE_MARENGO_LOG_TOKEN as string | undefined;
  if (token?.trim()) {
    headers['x-marengo-log-token'] = token.trim();
  }
  return headers;
}

export async function fetchConfigSnapshot(): Promise<ConfigSnapshotDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  const res = await fetch(`${root}/config/snapshot`, { headers: authHeaders() });
  if (!res.ok) {
    return null;
  }
  return (await res.json()) as ConfigSnapshotDto;
}

export async function patchConfig(
  patch: ConfigPatchDto,
  init?: { signal?: AbortSignal },
): Promise<ConfigPatchResultDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(`${root}/config/patch`, {
      method: 'POST',
      headers: authHeaders(),
      body: JSON.stringify(patch),
      signal: init?.signal,
    });
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as ConfigPatchResultDto;
  } catch {
    return null;
  }
}

export function motorForJoint(
  snapshot: ConfigSnapshotDto | null | undefined,
  joint: string,
): MotorConfigEntryDto | undefined {
  return snapshot?.motors.find((m) => m.joint === joint);
}
