import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { z } from "zod";
import { loadConfig } from "./config.js";
import { registerMemoryTools } from "./tools/memory.js";

function schemaOf(zodSchema: z.ZodTypeAny): Record<string, unknown> {
  return zodToJsonSchema(zodSchema) as Record<string, unknown>;
}

async function main() {
  const cfg = loadConfig();
  const allTools = registerMemoryTools(cfg);

  type ToolEntry = {
    description: string;
    inputSchema: z.ZodTypeAny;
    handler: (args: Record<string, unknown>) => Promise<string>;
  };

  const server = new Server(
    { name: "mem0-mcp", version: "0.1.0" },
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
      const parsed = tool.inputSchema.parse(request.params.arguments ?? {});
      const text = await tool.handler(parsed as Record<string, unknown>);
      const isError = text.startsWith("Error:");
      return {
        content: [{ type: "text", text }],
        isError,
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return {
        content: [{ type: "text", text: `Error: ${message}` }],
        isError: true,
      };
    }
  });

  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
