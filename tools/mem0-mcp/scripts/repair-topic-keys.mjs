/**
 * One-shot repair for mem0 rows missing metadata.topic_key (usually after text-only PUT).
 * Usage: node scripts/repair-topic-keys.mjs [--dry-run]
 */
import { loadConfig } from "../dist/config.js";
import {
  deleteMemory,
  getMemory,
  listMemories,
  searchMemories,
  updateMemory,
} from "../dist/client.js";
import { TOPIC_KEY_PATTERN, validateTopicKey } from "../dist/schema.js";
import { buildSaveMetadata } from "../dist/client.js";

const dryRun = process.argv.includes("--dry-run");

function topicKeyFromRow(row) {
  return String(row.metadata?.topic_key ?? "");
}

function slugFromContent(content) {
  if (/consul-liquid-glass-ui/i.test(content)) {
    return "consul-liquid-glass-ui";
  }
  const titleMatch = content.match(
    /^#\s*(?:Tasks|Apply Progress|Design|SDD state):\s*(.+)$/im,
  );
  if (titleMatch) {
    return titleMatch[1]
      .trim()
      .toLowerCase()
      .replace(/\s*\(.+\)\s*$/, "")
      .replace(/\s+/g, "-")
      .replace(/[^a-z0-9-]/g, "");
  }
  return null;
}

function inferTopicKey(content) {
  const change = slugFromContent(content);

  if (/^#\s*Tasks:/im.test(content) && change) {
    return `sdd/${change}/tasks`;
  }
  if (/^#\s*Apply Progress:/im.test(content) && change) {
    return `sdd/${change}/apply-progress`;
  }
  if (/^#\s*Design:/im.test(content) && change) {
    return `sdd/${change}/design`;
  }
  if (/^#\s*SDD state:/im.test(content) && change) {
    return `sdd/${change}/state`;
  }
  if (/Consul Liquid Glass UI Specification/i.test(content)) {
    return "sdd/consul-liquid-glass-ui/spec";
  }
  if (/graphify pilot/i.test(content)) {
    return "decision/software/graphify-pilot";
  }

  const embedded = content.match(
    /\b(sdd|feasibility|research|expert|maintenance|decision|hardware|cad|pi|control|software)\/[a-z0-9][a-z0-9-]*(?:\/[a-z0-9][a-z0-9-]*)*\b/,
  );
  if (embedded && TOPIC_KEY_PATTERN.test(embedded[0])) {
    return embedded[0];
  }

  return null;
}

function rowTime(row) {
  return Date.parse(row.updated_at ?? row.created_at ?? "0");
}

async function collectAllRows(cfg) {
  const byId = new Map();
  for (const row of await listMemories(cfg)) {
    if (row.id) {
      byId.set(row.id, row);
    }
  }
  for (const row of await searchMemories(cfg, "sdd maintenance decision expert research", 50)) {
    if (row.id && !byId.has(row.id)) {
      byId.set(row.id, row);
    }
  }
  return [...byId.values()];
}

async function main() {
  const cfg = loadConfig();
  const rows = await collectAllRows(cfg);

  const validByKey = new Map();
  const corrupted = [];

  for (const row of rows) {
    if (!row.id) {
      continue;
    }
    const key = topicKeyFromRow(row);
    if (key && TOPIC_KEY_PATTERN.test(key) && key !== row.id) {
      const existing = validByKey.get(key);
      if (!existing || rowTime(row) > rowTime(existing)) {
        validByKey.set(key, row);
      }
      continue;
    }
    corrupted.push(row);
  }

  console.log(`Scanned ${rows.length} rows; ${corrupted.length} corrupted.`);

  const actions = [];

  for (const row of corrupted) {
    const full = row.memory ? row : await getMemory(cfg, row.id);
    const inferred = inferTopicKey(full.memory ?? "");
    if (!inferred || validateTopicKey(inferred)) {
      actions.push({
        id: row.id,
        action: "skip",
        reason: inferred ? `invalid inferred ${inferred}` : "could not infer topic_key",
      });
      continue;
    }

    const canonical = validByKey.get(inferred);
    if (canonical && canonical.id !== row.id) {
      actions.push({
        id: row.id,
        action: "delete",
        topic_key: inferred,
        reason: `superseded by ${canonical.id}`,
      });
      continue;
    }

    actions.push({
      id: row.id,
      action: "repair",
      topic_key: inferred,
      reason: "no valid canonical row",
    });
    validByKey.set(inferred, row);
  }

  // If multiple corrupted share inferred key, keep newest, delete rest.
  const repairGroups = new Map();
  for (const act of actions.filter((a) => a.action === "repair")) {
    const group = repairGroups.get(act.topic_key) ?? [];
    group.push(act);
    repairGroups.set(act.topic_key, group);
  }
  for (const [key, group] of repairGroups) {
    if (group.length <= 1) {
      continue;
    }
    group.sort((a, b) => rowTime(rows.find((r) => r.id === b.id)) - rowTime(rows.find((r) => r.id === a.id)));
    for (const dup of group.slice(1)) {
      const idx = actions.findIndex((a) => a.id === dup.id);
      actions[idx] = {
        id: dup.id,
        action: "delete",
        topic_key: key,
        reason: "duplicate corrupted row; keeping newest for repair",
      };
    }
  }

  for (const act of actions) {
    console.log(`${dryRun ? "[dry-run] " : ""}${act.action.toUpperCase()} ${act.id} ${act.topic_key ?? ""} — ${act.reason ?? ""}`);
    if (dryRun || act.action === "skip") {
      continue;
    }
    if (act.action === "delete") {
      await deleteMemory(cfg, act.id);
      continue;
    }
    const row = await getMemory(cfg, act.id);
    const metadata = buildSaveMetadata({
      title: act.topic_key,
      topicKey: act.topic_key,
      project: "marengo",
      capturePrompt: false,
    });
    await updateMemory(cfg, act.id, row.memory ?? "", metadata);
  }

  console.log("Done.");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
