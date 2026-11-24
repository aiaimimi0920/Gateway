import { z } from "zod";
import type { GatewayApiClient } from "./client";

const catalogSchema = z.object({
  deployments: z.array(z.object({
    id: z.string().min(1).max(128), deploymentUrl: z.url(),
    configurationName: z.string().min(1).max(256).optional(),
    modelName: z.string().min(1).max(128).optional(), modelVersion: z.string().max(128).nullable().optional(),
  })).max(250),
  truncated: z.boolean(),
});
export type SapDeploymentCatalog = z.infer<typeof catalogSchema>;
export type SapDeploymentProtocol = "azure" | "bedrock" | "orchestration";
export type DiscoverSapDeployments = (body: { serviceKey: string; resourceGroup: string; protocol?: SapDeploymentProtocol }, signal: AbortSignal) => Promise<SapDeploymentCatalog>;

export function sapDeploymentLookup(client: GatewayApiClient, managementToken: string | null, secretGrant: string | null): DiscoverSapDeployments {
  return (body, signal) => client.request("/v1/internal/gateway/console/sap-aicore-deployments", catalogSchema, {
    method: "POST", body, signal, managementToken, secretGrant,
  });
}
