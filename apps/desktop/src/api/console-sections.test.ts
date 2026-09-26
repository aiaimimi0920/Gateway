import { describe, expect, it } from "vitest";

import type { GatewayApiClient, GatewayApiRequestOptions } from "./client";
import { createConsoleApi } from "./console";

type RecordedRequest = {
  path: string;
  options: GatewayApiRequestOptions | undefined;
};

function createRecordingClient(): { client: GatewayApiClient; requests: RecordedRequest[] } {
  const requests: RecordedRequest[] = [];
  return {
    requests,
    client: {
      async request<T>(path: string, _schema: unknown, options?: GatewayApiRequestOptions) {
        requests.push({ path, options });
        return undefined as T;
      },
    },
  };
}

describe("console API sections", () => {
  it("preserves the public method insertion order", () => {
    const { client } = createRecordingClient();

    expect(Object.keys(createConsoleApi(client))).toEqual([
      "getBootstrapStatus",
      "bootstrap",
      "verifySession",
      "confirmSecretAccess",
      "rotateSession",
      "logout",
      "getRouteConfig",
      "getAccountGroupSummary",
      "getProviderCredentialInventory",
      "getCredentialPoolAutomation",
      "runCredentialPoolAutomation",
      "pruneCredentialPool",
      "purgeCredentialArchive",
      "getCredentialRefill",
      "requestCredentialRefill",
      "createGeminiAuthSession",
      "completeGeminiAuthSession",
      "getGeminiAuthSession",
      "probeCredential",
      "probeProvider",
      "getCredentialUsage",
      "listUsageAggregates",
      "getRuntimePressure",
      "getCostOverview",
      "listProviderCredentialModelStates",
      "getRequestAuditSummary",
      "validateRouteConfig",
      "commitRouteConfig",
      "listRouteConfigRevisions",
      "getRouteConfigRevision",
      "getGatewayReadiness",
      "getOperatorSummary",
      "listRequestAudits",
      "listAnalysisSamples",
      "getRequestAuditFullSummary",
      "getUsageAggregateSummary",
      "getPromptCacheSummary",
      "getRateLimitHotspots",
      "listAnomalyIncidents",
      "getAnomalyIncidentSummary",
      "getAnomalyAlertQueue",
      "listAnomalyIncidentHistory",
      "acknowledgeAnomalyIncident",
      "resolveAnomalyIncident",
      "updateAnomalyIncidentFollowUp",
      "listAnomalyPolicies",
      "listRemediationQueue",
      "listRemediationRuns",
      "getRemediationEffectiveness",
      "listPersistedAnalysisExports",
      "getAnalysisExportInventorySummary",
      "getAnalysisExportDiff",
      "getAccessCatalog",
      "createAccessKey",
      "rotateAccessKey",
      "revokeAccessKey",
      "getAccessKeyBalance",
      "createAccessBundle",
      "inspectAccessAffinity",
      "resetAccessAffinity",
      "rotateApiAccess",
      "issueUserCredential",
      "verifyUserCredential",
      "revokeUserCredential",
    ]);
  });

  it("preserves query defaults while serializing false and zero", async () => {
    const { client, requests } = createRecordingClient();
    const api = createConsoleApi(client);

    await api.listRequestAudits?.("management-token", {
      limit: 0,
      stream: false,
      status: "",
    });
    await api.listAnalysisSamples?.("management-token", {
      createdFrom: "2026-09-04T00:00:00Z",
      limit: undefined,
    });

    expect(requests).toEqual([
      {
        path: "/v1/internal/gateway/requests?limit=0&stream=false",
        options: { managementToken: "management-token" },
      },
      {
        path:
          "/v1/internal/gateway/analysis/samples?limit=100&createdFrom=2026-09-04T00%3A00%3A00Z",
        options: { managementToken: "management-token" },
      },
    ]);
  });

  it("preserves encoded access paths and nullable mutation bodies", async () => {
    const { client, requests } = createRecordingClient();
    const api = createConsoleApi(client);

    await api.rotateAccessKey?.("management-token", "key/one");
    await api.revokeAccessKey?.("management-token", "key/one");
    await api.inspectAccessAffinity?.("management-token", {
      accessKeyId: "key/one",
      model: "model one",
    });
    await api.verifyUserCredential?.("management-token", "credential-one");

    expect(requests.map(({ path }) => path)).toEqual([
      "/v1/internal/gateway/access/keys/key%2Fone/rotate",
      "/v1/internal/gateway/access/keys/key%2Fone/revoke",
      "/v1/internal/gateway/access/affinity?accessKeyId=key%2Fone&model=model+one",
      "/v1/internal/gateway/user-credentials/verify",
    ]);
    expect(requests[1]?.options).toEqual({
      method: "POST",
      managementToken: "management-token",
      body: { reason: null },
    });
    expect(requests[3]?.options).toEqual({
      method: "POST",
      managementToken: "management-token",
      body: { credentialKey: "credential-one", scope: null },
    });
  });
});
