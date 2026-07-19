import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { wrapRemote } from "../env.js";

export const cleanTreeSchema = z.object({
  confirm: z.literal(true).describe("Must be true to modify the Pi working tree"),
  mode: z
    .enum(["stash", "reset-hard", "clean-untracked"])
    .default("stash")
    .describe(
      "stash = git stash --include-untracked (safest); " +
        "reset-hard = git reset --hard HEAD (discards tracked changes); " +
        "clean-untracked = git clean -fd plus git reset --hard HEAD",
    ),
});

export type CleanTreeArgs = z.infer<typeof cleanTreeSchema>;

export async function runCleanTree(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  args: CleanTreeArgs,
): Promise<string> {
  let body: string;
  switch (args.mode) {
    case "reset-hard":
      body = ["git reset --hard HEAD", "git status --short"].join("\n");
      break;
    case "clean-untracked":
      body = ["git reset --hard HEAD", "git clean -fd", "git status --short"].join("\n");
      break;
    case "stash":
    default:
      body = [
        "git stash push --include-untracked -m 'pi_clean_tree stash'",
        "git status --short",
      ].join("\n");
      break;
  }

  return runRemote(wrapRemote(cfg, body), 60_000);
}
