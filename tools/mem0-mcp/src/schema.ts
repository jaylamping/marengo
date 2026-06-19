import { z } from "zod";

/** Allowed top-level mem0 namespaces for Marengo. */
export const TOPIC_PREFIXES = [
  "sdd",
  "feasibility",
  "research",
  "expert",
  "maintenance",
  "decision",
  "hardware",
  "cad",
  "pi",
  "control",
  "software",
] as const;

export type TopicPrefix = (typeof TOPIC_PREFIXES)[number];

export const TOPIC_KEY_PATTERN = new RegExp(
  `^(${TOPIC_PREFIXES.join("|")})\/[a-z0-9][a-z0-9-]*(\/[a-z0-9][a-z0-9-]*)*$`,
);

export const MAX_CONTENT_LENGTH = 32_000;

const SECRET_PATTERNS: RegExp[] = [
  /\bm0sk_[A-Za-z0-9_-]{20,}\b/,
  /\bsk-or-v1-[A-Za-z0-9_-]{20,}\b/,
  /\bsk-[A-Za-z0-9]{20,}\b/,
  /\bBearer\s+[A-Za-z0-9._-]{20,}\b/i,
  /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/,
  /\bAWS_SECRET_ACCESS_KEY\s*=\s*\S+/i,
];

/** Line-level patterns for pasted log dumps — not tool names or distilled references. */
const RAW_LOG_LINE_PATTERNS: RegExp[] = [
  // candump: (0.001234) can0 701#AABBCCDD
  /^\s*\([0-9.]+\)\s+\S+\s+[0-9A-Fa-f]+#/m,
  // kernel dmesg: 123.456789 #1 [12345] abc [67890]
  /^\s*\d+\.\d+\s+#\d+\s+\[\d+\]\s+[0-9a-f]{3,}\s+\[\d+\]/im,
  // journalctl classic: Jun 19 02:48:05 hostname unit[1234]: message
  /^[A-Z][a-z]{2}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}\s+\S+\s+\S+\[\d+\]:/m,
];

/** Minimum matching log-like lines before rejecting (avoids one-off examples). */
const RAW_LOG_LINE_THRESHOLD = 3;

export function validateTopicKey(topicKey: string): string | null {
  if (!TOPIC_KEY_PATTERN.test(topicKey)) {
    return `topic_key must match ${TOPIC_KEY_PATTERN.source} (e.g. sdd/my-change/explore)`;
  }
  return null;
}

export function rejectSecrets(content: string): string | null {
  for (const pattern of SECRET_PATTERNS) {
    if (pattern.test(content)) {
      return "content appears to contain a secret or API key; refuse to save";
    }
  }
  return null;
}

export function rejectOversizedContent(content: string): string | null {
  if (content.length > MAX_CONTENT_LENGTH) {
    return `content exceeds max length of ${MAX_CONTENT_LENGTH} characters`;
  }
  return null;
}

export function countRawLogLines(content: string): number {
  const lines = content.split(/\r?\n/);
  let hits = 0;
  for (const line of lines) {
    if (RAW_LOG_LINE_PATTERNS.some((pattern) => pattern.test(line))) {
      hits += 1;
    }
  }
  return hits;
}

export function rejectRawLogs(content: string): string | null {
  if (countRawLogLines(content) >= RAW_LOG_LINE_THRESHOLD) {
    return "content appears to be raw log output; distill before saving";
  }
  return null;
}

/** Coerce Anthropic-style content blocks to a plain string for mem_save/mem_update. */
export function normalizeContentInput(value: unknown): unknown {
  if (typeof value === "string") {
    return value;
  }
  if (Array.isArray(value)) {
    return value
      .map((block) => {
        if (typeof block === "string") {
          return block;
        }
        if (block && typeof block === "object" && "text" in block) {
          return String((block as { text: unknown }).text);
        }
        return "";
      })
      .join("");
  }
  if (value && typeof value === "object" && "text" in value) {
    return String((value as { text: unknown }).text);
  }
  return value;
}

const contentFieldSchema = z.preprocess(
  normalizeContentInput,
  z
    .string({
      invalid_type_error:
        "content must be a plain string (not a [{text,type}] content block array)",
    })
    .min(1),
);

export function validateMemoryContent(content: string): string | null {
  return (
    rejectSecrets(content) ??
    rejectOversizedContent(content) ??
    rejectRawLogs(content)
  );
}

export function parseNamespace(topicKey: string | undefined): TopicPrefix | "other" {
  if (!topicKey) {
    return "other";
  }
  const prefix = topicKey.split("/")[0];
  if ((TOPIC_PREFIXES as readonly string[]).includes(prefix)) {
    return prefix as TopicPrefix;
  }
  return "other";
}

export const memSearchSchema = z.object({
  query: z.string().min(1),
  project: z.string().optional(),
  top_k: z.number().int().min(1).max(50).optional(),
});

export const memSaveSchema = z.object({
  title: z.string().min(1),
  topic_key: z.string().min(1),
  type: z.string().optional(),
  project: z.string().optional(),
  content: contentFieldSchema,
  capture_prompt: z.boolean().optional(),
});

export const memGetObservationSchema = z.object({
  id: z.string().min(1),
});

export const memGetByTopicKeySchema = z
  .object({
    topic_key: z.string().min(1).optional(),
    /** Legacy alias — agents often pass `query` copied from mem_search examples. */
    query: z.string().min(1).optional(),
    project: z.string().optional(),
  })
  .refine((value) => Boolean(value.topic_key ?? value.query), {
    message:
      "topic_key is required (exact key like sdd/my-change/tasks). Do not use mem_search's query param name here.",
  })
  .transform((value) => ({
    topic_key: (value.topic_key ?? value.query)!,
    project: value.project,
  }));

export const memUpdateSchema = z.object({
  id: z.string().min(1),
  content: contentFieldSchema,
});

export type MemSearchInput = z.infer<typeof memSearchSchema>;
export type MemSaveInput = z.infer<typeof memSaveSchema>;
export type MemGetObservationInput = z.infer<typeof memGetObservationSchema>;
export type MemGetByTopicKeyInput = z.infer<typeof memGetByTopicKeySchema>;
export type MemUpdateInput = z.infer<typeof memUpdateSchema>;

/** Tool names exposed by registerMemoryTools — keep in sync with tests and docs. */
export const MEMORY_TOOL_NAMES = [
  "mem_search",
  "mem_save",
  "mem_get_observation",
  "mem_get_by_topic_key",
  "mem_update",
] as const;
