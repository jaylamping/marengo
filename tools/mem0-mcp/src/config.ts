export type Mem0Config = {
  apiUrl: string;
  apiKey: string;
  userId: string;
};

export function loadConfig(): Mem0Config {
  const apiUrl = process.env.MEM0_API_URL?.replace(/\/$/, "");
  const apiKey = process.env.MEM0_API_KEY;
  const userId = process.env.MEM0_USER_ID ?? "marengo-joey";

  if (!apiUrl) {
    throw new Error("MEM0_API_URL is required (e.g. https://joey-pc.tail0b414.ts.net:8888)");
  }
  if (!apiKey) {
    throw new Error("MEM0_API_KEY is required (m0sk_… from mem0 dashboard)");
  }

  return { apiUrl, apiKey, userId };
}
