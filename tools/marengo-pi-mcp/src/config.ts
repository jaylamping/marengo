import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export type BenchProfile = "bare_motor" | "weighted_single_arm" | "arm_attached";

export interface MarengoPiConfig {
  host: string;
  user: string;
  piRoot: string;
  configDir: string;
  localRoot: string;
  sshIdentityFile?: string;
  benchProfile: BenchProfile;
  loadedJoint?: string;
  piStagingRoot: string;
}

function env(key: string, fallback?: string): string {
  const v = process.env[key]?.trim();
  if (v) return v;
  if (fallback !== undefined) return fallback;
  throw new Error(`Missing required env: ${key}`);
}

/** Marengo repo root: tools/marengo-pi-mcp/dist/config.js → ../../.. */
export function defaultLocalRoot(): string {
  const here = fileURLToPath(import.meta.url);
  return path.resolve(path.dirname(here), "..", "..", "..");
}

export function loadConfig(): MarengoPiConfig {
  return {
    host: env("MARENGO_PI_HOST", "marengo.local"),
    user: env("MARENGO_PI_USER", "joey"),
    piRoot: env("MARENGO_PI_ROOT", "/opt/marengo"),
    configDir: env(
      "MARENGO_CONFIG_DIR",
      "/opt/marengo/config/bringup/shoulder_pitch_dual",
    ),
    localRoot: env("MARENGO_LOCAL_ROOT", defaultLocalRoot()),
    sshIdentityFile: process.env.SSH_IDENTITY_FILE?.trim() || undefined,
    benchProfile: (process.env.MARENGO_BENCH_PROFILE?.trim() ||
      "bare_motor") as BenchProfile,
    loadedJoint: process.env.MARENGO_LOADED_JOINT?.trim() || undefined,
    piStagingRoot: process.env.MARENGO_PI_STAGING_ROOT?.trim() || "~/marengo",
  };
}

export function sshTarget(cfg: MarengoPiConfig): string {
  return `${cfg.user}@${cfg.host}`;
}

/** Absolute path to a script under MARENGO_PI_ROOT/scripts. */
export function piScriptPath(cfg: MarengoPiConfig, name: string): string {
  return `${cfg.piRoot}/scripts/${name}`;
}

/** Passwordless sudo can-up (requires /etc/sudoers.d/marengo-joey). */
export function sudoCanUpCommand(cfg: MarengoPiConfig): string {
  return `sudo -n ${piScriptPath(cfg, "can-up.sh")} can0 can1`;
}

/** Passwordless sudo install-pi; optional root when script lives outside piRoot (deploy staging). */
export function sudoInstallCommand(
  cfg: MarengoPiConfig,
  scriptRoot = cfg.piRoot,
): string {
  return `sudo -n ${scriptRoot}/scripts/install-pi.sh`;
}

/** Staging tree on Pi (~/marengo → /home/joey/marengo). */
export function piStagingAbs(cfg: MarengoPiConfig): string {
  if (cfg.piStagingRoot.startsWith("~/")) {
    return `/home/${cfg.user}${cfg.piStagingRoot.slice(1)}`;
  }
  return cfg.piStagingRoot;
}

/** Install from deploy staging into /opt/marengo (passwordless sudo). */
export function sudoStagingInstallCommand(cfg: MarengoPiConfig): string {
  return sudoInstallCommand(cfg, piStagingAbs(cfg));
}

export function auditLogPath(): string {
  return path.join(homedir(), ".marengo-pi-mcp", "audit.log");
}
