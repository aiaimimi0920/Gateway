import type { GatewayApiClient } from "../client";
import {
  consoleCostOverviewResponseSchema,
  consoleCredentialArchivePurgeResponseSchema,
  consoleCredentialPoolAutomationResponseSchema,
  consoleCredentialPoolAutomationRunResponseSchema,
  consoleCredentialProbeResponseSchema,
  consoleCredentialRefillRequestResponseSchema,
  consoleCredentialRefillResponseSchema,
  consoleCredentialUsageResponseSchema,
  consoleGeminiAuthSessionResponseSchema,
  consoleProviderCredentialInventoryResponseSchema,
  consoleProviderCredentialModelStateResponseSchema,
  consoleProviderProbeResponseSchema,
  consoleRequestAuditSummaryResponseSchema,
  consoleRuntimePressureResponseSchema,
  consoleUsageAggregateResponseSchema,
} from "../schemas";
import type { ConsoleApi } from "./api";
import { CONSOLE_ROOT } from "./roots";

type ConsoleCredentialsApi = Pick<
  ConsoleApi,
  | "getProviderCredentialInventory"
  | "getCredentialPoolAutomation"
  | "runCredentialPoolAutomation"
  | "pruneCredentialPool"
  | "purgeCredentialArchive"
  | "getCredentialRefill"
  | "requestCredentialRefill"
  | "createGeminiAuthSession"
  | "completeGeminiAuthSession"
  | "getGeminiAuthSession"
  | "probeCredential"
  | "probeProvider"
  | "readProviderProbeResults"
  | "getCredentialUsage"
  | "listUsageAggregates"
  | "getRuntimePressure"
  | "getCostOverview"
  | "listProviderCredentialModelStates"
  | "getRequestAuditSummary"
>;

export function createConsoleCredentialsApi(client: GatewayApiClient): ConsoleCredentialsApi {
  return {
    getProviderCredentialInventory: (managementToken) =>
      client.request(
        "/v1/internal/gateway/provider-credentials?maskSecrets=true",
        consoleProviderCredentialInventoryResponseSchema,
        { managementToken },
      ),
    getCredentialPoolAutomation: (managementToken) =>
      client.request(
        "/v1/internal/gateway/credential-pool-automation",
        consoleCredentialPoolAutomationResponseSchema,
        { managementToken },
      ),
    runCredentialPoolAutomation: (managementToken, providerId) =>
      client.request(
        `/v1/internal/gateway/credential-pool-automation/providers/${encodeURIComponent(providerId)}/run`,
        consoleCredentialPoolAutomationRunResponseSchema,
        { method: "POST", managementToken },
      ),
    pruneCredentialPool: (managementToken, providerId) =>
      client.request(
        `/v1/internal/gateway/credential-pool-automation/providers/${encodeURIComponent(providerId)}/prune`,
        consoleCredentialPoolAutomationRunResponseSchema,
        { method: "POST", managementToken },
      ),
    purgeCredentialArchive: (managementToken, providerId) =>
      client.request(
        `/v1/internal/gateway/credential-pool-automation/providers/${encodeURIComponent(providerId)}/archive`,
        consoleCredentialArchivePurgeResponseSchema,
        { method: "DELETE", managementToken },
      ),
    getCredentialRefill: (managementToken) =>
      client.request(
        "/v1/internal/gateway/credential-pool-refill",
        consoleCredentialRefillResponseSchema,
        { managementToken },
      ),
    requestCredentialRefill: (managementToken, providerId, requestedCount) =>
      client.request(
        `/v1/internal/gateway/credential-pool-refill/providers/${encodeURIComponent(providerId)}/request`,
        consoleCredentialRefillRequestResponseSchema,
        {
          method: "POST",
          managementToken,
          body: requestedCount === undefined ? {} : { requestedCount },
        },
      ),
    createGeminiAuthSession: (managementToken, request) =>
      client.request(`${CONSOLE_ROOT}/gemini-auth-sessions`, consoleGeminiAuthSessionResponseSchema, {
        method: "POST",
        managementToken,
        body: request,
      }),
    completeGeminiAuthSession: (managementToken, sessionId) =>
      client.request(
        `${CONSOLE_ROOT}/gemini-auth-sessions/${encodeURIComponent(sessionId)}/complete`,
        consoleGeminiAuthSessionResponseSchema,
        { method: "POST", managementToken },
      ),
    getGeminiAuthSession: (managementToken, sessionId) =>
      client.request(
        `${CONSOLE_ROOT}/gemini-auth-sessions/${encodeURIComponent(sessionId)}`,
        consoleGeminiAuthSessionResponseSchema,
        { managementToken },
      ),
    probeCredential: (managementToken, secretGrant, credentialId) =>
      client.request(
        `/v1/internal/gateway/console/credentials/${encodeURIComponent(credentialId)}/probe`,
        consoleCredentialProbeResponseSchema,
        { method: "POST", managementToken, secretGrant },
      ),
    probeProvider: (managementToken, secretGrant, providerId, request, options) =>
      client.request(
        `/v1/internal/gateway/console/providers/${encodeURIComponent(providerId)}/probe`,
        consoleProviderProbeResponseSchema,
        { method: "POST", managementToken, secretGrant, body: request, ...options },
      ),
    readProviderProbeResults: (managementToken, secretGrant, providerId, options, scope, query) => client.request(
      `/v1/internal/gateway/console/providers/${encodeURIComponent(providerId)}/probe/results`,
      consoleProviderProbeResponseSchema, { method: "POST", body: { scope, ...query }, managementToken, secretGrant, ...options },
    ),
    getCredentialUsage: (managementToken, credentialId, createdFrom) => {
      const query = new URLSearchParams({
        providerCredentialRef: credentialId,
        createdFrom,
        limit: "1000",
      });
      return client.request(
        `/v1/internal/gateway/usage-aggregates?${query.toString()}`,
        consoleCredentialUsageResponseSchema,
        { managementToken },
      );
    },
    listUsageAggregates: (managementToken, params) => {
      const query = new URLSearchParams({ limit: String(params?.limit ?? 5000) });
      if (params?.createdFrom) {
        query.set("createdFrom", params.createdFrom);
      }
      return client.request(
        `/v1/internal/gateway/usage-aggregates?${query.toString()}`,
        consoleUsageAggregateResponseSchema,
        { managementToken },
      );
    },
    getRuntimePressure: (managementToken) =>
      client.request("/v1/internal/gateway/pressure", consoleRuntimePressureResponseSchema, {
        managementToken,
      }),
    getCostOverview: (managementToken) =>
      client.request("/v1/internal/gateway/costs", consoleCostOverviewResponseSchema, {
        managementToken,
      }),
    listProviderCredentialModelStates: (managementToken, params) => {
      const query = new URLSearchParams({ limit: String(params?.limit ?? 1000) });
      return client.request(
        `/v1/internal/gateway/provider-credential-model-states?${query.toString()}`,
        consoleProviderCredentialModelStateResponseSchema,
        { managementToken },
      );
    },
    getRequestAuditSummary: (managementToken, params) => {
      const query = new URLSearchParams({ limit: String(params?.limit ?? 1000), includeModelTotals: "true" });
      if (params?.createdFrom) {
        query.set("createdFrom", params.createdFrom);
      }
      return client.request(
        `/v1/internal/gateway/requests/summary?${query.toString()}`,
        consoleRequestAuditSummaryResponseSchema,
        { managementToken },
      );
    },
  };
}
