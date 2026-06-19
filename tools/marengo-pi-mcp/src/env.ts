import type { MarengoPiConfig } from "./config.js";

/** Remote shell preamble for Pi SSH sessions. */
export function remotePreamble(cfg: MarengoPiConfig, debug = false): string {
  const rustLog = debug
    ? "robstride=trace,davout=debug,berthier=info,marengo_pi=debug"
    : "robstride=info,davout=info,berthier=info,marengo_pi=debug";

  return [
    "set -euo pipefail",
    "if [[ -f /etc/marengo/env ]]; then set -a; source /etc/marengo/env; set +a; fi",
    'if [[ -f "${HOME}/.cargo/env" ]]; then set -a; source "${HOME}/.cargo/env"; set +a; fi',
    'export PATH="${HOME}/.cargo/bin:/usr/local/cargo/bin:${PATH:-}"',
    `export MARENGO_ROOT=${shellQuote(cfg.piRoot)}`,
    `export MARENGO_CONFIG_DIR=${shellQuote(cfg.configDir)}`,
    `export RUST_LOG=${shellQuote(rustLog)}`,
    `cd ${shellQuote(cfg.piRoot)}`,
  ].join("\n");
}

export function wrapRemote(cfg: MarengoPiConfig, body: string, debug = false): string {
  return `${remotePreamble(cfg, debug)}\n${body}`;
}

/** Remote script with optional config-dir override (e.g. shoulder_pitch_right_only). */
export function wrapRemoteWithConfig(
  cfg: MarengoPiConfig,
  body: string,
  configDir?: string,
  debug = false,
): string {
  const effective = configDir ? { ...cfg, configDir } : cfg;
  return wrapRemote(effective, body, debug);
}

export function shellQuote(s: string): string {
  return `'${s.replace(/'/g, `'\\''`)}'`;
}
