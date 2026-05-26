import path from "node:path";
import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { shellQuote, wrapRemote } from "../env.js";
import { sshTarget } from "../config.js";
import { execLocal, formatRemoteResult } from "../ssh.js";

function expandStagingRoot(cfg: MarengoPiConfig): string {
  if (cfg.piStagingRoot.startsWith("~/")) {
    return `$HOME${cfg.piStagingRoot.slice(1)}`;
  }
  return cfg.piStagingRoot;
}

function remotePathExpr(path: string): string {
  if (path.startsWith("$HOME/")) {
    return `"${"$HOME"}${path.slice("$HOME".length)}"`;
  }
  return shellQuote(path);
}

export const directInstallRsyncLine =
  'rsync -r --no-owner --no-group --no-perms --omit-dir-times "$SRC/" "$DST/"';

export async function runSyncBenchConfig(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  args: {
    profile: string;
    install_to_opt: boolean;
  },
): Promise<string> {
  const localDir = path.join(cfg.localRoot, "config", "bringup", args.profile);
  const remoteRel = `config/bringup/${args.profile}`;
  const remoteStaging = `${cfg.piStagingRoot}/${remoteRel}`.replace(/\/+/g, "/");
  const remoteOpt = `${cfg.piRoot}/${remoteRel}`;

  const rsync = await execLocal(
    "rsync",
    [
      "-av",
      `${localDir}/`,
      `${sshTarget(cfg)}:${remoteStaging}/`,
    ],
    { cwd: cfg.localRoot, timeoutMs: 60_000 },
  );

  const steps: string[] = [
    `[local rsync → ${remoteStaging}]`,
    formatRemoteResult(rsync),
  ];

  if (rsync.exitCode !== 0) {
    return steps.join("\n\n");
  }

  const verifyBody = wrapRemote(
    cfg,
    [
      `grep -A3 'impedance:' ${remotePathExpr(`${expandStagingRoot(cfg)}/${remoteRel}/control.yaml`)}`,
    ].join("\n"),
  );
  const verify = await runRemote(verifyBody, 15_000);
  steps.push(`[staging control.yaml]\n${verify}`);

  if (args.install_to_opt) {
    const installBody = wrapRemote(
      cfg,
      [
        `SRC=${remotePathExpr(`${expandStagingRoot(cfg)}/${remoteRel}`)}`,
        `DST=${shellQuote(remoteOpt)}`,
        'if [[ -d "$DST" && -w "$DST" ]]; then',
        `  ${directInstallRsyncLine}`,
        '  echo "installed to $DST (direct write)"',
        '  grep -A3 impedance "$DST/control.yaml"',
        'elif sudo -n true 2>/dev/null; then',
        '  sudo mkdir -p "$DST"',
        '  sudo rsync -a "$SRC/" "$DST/"',
        '  echo "installed to $DST"',
        '  sudo grep -A3 impedance "$DST/control.yaml"',
        "else",
        '  echo "warn: cannot write $DST and passwordless sudo is unavailable"',
        '  echo "run once on the Pi:"',
        '  echo "  cd $HOME/marengo && sudo ./scripts/install-pi.sh"',
        "fi",
      ].join("\n"),
    );
    const install = await runRemote(installBody, 30_000);
    steps.push(`[install → ${remoteOpt}]\n${install}`);
  } else {
    steps.push(
      `[note] Staging only. Bench with MARENGO_CONFIG_DIR=${remoteStaging} or install_to_opt: true.`,
    );
  }

  return steps.join("\n\n---\n\n");
}

export const syncBenchConfigSchema = z.object({
  profile: z
    .string()
    .default("shoulder_pitch_right_only")
    .describe("Bringup folder under config/bringup/"),
  install_to_opt: z
    .boolean()
    .default(true)
    .describe("Also rsync into /opt/marengo (requires passwordless sudo)"),
});
