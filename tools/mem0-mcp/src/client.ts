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

export async function searchMemories(
  cfg: Mem0Config,
  query: string,
  topK = 10,
): Promise<Mem0MemoryRow[]> {
  const payload = await mem0Fetch<{ results?: Mem0MemoryRow[] }>(cfg, "/search", {
    method: "POST",
    body: JSON.stringify({
      query,
      filters: { user_id: cfg.userId },
      top_k: topK,
    }),
  });
  return payload.results ?? [];
}

export async function listMemories(cfg: Mem0Config): Promise<Mem0MemoryRow[]> {
  const payload = await mem0Fetch<{ results?: Mem0MemoryRow[] }>(
    cfg,
    `/memories?user_id=${encodeURIComponent(cfg.userId)}`,
  );
  return payload.results ?? [];
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
  },
): Promise<{ id?: string; message?: string; results?: unknown[] }> {
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

export async function deleteMemory(cfg: Mem0Config, id: string): Promise<void> {
  await mem0Fetch(cfg, `/memories/${encodeURIComponent(id)}`, { method: "DELETE" });
}

export function formatObservation(row: Mem0MemoryRow, history?: Mem0HistoryEvent[]): string {
  const meta = row.metadata ?? {};
  const topicKey = meta.topic_key ?? meta.title ?? row.id ?? "unknown";
  const lines = [
    `# Observation ${row.id ?? "?"}`,
    `topic_key: ${String(topicKey)}`,
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
