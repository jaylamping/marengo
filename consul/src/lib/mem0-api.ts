import {
  mem0BaseUrl,
  mem0UserId,
  normalizeMemory,
  type Mem0HistoryEvent,
  type Mem0Memory,
} from '@/lib/mem0-config';

type Mem0ListResponse = {
  results?: Array<{
    id?: string;
    memory?: string;
    created_at?: string;
    updated_at?: string;
    metadata?: Record<string, unknown>;
  }>;
};

type Mem0SearchResponse = {
  results?: Mem0ListResponse['results'];
};

async function mem0Fetch<T>(path: string, init?: RequestInit): Promise<T | null> {
  try {
    const response = await fetch(`${mem0BaseUrl()}${path}`, {
      ...init,
      headers: {
        'Content-Type': 'application/json',
        ...(init?.headers ?? {}),
      },
    });
    if (!response.ok) {
      return null;
    }
    return (await response.json()) as T;
  } catch {
    return null;
  }
}

export async function fetchMemories(userId = mem0UserId()): Promise<Mem0Memory[]> {
  const payload = await mem0Fetch<Mem0ListResponse>(
    `/memories?user_id=${encodeURIComponent(userId)}`,
  );
  if (!payload?.results) {
    return [];
  }
  return payload.results
    .map((row) => normalizeMemory(row))
    .filter((row): row is Mem0Memory => row !== null)
    .sort((a, b) => {
      const aTs = Date.parse(a.updated_at ?? a.created_at ?? '0');
      const bTs = Date.parse(b.updated_at ?? b.created_at ?? '0');
      return bTs - aTs;
    });
}

export async function searchMemories(
  query: string,
  userId = mem0UserId(),
): Promise<Mem0Memory[]> {
  const payload = await mem0Fetch<Mem0SearchResponse>('/search', {
    method: 'POST',
    body: JSON.stringify({
      query,
      filters: { user_id: userId },
      top_k: 25,
    }),
  });
  if (!payload?.results) {
    return [];
  }
  return payload.results
    .map((row) => normalizeMemory(row))
    .filter((row): row is Mem0Memory => row !== null);
}

export async function fetchMemoryHistory(id: string): Promise<Mem0HistoryEvent[]> {
  const payload = await mem0Fetch<Mem0HistoryEvent[] | { history?: Mem0HistoryEvent[] }>(
    `/memories/${encodeURIComponent(id)}/history`,
  );
  if (!payload) {
    return [];
  }
  if (Array.isArray(payload)) {
    return payload;
  }
  return payload.history ?? [];
}

export async function pingMem0(userId = mem0UserId()): Promise<boolean> {
  const payload = await mem0Fetch<Mem0ListResponse>(
    `/memories?user_id=${encodeURIComponent(userId)}`,
  );
  return payload !== null;
}
