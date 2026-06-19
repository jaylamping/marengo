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
      "  done",
      "else",
      '  echo "warn: $DST not writable and passwordless sudo is unavailable"',
      '  echo "run once on the Pi:"',
      '  echo "  cd $HOME/marengo && sudo ./scripts/install-pi.sh"',
      "fi",
      "for asset in " + assetNames + "; do",
      '  if [[ -f "$DST/$asset" ]]; then',
      '    echo "--- $DST/$asset"',
      '    grep -A3 "<inertial>" "$DST/$asset" | sed -n "1,4p"',
      "  fi",
      "done",
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
        `mkdir -p ${shellQuote(`${expandStagingRoot(cfg)}/${remoteRel}`)}`,
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
        `mkdir -p ${shellQuote(`${expandStagingRoot(cfg)}/${remoteRel}`)}`,
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
    return steps.join("\n\n");
  }

  const verifyStaging = await runRemote(
    benchUrdfStagingVerifyBody(cfg, args.assets),
    15_000,
  );
  steps.push(`[staging URDF inertials]\n${verifyStaging}`);

  if (args.install_to_opt) {
    const installBody = benchUrdfInstallBody(cfg, args.assets);
    const install = await runRemote(installBody, 30_000);
    steps.push(`[install URDFs → ${remoteOpt}]\n${install}`);
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
    .default("shoulder_pitch_right_only")
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
    .describe("Also install into /opt/marengo/assets/urdf when writable"),
});
