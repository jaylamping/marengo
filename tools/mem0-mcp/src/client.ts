import type { Mem0Config } from "./config.js";

export type Mem0MemoryRow = {
  id?: string;
  memory?: string;
  hash?: string;
  created_at?: string;
  updated_at?: string;
  user_id?: string;
  metadata?: Record<string, unknown>;
  score?: number;
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

async function mem0Fetch<T>(
  cfg: Mem0Config,
  path: string,
  init?: RequestInit,
): Promise<T> {
  const url = `${cfg.apiUrl}${path.startsWith("/") ? path : `/${path}`}`;
  const response = await fetch(url, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      "X-API-Key": cfg.apiKey,
      ...(init?.headers ?? {}),
    },
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(`mem0 ${response.status}: ${body.slice(0, 500)}`);
  }

  return (await response.json()) as T;
}

function matchesProject(row: Mem0MemoryRow, project?: string): boolean {
  if (!project) {
    return true;
  }
  const meta = row.metadata ?? {};
  return meta.project === project;
}

function topicKeyFromRow(row: Mem0MemoryRow): string {
  const meta = row.metadata ?? {};
  return String(meta.topic_key ?? "");
}

function sortByRecency(rows: Mem0MemoryRow[]): Mem0MemoryRow[] {
  return [...rows].sort((a, b) => {
    const aTime = Date.parse(a.updated_at ?? a.created_at ?? "0");
    const bTime = Date.parse(b.updated_at ?? b.created_at ?? "0");
    return bTime - aTime;
  });
}

/** mem0 OSS defaults GET /memories to top_k=20; SDD stores exceed that quickly. */
export const LIST_MEMORIES_TOP_K = 10_000;

export function buildSaveMetadata(args: {
  title: string;
  topicKey: string;
  type?: string;
  project?: string;
  capturePrompt?: boolean;
}): Record<string, unknown> {
  const metadata: Record<string, unknown> = {
    topic_key: args.topicKey,
    title: args.title,
  };
  if (args.type) {
    metadata.type = args.type;
  }
  if (args.project) {
    metadata.project = args.project;
  }
  if (args.capturePrompt !== undefined) {
    metadata.capture_prompt = args.capturePrompt;
  }
  return metadata;
}

export function mergeSaveMetadata(
  existing: Record<string, unknown> | undefined,
  args: {
    title: string;
    topicKey: string;
    type?: string;
    project?: string;
    capturePrompt?: boolean;
  },
): Record<string, unknown> {
  return {
    ...(existing ?? {}),
    ...buildSaveMetadata(args),
  };
}

export async function searchMemories(
  cfg: Mem0Config,
  query: string,
  topK = 10,
  project?: string,
): Promise<Mem0MemoryRow[]> {
  const payload = await mem0Fetch<{ results?: Mem0MemoryRow[] }>(cfg, "/search", {
    method: "POST",
    body: JSON.stringify({
      query,
      filters: { user_id: cfg.userId },
      top_k: topK,
    }),
  });
  let rows = payload.results ?? [];
  if (project) {
    rows = rows.filter((row) => matchesProject(row, project));
  }
  return rows;
}

export async function listMemories(cfg: Mem0Config): Promise<Mem0MemoryRow[]> {
  const payload = await mem0Fetch<{ results?: Mem0MemoryRow[] }>(
    cfg,
    `/memories?user_id=${encodeURIComponent(cfg.userId)}&top_k=${LIST_MEMORIES_TOP_K}`,
  );
  return payload.results ?? [];
}

export function filterRowsByTopicKey(
  rows: Mem0MemoryRow[],
  topicKey: string,
  project?: string,
): Mem0MemoryRow[] {
  return rows.filter((row) => {
    if (topicKeyFromRow(row) !== topicKey) {
      return false;
    }
    return matchesProject(row, project);
  });
}

export async function findMemoriesByTopicKey(
  cfg: Mem0Config,
  topicKey: string,
  project?: string,
): Promise<Mem0MemoryRow[]> {
  const fromList = filterRowsByTopicKey(await listMemories(cfg), topicKey, project);
  if (fromList.length > 0) {
    return sortByRecency(fromList);
  }

  const fromSearch = filterRowsByTopicKey(
    await searchMemories(cfg, topicKey, 50, project),
    topicKey,
    project,
  );
  return sortByRecency(fromSearch);
}

