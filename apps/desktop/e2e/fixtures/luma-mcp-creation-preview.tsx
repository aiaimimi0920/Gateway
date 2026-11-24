// Stateful synthetic client for the actual creation panel; no external or management requests.
import { useState } from "react";
import { Button, Dialog } from "@neuro/ui";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../src/api/client";
import { LumaMcpCreationPanel } from "../../src/features/console/LumaMcpCreationPanel";
import type { LumaCreationTask } from "../../src/api/console/luma-creation";
import { reviewPreview } from "./luma-mcp-review-preview";

const synthetic = window as unknown as { syntheticRequests: unknown[] };
const tasks = new Map<string, LumaCreationTask>();
const client = { request: async (path: string, schema, options?: GatewayApiRequestOptions) => {
  const body = options?.body as { action: string; taskId?: string; request?: { kind: "image" | "video"; budgetCredits: number;
    options?: { model?: string; operation?: string } }; chargeConfirmed?: boolean };
  synthetic.syntheticRequests.push({ path, body });
  let task: LumaCreationTask;
  if (body.action === "prepare" && body.request) {
    const request = body.request;
    task = { id: `luma-mcp-${crypto.randomUUID()}`, providerId: "preview-mcp", credentialId: "synthetic", revision: "preview-r1",
      kind: request.kind, budgetCredits: request.budgetCredits, state: "prepared", sessionId: null, runId: null,
      ended: false, leaseUntil: 0, error: null, assets: [], usage: null,
      requestedModel: request.options?.model ?? "luma_uni_1", operation: request.options?.operation ?? "generate" };
  } else {
    const saved = tasks.get(body.taskId ?? "");
    if (!saved) throw new Error("Unknown synthetic task");
    task = { ...saved };
    if (body.action === "start") {
      if (task.state !== "prepared" || !body.chargeConfirmed) throw new Error("Unconfirmed synthetic generation");
      task.state = "running"; task.sessionId = "synthetic-session"; task.runId = "synthetic-run";
    } else if (body.action === "read") {
      task.state = "completed";
      task.assets = [{ id: "synthetic-asset", kind: task.kind, state: "ready", url: "https://cdn.example.com/result" }];
      task.usage = { credits: 9, settled: true, estimated: false, unresolvedEvents: 0, scope: "session" };
    } else if (body.action === "cancel") task.state = "cancelled";
    else if (body.action === "end") task.ended = true;
    if (new URLSearchParams(window.location.search).get("mcpReview") === "1") reviewPreview(task, body.action);
  }
  tasks.set(task.id, task);
  return schema.parse({ revision: "preview-r1", providerId: "preview-mcp", credentialId: "synthetic", task });
} } as GatewayApiClient;

export function LumaMcpCreationPreview() {
  const [open, setOpen] = useState(false);
  return <>
    <Button onClick={() => setOpen(true)}>打开 MCP 创作预览</Button>
    <Dialog open={open} onOpenChange={setOpen} title="合成 MCP 创作" closeLabel="关闭" className="nt-credential-dialog">
      <div className="nt-credential-scroll"><LumaMcpCreationPanel client={client} managementToken="synthetic-owner" secretGrant="synthetic-grant"
        providerId="preview-mcp" credentialId="synthetic" expectedRevision="preview-r1" disabled={false}
        onRequestSecretAccess={() => undefined} /></div>
    </Dialog>
  </>;
}
