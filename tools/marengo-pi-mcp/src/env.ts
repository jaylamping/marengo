import type { MarengoPiConfig } from "./config.js";

/** Remote shell preamble for Pi SSH sessions. */
export function remotePreamble(cfg: MarengoPiConfig, debug = false): string {
  const rustLog = debug
    ? "robstride=trace,davout=debug,berthier=info,marengo_pi=info"
    : "robstride=info,davout=info,berthier=info,marengo_pi=info";

  return [
    "set -euo pipefail",
    "if [[ -f /etc/marengo/env ]]; then set -a; source /etc/marengo/env; set +a; fi",
    `export MARENGO_ROOT=${shellQuote(cfg.piRoot)}`,
    `export MARENGO_CONFIG_DIR=${shellQuote(cfg.configDir)}`,
    `export RUST_LOG=${shellQuote(rustLog)}`,
    `cd ${shellQuote(cfg.piRoot)}`,
  ].join("\n");
}

export function wrapRemote(cfg: MarengoPiConfig, body: string, debug = false): string {
  return `${remotePreamble(cfg, debug)}\n${body}`;
}

export function shellQuote(s: string): string {
  return `'${s.replace(/'/g, `'\\''`)}'`;
}
