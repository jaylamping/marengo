import type { Mem0Config } from "../config.js";
import {
  formatObservation,
  formatSearchResults,
  getMemory,
  getMemoryHistory,
  saveMemory,
  searchMemories,
  updateMemory,
} from "../client.js";
import {
  memGetObservationSchema,
  memSaveSchema,
  memSearchSchema,
  memUpdateSchema,
  rejectSecrets,
  validateTopicKey,
} from "../schema.js";
import type { z } from "zod";

type ToolEntry = {
  description: string;
  inputSchema: z.ZodTypeAny;
  handler: (args: Record<string, unknown>) => Promise<string>;
};

export function registerMemoryTools(cfg: Mem0Config): Record<string, ToolEntry> {
  return {
    mem_search: {
      description:
        "Search Marengo mem0 memories (Engram-compatible). Returns truncated previews with IDs; call mem_get_observation for full content.",
      inputSchema: memSearchSchema,
      handler: async (raw) => {
        const args = memSearchSchema.parse(raw);
        const rows = await searchMemories(cfg, args.query, args.top_k ?? 10);
        return formatSearchResults(rows);
      },
    },
    mem_save: {
      description:
        "Save a memory to self-hosted mem0. Requires topic_key under sdd/, feasibility/, research/, expert/, or maintenance/.",
      inputSchema: memSaveSchema,
      handler: async (raw) => {
        const args = memSaveSchema.parse(raw);
        const topicError = validateTopicKey(args.topic_key);
        if (topicError) {
          return `Error: ${topicError}`;
        }
        const secretError = rejectSecrets(args.content);
        if (secretError) {
          return `Error: ${secretError}`;
        }
        const result = await saveMemory(cfg, {
          title: args.title,
          topicKey: args.topic_key,
          type: args.type,
          project: args.project,
          content: args.content,
        });
        const id =
          (result.results as { id?: string }[] | undefined)?.[0]?.id ??
          (result as { id?: string }).id;
        return id
          ? `Observation saved (id=${id}, topic_key=${args.topic_key})`
          : `Observation saved (topic_key=${args.topic_key})`;
      },
    },
    mem_get_observation: {
      description:
        "Retrieve full mem0 memory content and revision history by observation ID.",
      inputSchema: memGetObservationSchema,
      handler: async (raw) => {
        const args = memGetObservationSchema.parse(raw);
        const row = await getMemory(cfg, args.id);
        const history = await getMemoryHistory(cfg, args.id);
        return formatObservation(row, history);
      },
    },
    mem_update: {
      description:
        "Update an existing mem0 memory by observation ID. Replaces stored text content.",
      inputSchema: memUpdateSchema,
      handler: async (raw) => {
        const args = memUpdateSchema.parse(raw);
        const secretError = rejectSecrets(args.content);
        if (secretError) {
          return `Error: ${secretError}`;
        }
        const row = await updateMemory(cfg, args.id, args.content);
        return `Observation updated (id=${row.id ?? args.id})`;
      },
    },
  };
}
