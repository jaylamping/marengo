import { MEM0_USER_ID, resolveMem0ApiUrl } from "./defaults.js";

export type Mem0Config = {
  apiUrl: string;
  apiKey: string;
  userId: string;
};

export function loadConfig(): Mem0Config {
  const apiUrl = resolveMem0ApiUrl();
  const apiKey = process.env.MEM0_API_KEY;
  const userId = process.env.MEM0_USER_ID ?? MEM0_USER_ID;

  if (!apiKey) {
    throw new Error("MEM0_API_KEY is required (m0sk_… from mem0 dashboard)");
  }

  return { apiUrl, apiKey, userId };
}
