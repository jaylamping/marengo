import { z } from "zod";

/** Allowed top-level mem0 namespaces for Marengo. */
export const TOPIC_KEY_PATTERN =
  /^(sdd|feasibility|research|expert|maintenance)\/[a-z0-9][a-z0-9-]*(\/[a-z0-9][a-z0-9-]*)*$/;

const SECRET_PATTERNS: RegExp[] = [
  /\bm0sk_[A-Za-z0-9_-]{20,}\b/,
  /\bsk-or-v1-[A-Za-z0-9_-]{20,}\b/,
  /\bsk-[A-Za-z0-9]{20,}\b/,
  /\bBearer\s+[A-Za-z0-9._-]{20,}\b/i,
  /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/,
  /\bAWS_SECRET_ACCESS_KEY\s*=\s*\S+/i,
];

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

export function parseNamespace(topicKey: string | undefined): string {
  if (!topicKey) {
    return "other";
  }
  const prefix = topicKey.split("/")[0];
  if (
    prefix === "sdd" ||
    prefix === "feasibility" ||
    prefix === "research" ||
    prefix === "expert" ||
    prefix === "maintenance"
  ) {
    return prefix;
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
  content: z.string().min(1),
  capture_prompt: z.boolean().optional(),
});

export const memGetObservationSchema = z.object({
  id: z.string().min(1),
});

export const memUpdateSchema = z.object({
  id: z.string().min(1),
  content: z.string().min(1),
});

export type MemSearchInput = z.infer<typeof memSearchSchema>;
export type MemSaveInput = z.infer<typeof memSaveSchema>;
export type MemGetObservationInput = z.infer<typeof memGetObservationSchema>;
export type MemUpdateInput = z.infer<typeof memUpdateSchema>;
