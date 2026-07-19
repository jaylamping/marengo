import path from "node:path";
import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { homingPreflightShell } from "../homing-preflight.js";
import { shellQuote, wrapRemote, wrapRemoteWithConfig } from "../env.js";
import { sshTarget } from "../config.js";
import { execLocal, formatRemoteResult } from "../ssh.js";

const benchUrdfAssets = [
  "shoulder_pitch_right_only.urdf",
  "shoulder_pitch_weighted.urdf",
  "shoulder_pitch_left_bare.urdf",
  "arm_4dof_right.urdf",
  "arm_3dof_right.urdf",
] as const;

type BenchUrdfAsset = (typeof benchUrdfAssets)[number];

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

export function benchUrdfStagingVerifyBody(
  cfg: MarengoPiConfig,
  assets: BenchUrdfAsset[],
): string {
  const remoteRel = "assets/urdf";
  const stagedFiles = assets
    .map((asset) => remotePathExpr(`${expandStagingRoot(cfg)}/${remoteRel}/${asset}`))
    .join(" ");

  return wrapRemote(
    cfg,
    [
      "for urdf in " + stagedFiles + "; do",
      '  echo "--- $urdf"',
      '  grep -A3 "<inertial>" "$urdf" | sed -n "1,4p"',
      "done",
    ].join("\n"),
  );
}

/** True when a remote step string includes a non-zero `[exit N]` from formatRemoteResult. */
export function remoteStepFailed(output: string): boolean {
  return /\[exit [1-9]\d*\]/.test(output);
}

export function benchUrdfInstallBody(
  cfg: MarengoPiConfig,
  assets: BenchUrdfAsset[],
): string {
  const remoteRel = "assets/urdf";
  const remoteOpt = `${cfg.piRoot}/${remoteRel}`;
  const assetNames = assets.map((asset) => shellQuote(asset)).join(" ");

  return wrapRemote(
    cfg,
    [
      `SRC=${remotePathExpr(`${expandStagingRoot(cfg)}/${remoteRel}`)}`,
      `DST=${shellQuote(remoteOpt)}`,
      'if [[ -d "$DST" && -w "$DST" ]]; then',
      "  for asset in " + assetNames + "; do",
      '    install -m 0644 "$SRC/$asset" "$DST/$asset"',
      '    echo "installed $DST/$asset (direct write)"',
      '    echo "--- $DST/$asset"',
      '    grep -A3 "<inertial>" "$DST/$asset" | sed -n "1,4p"',
      "  done",
      "elif sudo -n /opt/marengo/scripts/can-up.sh can0 can1 2>/dev/null; then",
      '  echo "warn: $DST not writable; run pi_install_staging to refresh /opt from ~/marengo"',
      "  exit 1",
      "else",
      '  echo "warn: cannot write $DST and passwordless sudo is unavailable"',
      '  echo "run once on the Pi:"',
      '  echo "  cd $HOME/marengo && sudo ./scripts/install-pi.sh"',
      "  exit 1",
      "fi",
    ].join("\n"),
  );
}

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

  let sync = await execLocal(
    "rsync",
    [
      "-av",
      `${localDir}/`,
      `${sshTarget(cfg)}:${remoteStaging}/`,
    ],
    { cwd: cfg.localRoot, timeoutMs: 60_000 },
  );

  const steps: string[] = [`[local sync → ${remoteStaging}]`];

  if (sync.exitCode !== 0) {
    await runRemote(
      wrapRemote(
        cfg,
        `mkdir -p ${remotePathExpr(`${expandStagingRoot(cfg)}/${remoteRel}`)}`,
      ),
      15_000,
    );
    steps.push("[mkdir staging]");
    sync = await execLocal(
      "scp",
      ["-r", `${localDir}/.`, `${sshTarget(cfg)}:${remoteStaging}/`],
      { cwd: cfg.localRoot, timeoutMs: 60_000 },
    );
    steps.push(`[local scp fallback]\n${formatRemoteResult(sync)}`);
  } else {
    steps.push(formatRemoteResult(sync));
  }

  if (sync.exitCode !== 0) {
    return `Error: config sync failed\n\n${steps.join("\n\n")}`;
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
        'elif sudo -n /opt/marengo/scripts/can-up.sh can0 can1 2>/dev/null; then',
        '  echo "warn: $DST not writable; run pi_install_staging to refresh /opt from ~/marengo"',
        "else",
        '  echo "warn: cannot write $DST and passwordless sudo is unavailable"',
        '  echo "run once on the Pi:"',
        '  echo "  cd $HOME/marengo && sudo ./scripts/install-pi.sh"',
        "fi",
      ].join("\n"),
    );
    const install = await runRemote(installBody, 30_000);
    steps.push(`[install → ${remoteOpt}]\n${install}`);

    const homingBody = wrapRemoteWithConfig(
      cfg,
      homingPreflightShell(false),
      remoteOpt,
    );
    const homing = await runRemote(homingBody, 30_000);
    steps.push(`[homing preflight → ${remoteOpt}]\n${homing}`);
  } else {
    steps.push(
      `[note] Staging only. Bench with MARENGO_CONFIG_DIR=${remoteStaging} or install_to_opt: true.`,
    );
  }

  return steps.join("\n\n---\n\n");
}

