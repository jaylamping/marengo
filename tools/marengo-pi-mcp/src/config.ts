import { homedir } from "node:os";
import path from "node:path";

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

export function loadConfig(): MarengoPiConfig {
  return {
    host: env("MARENGO_PI_HOST", "marengo.local"),
    user: env("MARENGO_PI_USER", "joey"),
    piRoot: env("MARENGO_PI_ROOT", "/opt/marengo"),
    configDir: env(
      "MARENGO_CONFIG_DIR",
      "/opt/marengo/config/bringup/shoulder_pitch_dual",
    ),
    localRoot: env("MARENGO_LOCAL_ROOT", process.cwd()),
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

export function auditLogPath(): string {
  return path.join(homedir(), ".marengo-pi-mcp", "audit.log");
}
