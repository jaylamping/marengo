import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { wrapRemote } from "../env.js";

export const syncTreeSchema = z.object({}).strict();

export async function runSyncTree(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
): Promise<string> {
  const body = wrapRemote(
    cfg,
    [
      "if ! git diff --quiet || ! git diff --cached --quiet; then",
      "  echo 'dirty working tree — commit or stash first' >&2",
      "  git status --short",
      "  exit 1",
      "fi",
      "git fetch origin",
      "git checkout main",
      "git pull --ff-only origin main",
      "echo '---'",
      "git log -1 --oneline",
      "git rev-parse HEAD",
    ].join("\n"),
  );

  return runRemote(body, 120_000);
}