export async function runSyncBenchUrdfAssets(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  args: {
    assets: BenchUrdfAsset[];
    install_to_opt: boolean;
  },
): Promise<string> {
  const localDir = path.join(cfg.localRoot, "assets", "urdf");
  const remoteRel = "assets/urdf";
  const remoteStaging = `${cfg.piStagingRoot}/${remoteRel}`.replace(/\/+/g, "/");
  const remoteOpt = `${cfg.piRoot}/${remoteRel}`;
  const localFiles = args.assets.map((asset) => path.join(localDir, asset));

  let sync = await execLocal(
    "rsync",
    [
      "-av",
      ...localFiles,
      `${sshTarget(cfg)}:${remoteStaging}/`,
    ],
    { cwd: cfg.localRoot, timeoutMs: 60_000 },
  );

  const steps: string[] = [`[local URDF sync → ${remoteStaging}]`];

  if (sync.exitCode !== 0) {
    await runRemote(
      wrapRemote(
        cfg,
        `mkdir -p ${remotePathExpr(`${expandStagingRoot(cfg)}/${remoteRel}`)}`,
      ),
      15_000,
    );
    steps.push("[mkdir URDF staging]");
    sync = await execLocal(
      "scp",
      [
        ...localFiles,
        `${sshTarget(cfg)}:${remoteStaging}/`,
      ],
      { cwd: cfg.localRoot, timeoutMs: 60_000 },
    );
    steps.push(`[local scp fallback]\n${formatRemoteResult(sync)}`);
  } else {
    steps.push(formatRemoteResult(sync));
  }

  if (sync.exitCode !== 0) {
    return `Error: URDF sync failed\n\n${steps.join("\n\n")}`;
  }

  const verifyStaging = await runRemote(
    benchUrdfStagingVerifyBody(cfg, args.assets),
    15_000,
  );
  steps.push(`[staging URDF inertials]\n${verifyStaging}`);
  if (remoteStepFailed(verifyStaging)) {
    return `Error: URDF staging verify failed\n\n${steps.join("\n\n---\n\n")}`;
  }

  if (args.install_to_opt) {
    const installBody = benchUrdfInstallBody(cfg, args.assets);
    const install = await runRemote(installBody, 30_000);
    steps.push(`[install URDFs → ${remoteOpt}]\n${install}`);
    if (remoteStepFailed(install)) {
      return `Error: URDF install to ${remoteOpt} failed\n\n${steps.join("\n\n---\n\n")}`;
    }
  } else {
    steps.push(
      `[note] Staging only. Bench with MARENGO_ROOT=${expandStagingRoot(cfg)} or install_to_opt: true.`,
    );
  }

  return steps.join("\n\n---\n\n");
}

export const syncBenchConfigSchema = z.object({
  profile: z
    .string()
    .default("arm_4dof_right")
    .describe("Bringup folder under config/bringup/"),
  install_to_opt: z
    .boolean()
    .default(true)
    .describe("Also rsync into /opt/marengo (requires passwordless sudo)"),
});

export const syncBenchUrdfSchema = z.object({
  assets: z
    .array(z.enum(benchUrdfAssets))
    .nonempty()
    .default(["shoulder_pitch_right_only.urdf", "shoulder_pitch_weighted.urdf"])
    .describe("Bench URDF asset filenames under assets/urdf/"),
  install_to_opt: z
    .boolean()
    .default(true)
    .describe("Also install into /opt/marengo (requires writable assets/urdf or pi_install_staging)"),
});
