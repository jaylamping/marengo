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
  revision?: string;
  persist_ok?: boolean;
};

export type ProfilesDto = {
  active_slug: string;
  profiles: { slug: string; revision: string }[];
  presets: { preset_id: string; profile_slug: string }[];
};

export type ApplyActuatorDto = {
  target_profile: string;
  expected_revision?: string;
  operator_id: string;
  op: 'upsert_limits' | 'add_joint' | 'preview';
  joint: string;
  position_lower_rad?: number;
  position_upper_rad?: number;
  torque_limit_nm?: number;
  position_soft_lower_rad?: number;
  position_soft_upper_rad?: number;
  velocity_max_rad_s?: number;
};

export type ApplyActuatorResultDto = {
  ok: boolean;
  message: string;
  applied_live: boolean;
  restart_required: boolean;
  revision?: string | null;
  persist_status: 'durable' | 'pending' | 'failed' | 'n/a';
  decision?:
    | 'add'
    | 'overwrite'
    | 'noop'
    | 'unmapped_preset'
    | 'unsupported_membership'
    | null;
  before?: {
    joint: string;
    position_lower_rad: number;
    position_upper_rad: number;
  } | null;
  after?: {
    joint: string;
    position_lower_rad: number;
    position_upper_rad: number;
  } | null;
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

export type RestartMarengoPiResultDto = {
  ok: boolean;
  message: string;
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

export async function fetchProfiles(): Promise<ProfilesDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(`${root}/config/profiles`, { headers: authHeaders() });
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as ProfilesDto;
  } catch {
    return null;
  }
}

export async function applyActuatorConfig(
  body: ApplyActuatorDto,
  init?: { signal?: AbortSignal },
): Promise<ApplyActuatorResultDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(`${root}/config/actuators/apply`, {
      method: 'POST',
      headers: authHeaders(),
      body: JSON.stringify(body),
      signal: init?.signal,
    });
    const text = await res.text();
    let parsed: ApplyActuatorResultDto | null = null;
    try {
      parsed = JSON.parse(text) as ApplyActuatorResultDto;
    } catch {
      parsed = null;
    }
    if (parsed && typeof parsed.ok === 'boolean') {
      return parsed;
    }
    return res.ok
      ? null
      : {
          ok: false,
          message: text.trim() || `HTTP ${res.status}`,
          applied_live: false,
          restart_required: false,
          persist_status: 'failed',
        };
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

/** Restart marengo-pi so Davout reloads hard limits from motors.yaml. */
export async function restartMarengoPi(init?: {
  signal?: AbortSignal;
}): Promise<RestartMarengoPiResultDto> {
  const root = baseUrl();
  if (!root) {
    return { ok: false, message: 'Chappe HTTP URL not configured' };
  }
  try {
    const res = await fetch(`${root}/control/restart-marengo-pi`, {
      method: 'POST',
      headers: authHeaders(),
      body: JSON.stringify({ confirm: true }),
      signal: init?.signal,
    });
    const text = await res.text();
    let parsed: RestartMarengoPiResultDto | null = null;
    try {
      parsed = JSON.parse(text) as RestartMarengoPiResultDto;
    } catch {
      parsed = null;
    }
    if (parsed && typeof parsed.ok === 'boolean') {
      return {
        ok: parsed.ok && res.ok,
        message: parsed.message || res.statusText || `HTTP ${res.status}`,
      };
    }
    return {
      ok: false,
      message: text.trim() || `HTTP ${res.status}`,
    };
  } catch (e) {
    return {
      ok: false,
      message: e instanceof Error ? e.message : 'Restart request failed',
    };
  }
}
