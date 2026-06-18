#!/usr/bin/env node
import { loadConfig } from "./config.js";
import { deleteMemory, listMemories } from "./client.js";
import {
  isProtected,
  namespaceCounts,
  sortDeletableForPrune,
  topicKeyFromRow,
} from "./prune-policy.js";

const targetMax = Number(process.env.MEM0_PRUNE_TARGET_MAX ?? "400");
const dryRun = process.argv.includes("--dry-run");

async function main() {
  const cfg = loadConfig();
  const rows = await listMemories(cfg);
  const deletable = sortDeletableForPrune(
    rows.filter((row) => {
      const key = topicKeyFromRow(row);
      return !isProtected(key, row.updated_at ?? row.created_at);
    }),
  );

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
        namespaces: namespaceCounts(rows),
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
