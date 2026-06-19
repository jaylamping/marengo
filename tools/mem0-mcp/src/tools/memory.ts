import type { Mem0Config } from "../config.js";
import {
  findMemoriesByTopicKey,
  formatObservation,
  formatSearchResults,
  getMemory,
  getMemoryHistory,
  searchMemories,
  updateMemory,
  upsertMemoryByTopicKey,
} from "../client.js";
import {
  memGetByTopicKeySchema,
  memGetObservationSchema,
  memSaveSchema,
  memSearchSchema,
  memUpdateSchema,
  validateMemoryContent,
  validateTopicKey,
} from "../schema.js";
import type { z } from "zod";

type ToolEntry = {
  description: string;
  inputSchema: z.ZodTypeAny;
  handler: (args: Record<string, unknown>) => Promise<string>;
};

const ALLOWED_NAMESPACE_HINT =
  "sdd/, feasibility/, research/, expert/, maintenance/, decision/, hardware/, cad/, pi/, control/, or software/";

export function registerMemoryTools(cfg: Mem0Config): Record<string, ToolEntry> {
  return {
    mem_search: {
      description:
        "Search Marengo mem0 memories (Engram-compatible). Returns truncated previews with IDs; call mem_get_observation for full content. Optional project filters results client-side.",
      inputSchema: memSearchSchema,
      handler: async (raw) => {
        const args = memSearchSchema.parse(raw);
        const rows = await searchMemories(
          cfg,
          args.query,
          args.top_k ?? 10,
          args.project,
        );
        return formatSearchResults(rows);
      },
    },
    mem_save: {
      description: `Save or upsert a memory to self-hosted mem0 by topic_key. Requires topic_key under ${ALLOWED_NAMESPACE_HINT}. Same topic_key updates the existing observation.`,
      inputSchema: memSaveSchema,
      handler: async (raw) => {
        const args = memSaveSchema.parse(raw);
        const topicError = validateTopicKey(args.topic_key);
        if (topicError) {
          return `Error: ${topicError}`;
        }
        const contentError = validateMemoryContent(args.content);
        if (contentError) {
          return `Error: ${contentError}`;
        }
        const result = await upsertMemoryByTopicKey(cfg, {
          title: args.title,
          topicKey: args.topic_key,
          type: args.type,
          project: args.project,
          content: args.content,
          capturePrompt: args.capture_prompt,
        });
        const verb = result.action === "updated" ? "updated" : "saved";
        return result.id
          ? `Observation ${verb} (id=${result.id}, topic_key=${args.topic_key})`
          : `Observation ${verb} (topic_key=${args.topic_key})`;
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
    mem_get_by_topic_key: {
      description:
        "Exact lookup by metadata topic_key (preferred for SDD/bootstrap recovery). Returns full content for the newest match.",
      inputSchema: memGetByTopicKeySchema,
      handler: async (raw) => {
        const args = memGetByTopicKeySchema.parse(raw);
        const topicError = validateTopicKey(args.topic_key);
        if (topicError) {
          return `Error: ${topicError}`;
        }
        const rows = await findMemoriesByTopicKey(cfg, args.topic_key, args.project);
        if (rows.length === 0) {
          return `No memory found for topic_key=${args.topic_key}`;
        }
        const row = rows[0];
        if (!row.id) {
          return `No memory found for topic_key=${args.topic_key}`;
        }
        const history = await getMemoryHistory(cfg, row.id);
        return formatObservation(row, history);
      },
    },
    mem_update: {
      description:
        "Update an existing mem0 memory by observation ID. Replaces stored text content and preserves topic_key metadata when present. Prefer mem_save with the same topic_key for upserts.",
      inputSchema: memUpdateSchema,
      handler: async (raw) => {
        const args = memUpdateSchema.parse(raw);
        const contentError = validateMemoryContent(args.content);
        if (contentError) {
          return `Error: ${contentError}`;
        }
        const existing = await getMemory(cfg, args.id);
        const metadata = existing.metadata;
        const row = await updateMemory(cfg, args.id, args.content, metadata);
        return `Observation updated (id=${row.id ?? args.id})`;
      },
    },
  };
}
