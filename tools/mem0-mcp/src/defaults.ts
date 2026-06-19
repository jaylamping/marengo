/** Canonical Marengo mem0 endpoints (Tailscale Serve on joey-pc). Single source of truth. */
export const MEM0_API_URL = "https://joey-pc.tail0b414.ts.net:8888";
export const MEM0_DASHBOARD_URL = "https://joey-pc.tail0b414.ts.net";
export const MEM0_USER_ID = "marengo-joey";

/** Resolve API URL: explicit arg → MEM0_API_URL env → canonical default. */
export function resolveMem0ApiUrl(explicit?: string): string {
  const raw = explicit ?? process.env.MEM0_API_URL ?? MEM0_API_URL;
  return raw.replace(/\/$/, "");
}
