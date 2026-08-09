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

export async function fetchCompleteness(): Promise<CompletenessDto | null> {
  const root = baseUrl();
  if (!root) {
    return null;
  }
  try {
    const res = await fetch(`${root}/hardware/completeness`, {
      headers: authHeaders(),
    });
    if (!res.ok) {
      return null;
    }
    return (await res.json()) as CompletenessDto;
  } catch {
    return null;
  }
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
  try {
    const res = await fetch(`${root}/hardware/urdf/activate`, {
      method: 'POST',
      headers: authHeaders('application/json'),
      body: JSON.stringify({ ...body, operator_id: body.operator_id ?? 'consul' }),
      signal: init?.signal,
    });
    const parsed = (await res.json()) as ActivateUrdfResultDto;
    if (!parsed || typeof parsed.ok !== 'boolean') {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
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
