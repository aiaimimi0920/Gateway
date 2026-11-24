// Contract snapshots contain catalog data only; annotations never authorize execution or retries.
import { z } from "zod";

export const lumaMcpToolSchema = z.object({ name: z.string().min(1).max(128).regex(/^[A-Za-z0-9_.-]+$/),
  title: z.string().max(1024).nullable(), description: z.string().max(16384).nullable(),
  inputSchema: z.object({ type: z.literal("object") }).passthrough(),
  outputSchema: z.record(z.string(), z.unknown()).nullable(),
  annotations: z.record(z.string(), z.unknown()).nullable().optional() });
type Tool = z.infer<typeof lumaMcpToolSchema>;

export function downloadLumaMcpSnapshot(tools: readonly Tool[], capturedAt: string) {
  // Explicit projection excludes management identity, transport headers and arbitrary response metadata.
  const snapshot = { format: "gateway-luma-mcp-catalog", schemaVersion: 1, capturedAt,
    resource: "https://agents.lumalabs.ai/mcp", protocolVersion: "2025-03-26",
    tools: tools.map(({ name, title, description, inputSchema, outputSchema, annotations }) =>
      ({ name, title, description, inputSchema, outputSchema, annotations: annotations ?? null })) };
  const url = URL.createObjectURL(new Blob([JSON.stringify(snapshot, null, 2) + "\n"], { type: "application/json;charset=utf-8" }));
  const anchor = document.createElement("a");
  anchor.href = url; anchor.download = `luma-mcp-catalog-${capturedAt.replace(/[:.]/g, "-")}.json`;
  try { document.body.append(anchor); anchor.click(); }
  finally { anchor.remove(); window.setTimeout(() => URL.revokeObjectURL(url), 1000); }
}
