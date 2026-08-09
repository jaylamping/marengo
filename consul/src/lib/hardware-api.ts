import { getChappeEndpoints } from '@/lib/chappe-config';

export type CompletenessWarningDto = {
  code: string;
  severity: string;
  joint?: string;
  link?: string;
  message: string;
};

export type CompletenessDto = {
  warnings: CompletenessWarningDto[];
};

export type FieldDiffDto = {
  joint: string;
  field: string;
  master_value: string;
  contributor_value: string;
  kinematics_critical: boolean;
};

export type MergePreviewDto = {
  overlapping_joints: string[];
  new_joints: string[];
  field_diffs: FieldDiffDto[];
};

export type ResolutionChoice = 'master' | 'contributor';

export type FieldResolutionDto = {
  joint: string;
  field: string;
  choice: ResolutionChoice;
};

export type UrdfUploadResultDto = {
  ok: boolean;
  upload_id: string;
  preview: MergePreviewDto;
};

export type ResolvePreviewResultDto = {
  ok: boolean;
  preview: MergePreviewDto;
  unresolved_critical: string[];
  merged_preview_available: boolean;
};

export type ActivateUrdfResultDto = {
  ok: boolean;
  message: string;
  checksum_sha256: string;
  completeness: CompletenessDto;
  restart_required?: boolean;
};

export type ArchiveEntryDto = {
  upload_id: string;
  archived_at?: string | null;
  source?: string | null;
  checksum_sha256?: string | null;
  contributor_checksum_sha256?: string | null;
  replaced_active_checksum_sha256?: string | null;
};

export type ArchiveListDto = {
  entries: ArchiveEntryDto[];
};

export type ArchiveFetchDto = {
  upload_id: string;
  manifest: Record<string, unknown>;
  contributor_urdf: string;
  replaced_active_urdf?: string | null;
};

function baseUrl(): string | null {
  return getChappeEndpoints()?.httpUrl ?? null;
}

function authHeaders(contentType?: string): Record<string, string> {
  const headers: Record<string, string> = {};
  if (contentType) {
    headers['Content-Type'] = contentType;
  }
  const token = import.meta.env.VITE_MARENGO_LOG_TOKEN as string | undefined;
  if (token?.trim()) {
    headers['x-marengo-log-token'] = token.trim();
  }
  return headers;
}

export async function fetchCompleteness(): Promise<CompletenessDto> {
  const root = baseUrl();
  if (!root) {
    throw new Error('Chappe endpoints not configured');
  }
  const res = await fetch(`${root}/hardware/completeness`, {
    headers: authHeaders(),
  });
  if (!res.ok) {
    throw new Error(`Hardware completeness failed: HTTP ${res.status}`);
  }
  const parsed: unknown = await res.json();
  if (!isCompleteness(parsed)) {
    throw new Error('Hardware completeness response was invalid');
  }
  return parsed;
}

export async function fetchLiveUrdf(): Promise<{
  bytes: ArrayBuffer;
  checksum: string | null;
} | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(`${root}/hardware/urdf`, { headers: authHeaders() });
    if (!res.ok) {
      return null;
    }
    const checksum = res.headers.get('x-urdf-checksum-sha256');
    const bytes = await res.arrayBuffer();
    return { bytes, checksum };
  } catch {
    return null;
  }
}

export async function uploadUrdf(
  urdfXml: string,
  init?: { signal?: AbortSignal },
): Promise<UrdfUploadResultDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(`${root}/hardware/urdf/upload`, {
      method: 'POST',
      headers: authHeaders('application/xml'),
      body: urdfXml,
      signal: init?.signal,
    });
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as UrdfUploadResultDto;
  } catch {
    return null;
  }
}

export async function resolveUrdfPreview(
  body: { upload_id: string; resolutions: FieldResolutionDto[] },
  init?: { signal?: AbortSignal },
): Promise<ResolvePreviewResultDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(`${root}/hardware/urdf/resolve-preview`, {
      method: 'POST',
      headers: authHeaders('application/json'),
      body: JSON.stringify(body),
      signal: init?.signal,
    });
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as ResolvePreviewResultDto;
  } catch {
    return null;
  }
}

export async function activateUrdf(
  body: {
    upload_id: string;
    resolutions: FieldResolutionDto[];
    operator_id?: string;
  },
  init?: { signal?: AbortSignal },
): Promise<ActivateUrdfResultDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  let res: Response;
  try {
    res = await fetch(`${root}/hardware/urdf/activate`, {
      method: 'POST',
      headers: authHeaders('application/json'),
      body: JSON.stringify({ ...body, operator_id: body.operator_id ?? 'consul' }),
      signal: init?.signal,
    });
  } catch {
    return null;
  }
  const parsed: unknown = await res.json().catch(() => null);
  if (res.status === 409) {
    throw new Error(
      responseMessage(parsed) ??
        'Activate refused — robot is ACTIVE or the hardware state changed.',
    );
  }
  if (!isActivateUrdfResult(parsed)) {
    return null;
  }
  return parsed;
}

function responseMessage(body: unknown): string | null {
  if (typeof body !== 'object' || body === null) {
    return null;
  }
  if ('message' in body && typeof body.message === 'string' && body.message.trim()) {
    return body.message;
  }
  if ('error' in body && typeof body.error === 'string' && body.error.trim()) {
    return body.error;
  }
  return null;
}

function isActivateUrdfResult(value: unknown): value is ActivateUrdfResultDto {
  return (
    typeof value === 'object' &&
    value !== null &&
    'ok' in value &&
    typeof value.ok === 'boolean' &&
    'message' in value &&
    typeof value.message === 'string' &&
    'checksum_sha256' in value &&
    typeof value.checksum_sha256 === 'string' &&
    'completeness' in value &&
    isCompleteness(value.completeness) &&
    (!('restart_required' in value) || typeof value.restart_required === 'boolean')
  );
}

function isCompleteness(value: unknown): value is CompletenessDto {
  return (
    typeof value === 'object' &&
    value !== null &&
    'warnings' in value &&
    Array.isArray(value.warnings) &&
    value.warnings.every(isCompletenessWarning)
  );
}

function isCompletenessWarning(value: unknown): value is CompletenessWarningDto {
  return (
    typeof value === 'object' &&
    value !== null &&
    'code' in value &&
    typeof value.code === 'string' &&
    'severity' in value &&
    typeof value.severity === 'string' &&
    'message' in value &&
    typeof value.message === 'string' &&
    (!('joint' in value) || value.joint === undefined || typeof value.joint === 'string') &&
    (!('link' in value) || value.link === undefined || typeof value.link === 'string')
  );
}

export async function fetchUrdfArchiveList(): Promise<ArchiveListDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(`${root}/hardware/urdf/archive`, { headers: authHeaders() });
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as ArchiveListDto;
  } catch {
    return null;
  }
}

export async function fetchUrdfArchive(
  uploadId: string,
): Promise<ArchiveFetchDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(`${root}/hardware/urdf/archive/${encodeURIComponent(uploadId)}`, {
      headers: authHeaders(),
    });
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as ArchiveFetchDto;
  } catch {
    return null;
  }
}

export async function restoreUrdfArchive(
  uploadId: string,
  init?: { signal?: AbortSignal },
): Promise<UrdfUploadResultDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(
      `${root}/hardware/urdf/archive/${encodeURIComponent(uploadId)}/restore`,
      {
        method: 'POST',
        headers: authHeaders('application/json'),
        body: '{}',
        signal: init?.signal,
      },
    );
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as UrdfUploadResultDto;
  } catch {
    return null;
  }
}
