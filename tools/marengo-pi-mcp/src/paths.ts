import path from "node:path";
import type { MarengoPiConfig } from "./config.js";

/** Allowlisted remote paths for pi_read_file. */
export function isAllowedReadPath(cfg: MarengoPiConfig, requested: string): boolean {
  const normalized = path.posix.normalize(requested.replace(/\\/g, "/"));
  if (normalized.includes("..")) return false;

  const allowPrefixes = [
    `${cfg.piRoot}/config/`,
    `${cfg.piRoot}/var/log/`,
    "/etc/marengo/env",
  ];

  if (allowPrefixes.some((p) => normalized === p || normalized.startsWith(p))) {
    return true;
  }

  const basename = path.posix.basename(normalized);
  if (
    normalized.startsWith(`${cfg.piRoot}/`) &&
    (basename === "motors.yaml" ||
      basename === "robot.yaml" ||
      basename === "control.yaml" ||
      basename === ".deploy-rev")
  ) {
    return true;
  }

  return false;
}