export async function getMemory(
  cfg: Mem0Config,
  id: string,
): Promise<Mem0MemoryRow> {
  return mem0Fetch<Mem0MemoryRow>(cfg, `/memories/${encodeURIComponent(id)}`);
}

export async function getMemoryHistory(
  cfg: Mem0Config,
  id: string,
): Promise<Mem0HistoryEvent[]> {
  const payload = await mem0Fetch<Mem0HistoryEvent[] | { history?: Mem0HistoryEvent[] }>(
    cfg,
    `/memories/${encodeURIComponent(id)}/history`,
  );
  if (Array.isArray(payload)) {
    return payload;
  }
  return payload.history ?? [];
}

export async function saveMemory(
  cfg: Mem0Config,
  args: {
    title: string;
    topicKey: string;
    type?: string;
    project?: string;
    content: string;
    capturePrompt?: boolean;
  },
): Promise<{ id?: string; message?: string; results?: unknown[] }> {
  const metadata = buildSaveMetadata(args);

  return mem0Fetch(cfg, "/memories", {
    method: "POST",
    body: JSON.stringify({
      messages: [{ role: "user", content: args.content }],
      user_id: cfg.userId,
      metadata,
      infer: false,
    }),
  });
}

export async function upsertMemoryByTopicKey(
  cfg: Mem0Config,
  args: {
    title: string;
    topicKey: string;
    type?: string;
    project?: string;
    content: string;
    capturePrompt?: boolean;
  },
): Promise<{ id?: string; action: "created" | "updated" }> {
  const existing = await findMemoriesByTopicKey(cfg, args.topicKey, args.project);
  if (existing.length > 0 && existing[0].id) {
    const primaryId = existing[0].id;
    const existingMeta = existing[0].metadata;
    await updateMemory(
      cfg,
      primaryId,
      args.content,
      mergeSaveMetadata(existingMeta, args),
    );
    for (let i = 1; i < existing.length; i++) {
      const duplicateId = existing[i].id;
      if (duplicateId) {
        await deleteMemory(cfg, duplicateId);
      }
    }
    return { id: primaryId, action: "updated" };
  }

  const result = await saveMemory(cfg, args);
  const id =
    (result.results as { id?: string }[] | undefined)?.[0]?.id ??
    (result as { id?: string }).id;
  return { id, action: "created" };
}

export async function deleteMemory(cfg: Mem0Config, id: string): Promise<void> {
  await mem0Fetch(cfg, `/memories/${encodeURIComponent(id)}`, { method: "DELETE" });
}

export async function updateMemory(
  cfg: Mem0Config,
  id: string,
  content: string,
  metadata?: Record<string, unknown>,
): Promise<Mem0MemoryRow> {
  const body: Record<string, unknown> = { text: content };
  if (metadata !== undefined) {
    body.metadata = metadata;
  }
  return mem0Fetch<Mem0MemoryRow>(cfg, `/memories/${encodeURIComponent(id)}`, {
    method: "PUT",
    body: JSON.stringify(body),
  });
}

export function formatObservation(row: Mem0MemoryRow, history?: Mem0HistoryEvent[]): string {
  const meta = row.metadata ?? {};
  const topicKey = meta.topic_key ?? meta.title ?? row.id ?? "unknown";
  const lines = [
    `# Observation ${row.id ?? "?"}`,
    `topic_key: ${String(topicKey)}`,
    `project: ${String(meta.project ?? "?")}`,
    `created_at: ${row.created_at ?? "?"}`,
    `updated_at: ${row.updated_at ?? "?"}`,
    "",
    row.memory ?? "",
  ];
  if (history && history.length > 0) {
    lines.push("", "## History");
    for (const event of history) {
      lines.push(
        `- ${event.event ?? "UPDATE"} @ ${event.created_at ?? "?"}: ${(event.new_memory ?? "").slice(0, 120)}`,
      );
    }
  }
  return lines.join("\n");
}

export function formatSearchResults(rows: Mem0MemoryRow[]): string {
  if (rows.length === 0) {
    return "No memories found.";
  }
  return rows
    .map((row, index) => {
      const meta = row.metadata ?? {};
      const topicKey = meta.topic_key ?? meta.title ?? row.id;
      const preview = (row.memory ?? "").slice(0, 240).replace(/\n/g, " ");
      const score =
        row.score !== undefined ? ` score=${row.score.toFixed(3)}` : "";
      return `${index + 1}. id=${row.id} topic_key=${String(topicKey)}${score}\n   ${preview}`;
    })
    .join("\n\n");
}
