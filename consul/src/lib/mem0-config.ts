/** mem0 Memory Observatory — proxied via Vite `/mem0-api` in dev. */

export type Mem0Namespace =
  | 'all'
  | 'sdd'
  | 'feasibility'
  | 'research'
  | 'expert'
  | 'maintenance'
  | 'other';

export type Mem0Memory = {
  id: string;
  memory: string;
  created_at?: string;
  updated_at?: string;
  metadata?: Record<string, unknown>;
  namespace: Mem0Namespace;
  topicKey: string;
};

export type Mem0HistoryEvent = {
  id?: string;
  memory_id?: string;
  old_memory?: string | null;
  new_memory?: string | null;
  event?: string;
  created_at?: string;
  updated_at?: string;
};

const TOPIC_PREFIXES = [
  'sdd',
  'feasibility',
  'research',
  'expert',
  'maintenance',
] as const;

export function parseNamespace(topicKey: string | undefined): Mem0Namespace {
  if (!topicKey) {
    return 'other';
  }
  const prefix = topicKey.split('/')[0];
  if ((TOPIC_PREFIXES as readonly string[]).includes(prefix)) {
    return prefix as Mem0Namespace;
  }
  return 'other';
}

export function topicKeyFromMetadata(metadata?: Record<string, unknown>): string {
  if (!metadata) {
    return '';
  }
  const key = metadata.topic_key ?? metadata.title;
  return typeof key === 'string' ? key : '';
}

export function normalizeMemory(row: {
  id?: string;
  memory?: string;
  created_at?: string;
  updated_at?: string;
  metadata?: Record<string, unknown>;
}): Mem0Memory | null {
  if (!row.id) {
    return null;
  }
  const topicKey = topicKeyFromMetadata(row.metadata);
  return {
    id: row.id,
    memory: row.memory ?? '',
    created_at: row.created_at,
    updated_at: row.updated_at,
    metadata: row.metadata,
    topicKey,
    namespace: parseNamespace(topicKey),
  };
}

export function mem0UserId(): string {
  return (import.meta.env.VITE_MEM0_USER_ID as string | undefined)?.trim() || 'marengo-joey';
}

export function mem0PollMs(): number {
  const raw = import.meta.env.VITE_MEM0_POLL_MS as string | undefined;
  const parsed = raw ? Number.parseInt(raw, 10) : 30_000;
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : 30_000;
}

export function isMem0Configured(): boolean {
  return import.meta.env.DEV;
}

/** Dev proxy path; Pi-hosted builds show offline banner. */
export function mem0BaseUrl(): string {
  return '/mem0-api';
}

export function isMem0Live(): boolean {
  return isMem0Configured();
}

export const MEM0_DASHBOARD_URL = 'https://joey-pc.tail0b414.ts.net';
