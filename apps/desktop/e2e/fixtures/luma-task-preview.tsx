// Task lookup layout fixture; each explicit read returns the next synthetic supplier observation.
import { useState } from "react";
import { Button, Dialog } from "@neuro/ui";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../src/api/client";
import { OperationsRequestTable } from "../../src/features/console/OperationsRequestTable";
import { operationsRequestFixture } from "../../src/features/console/OperationsWorkspace.fixtures";
import { LumaTaskPanel } from "../../src/features/console/LumaTaskPanel";
import "../../src/features/console/CredentialDialog.css";
import "../../src/features/console/OperationsWorkspace.css";

const id = "d290f1ee-6c54-4b01-90e6-d701748f0851";
const task = { id, model: "uni-1", type: "image", state: "completed", usage: { credits_charged: null } };
const synthetic = window as unknown as { syntheticRequests: unknown[] };
const client = { request: async (path: string, schema, options?: GatewayApiRequestOptions) => {
  synthetic.syntheticRequests.push({ path, body: options?.body });
  return schema.parse({ revision: "preview-r1", providerId: "preview-luma", credentialId: "synthetic",
    result: { ...task, usage: { credits_charged: [null, 0, 12.5][Math.min(synthetic.syntheticRequests.length - 1, 2)] } } });
} } as GatewayApiClient;
const row = { ...operationsRequestFixture("cancelled-luma-task"), status: "cancelled", providerAccountId: "preview-luma",
  requestedModel: "uni-1", resolvedModel: "uni-1", endpointKind: "images_generations", upstreamStatus: null, errorSummary: null,
  routeTrace: { realCredentialRef: "synthetic", lumaAgentsGeneration: { ...task, mediaVerified: false } } };

export function LumaTaskPreview() {
  const [open, setOpen] = useState(false);
  return <>
    <Button onClick={() => setOpen(true)}>打开任务查询</Button>
    <OperationsRequestTable rows={[row]} t={zh => zh} />
    <Dialog open={open} onOpenChange={setOpen} title="合成任务查询" closeLabel="关闭" className="nt-credential-dialog">
      <div className="nt-credential-scroll"><LumaTaskPanel client={client} managementToken="synthetic-owner" secretGrant="synthetic-grant"
        providerId="preview-luma" credentialId="synthetic" expectedRevision="preview-r1" disabled={false} onRequestSecretAccess={() => undefined} /></div>
      <div className="n-dialog-actions"><Button onClick={() => setOpen(false)}>关闭</Button></div>
    </Dialog>
  </>;
}
