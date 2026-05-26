import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { zodToJsonSchema } from "zod-to-json-schema";
import { loadConfig } from "./config.js";
import { execRemote, formatRemoteResult } from "./ssh.js";
import { registerReadonlyTools } from "./tools/readonly.js";
import { registerLogTools } from "./tools/logs.js";
import { registerAdminTools } from "./tools/admin.js";
import { makeAuditMotion, registerMotionTools } from "./tools/motion.js";

// zod-to-json-schema is optional; inline minimal schema helper if not installed
function schemaOf(zodSchema: { _def?: unknown }): Record<string, unknown> {
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return zodToJsonSchema(zodSchema as any) as Record<string, unknown>;
  } catch {
    return { type: "object", properties: {} };
  }
}

async function main() {
  const cfg = loadConfig();

  async function runRemote(body: string, timeoutMs?: number): Promise<string> {
    const r = await execRemote(cfg, body, { timeoutMs });
    return formatRemoteResult(r);
  }

  const auditMotion = makeAuditMotion(cfg);
  const readonly = registerReadonlyTools(cfg, runRemote);
  const logs = registerLogTools(cfg, runRemote);
  const admin = registerAdminTools(cfg, runRemote);
  const motion = registerMotionTools(cfg, runRemote, auditMotion);

  const allTools = { ...readonly, ...logs, ...admin, ...motion };

  type ToolEntry = {
    description: string;
    inputSchema: { _def?: unknown };
    handler: (args: Record<string, unknown>) => Promise<string>;
  };

  const server = new Server(
    { name: "marengo-pi-mcp", version: "0.1.0" },
    { capabilities: { tools: {} } },
  );

  server.setRequestHandler(ListToolsRequestSchema, async () => ({
    tools: Object.entries(allTools).map(([name, t]) => {
      const tool = t as ToolEntry;
      return {
        name,
        description: tool.description,
        inputSchema: schemaOf(tool.inputSchema),
      };
    }),
  }));

  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const name = request.params.name;
    const tool = allTools[name as keyof typeof allTools] as ToolEntry | undefined;
    if (!tool) {
      return {
        content: [{ type: "text", text: `Unknown tool: ${name}` }],
        isError: true,
      };
    }

    try {
      const text = await tool.handler(
        (request.params.arguments ?? {}) as Record<string, unknown>,
      );
      const isError =
        text.startsWith("Motion blocked") ||
        text.startsWith("Weighted motion blocked") ||
        text.startsWith("Error:") ||
        text.startsWith("Local repo dirty") ||
        text.includes("RECOVER_FAIL");
      return {
        content: [{ type: "text", text }],
        isError,
      };
    } catch (err) {
      return {
        content: [{ type: "text", text: String(err) }],
        isError: true,
      };
    }
  });

  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
