import { parseNamespace } from "./schema.js";

export function topicKeyFromRow(row: {
  metadata?: Record<string, unknown>;
}): string {
  const meta = row.metadata ?? {};
  return String(meta.topic_key ?? "");
}

function ageDays(updatedAt?: string): number | null {
  if (!updatedAt) {
    return null;
  }
  const parsed = Date.parse(updatedAt);
  if (Number.isNaN(parsed)) {
    return null;
  }
  return (Date.now() - parsed) / (24 * 60 * 60 * 1000);
}

function isRecent(key: string, updatedAt: string | undefined, maxDays: number): boolean {
  const age = ageDays(updatedAt);
  if (age === null) {
    return true;
  }
  return age < maxDays;
}

export function isProtected(key: string, updatedAt?: string): boolean {
  if (!key) {
    return false;
  }

  if (key.startsWith("expert/")) {
    return true;
  }
  if (key.match(/^feasibility\/[^/]+\/brief$/)) {
    return true;
  }
  if (key === "maintenance/skill-registry") {
    return true;
  }
  if (key.startsWith("maintenance/session-handoff/")) {
    return true;
  }
  if (key.startsWith("research/")) {
    return true;
  }
  if (key.startsWith("sdd/")) {
    return isRecent(key, updatedAt, 14);
  }

  const recentSubsystemPrefixes = [
    "decision/",
    "hardware/",
    "cad/",
    "pi/",
    "control/",
    "software/",
  ] as const;
  for (const prefix of recentSubsystemPrefixes) {
    if (key.startsWith(prefix)) {
      return isRecent(key, updatedAt, 30);
    }
  }

  if (key.match(/^maintenance\/prune\//)) {
    return isRecent(key, updatedAt, 30);
  }

  return false;
}

export function sortDeletableForPrune<
  T extends { updated_at?: string; created_at?: string },
>(rows: T[]): T[] {
  return [...rows].sort((a, b) => {
    const aTime = Date.parse(a.updated_at ?? a.created_at ?? "0");
    const bTime = Date.parse(b.updated_at ?? b.created_at ?? "0");
    return aTime - bTime;
  });
}

export function namespaceCounts(rows: { metadata?: Record<string, unknown> }[]): Record<string, number> {
  return rows.reduce<Record<string, number>>((acc, row) => {
    const ns = parseNamespace(topicKeyFromRow(row));
    acc[ns] = (acc[ns] ?? 0) + 1;
    return acc;
  }, {});
}
