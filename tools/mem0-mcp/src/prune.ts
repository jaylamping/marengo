#!/usr/bin/env node
import { loadConfig } from "./config.js";
import { deleteMemory, listMemories } from "./client.js";
import { parseNamespace } from "./schema.js";

const targetMax = Number(process.env.MEM0_PRUNE_TARGET_MAX ?? "400");
const dryRun = process.argv.includes("--dry-run");

function topicKey(row: { metadata?: Record<string, unknown> }): string {
  const meta = row.metadata ?? {};
  return String(meta.topic_key ?? "");
}

function isProtected(key: string, updatedAt?: string): boolean {
  if (!key) {
    return false;
  }
  if (key.startsWith("expert/")) {
    return true;
  }
  if (key.match(/^feasibility\/[^/]+\/brief$/)) {
    return true;
  }
  if (key.startsWith("sdd/")) {
    if (!updatedAt) {
      return true;
    }
    const ageMs = Date.now() - Date.parse(updatedAt);
    return ageMs < 14 * 24 * 60 * 60 * 1000;
  }
  return false;
}

async function main() {
  const cfg = loadConfig();
  const rows = await listMemories(cfg);
  const deletable = rows.filter((row) => {
    const key = topicKey(row);
    return !isProtected(key, row.updated_at ?? row.created_at);
  });

  const toDelete =
    rows.length > targetMax
      ? deletable.slice(0, Math.max(0, rows.length - targetMax))
      : [];

  console.log(
    JSON.stringify(
      {
        total: rows.length,
        protected: rows.length - deletable.length,
        target_max: targetMax,
        delete_count: toDelete.length,
        dry_run: dryRun,
        namespaces: rows.reduce<Record<string, number>>((acc, row) => {
          const ns = parseNamespace(topicKey(row));
          acc[ns] = (acc[ns] ?? 0) + 1;
          return acc;
        }, {}),
      },
      null,
      2,
    ),
  );

  if (dryRun) {
    return;
  }

  for (const row of toDelete) {
    if (!row.id) {
      continue;
    }
    await deleteMemory(cfg, row.id);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
