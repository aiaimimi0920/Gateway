import type {
  BootstrapStatus,
  ConsoleAccountGroupSummaryResponse,
  ConsoleCredentialProbeResponse,
  ConsoleCredentialPoolAutomationResponse,
  ConsoleCredentialPoolAutomationRunResponse,
  ConsoleCredentialRefillRequestResponse,
  ConsoleCredentialRefillResponse,
  ConsoleGeminiAuthSessionRequest,
  ConsoleGeminiAuthSessionResponse,
  ConsoleRouteConfigCommitRequest,
  ConsoleRouteConfigCommitResponse,
  ConsoleRouteConfigResponse,
  ConsoleRouteConfigValidationRequest,
  ConsoleRouteConfigValidationResponse,
  ConsoleRouteRevisionDetailResponse,
  ConsoleRouteRevisionListResponse,
  ManagementSession,
  OperationSuccess,
  SecretGrant,
} from "./contracts";
import type { GatewayApiClient } from "./client";
import {
  bootstrapStatusSchema,
  consoleAccountGroupSummaryResponseSchema,
  consoleCredentialProbeResponseSchema,
  consoleCredentialPoolAutomationResponseSchema,
  consoleCredentialPoolAutomationRunResponseSchema,
  consoleCredentialRefillRequestResponseSchema,
  consoleCredentialRefillResponseSchema,
  consoleGeminiAuthSessionResponseSchema,
  consoleRouteConfigCommitResponseSchema,
  consoleRouteConfigResponseSchema,
  consoleRouteConfigValidationResponseSchema,
  consoleRouteRevisionDetailResponseSchema,
  consoleRouteRevisionListResponseSchema,
  managementSessionSchema,
  operationSuccessSchema,
  secretGrantSchema,
} from "./schemas";

const CONSOLE_ROOT = "/v1/internal/gateway/console";

export type ConsoleApi = {
  getBootstrapStatus(): Promise<BootstrapStatus>;
  bootstrap(token: string): Promise<OperationSuccess>;
  verifySession(token: string): Promise<ManagementSession>;
  confirmSecretAccess(managementToken: string, confirmationToken: string): Promise<SecretGrant>;
  rotateSession(currentToken: string, newToken: string): Promise<OperationSuccess>;
  logout(token: string): Promise<OperationSuccess>;
  getRouteConfig(managementToken: string): Promise<ConsoleRouteConfigResponse>;
  getAccountGroupSummary(managementToken: string): Promise<ConsoleAccountGroupSummaryResponse>;
  getCredentialPoolAutomation(
    managementToken: string,
  ): Promise<ConsoleCredentialPoolAutomationResponse>;
  runCredentialPoolAutomation(
    managementToken: string,
    providerId: string,
  ): Promise<ConsoleCredentialPoolAutomationRunResponse>;
  getCredentialRefill(managementToken: string): Promise<ConsoleCredentialRefillResponse>;
  requestCredentialRefill(
    managementToken: string,
    providerId: string,
    requestedCount?: number,
  ): Promise<ConsoleCredentialRefillRequestResponse>;
  createGeminiAuthSession(
    managementToken: string,
    request: ConsoleGeminiAuthSessionRequest,
  ): Promise<ConsoleGeminiAuthSessionResponse>;
  completeGeminiAuthSession(
    managementToken: string,
    sessionId: string,
  ): Promise<ConsoleGeminiAuthSessionResponse>;
  getGeminiAuthSession(
    managementToken: string,
    sessionId: string,
  ): Promise<ConsoleGeminiAuthSessionResponse>;
  probeCredential(
    managementToken: string,
    secretGrant: string,
    credentialId: string,
  ): Promise<ConsoleCredentialProbeResponse>;
  validateRouteConfig(
    managementToken: string,
    draft: ConsoleRouteConfigValidationRequest,
    secretGrant?: string,
  ): Promise<ConsoleRouteConfigValidationResponse>;
  commitRouteConfig(
    managementToken: string,
    draft: ConsoleRouteConfigCommitRequest,
    secretGrant?: string,
  ): Promise<ConsoleRouteConfigCommitResponse>;
  listRouteConfigRevisions(managementToken: string): Promise<ConsoleRouteRevisionListResponse>;
  getRouteConfigRevision(
    managementToken: string,
    revisionId: string,
  ): Promise<ConsoleRouteRevisionDetailResponse>;
};

export function createConsoleApi(client: GatewayApiClient): ConsoleApi {
  return {
    getBootstrapStatus: () =>
      client.request(`${CONSOLE_ROOT}/bootstrap/status`, bootstrapStatusSchema),
    bootstrap: (token) =>
      client.request(`${CONSOLE_ROOT}/bootstrap`, operationSuccessSchema, {
        method: "POST",
        body: { token },
      }),
    verifySession: (token) =>
      client.request(`${CONSOLE_ROOT}/session/verify`, managementSessionSchema, {
        method: "POST",
        managementToken: token,
      }),
    confirmSecretAccess: (managementToken, confirmationToken) =>
      client.request(`${CONSOLE_ROOT}/session/confirm-secret-access`, secretGrantSchema, {
        method: "POST",
        managementToken,
        body: { token: confirmationToken },
      }),
    rotateSession: (currentToken, newToken) =>
      client.request(`${CONSOLE_ROOT}/session/rotate`, operationSuccessSchema, {
        method: "POST",
        managementToken: currentToken,
        body: { newToken },
      }),
    logout: (token) =>
      client.request(`${CONSOLE_ROOT}/session/logout`, operationSuccessSchema, {
        method: "POST",
        managementToken: token,
      }),
    getRouteConfig: (managementToken) =>
      client.request(`${CONSOLE_ROOT}/route-config`, consoleRouteConfigResponseSchema, {
        managementToken,
      }),
    getAccountGroupSummary: (managementToken) =>
      client.request(
        "/v1/internal/gateway/account-groups",
        consoleAccountGroupSummaryResponseSchema,
        {
          managementToken,
        },
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
        {
          method: "POST",
          managementToken,
        },
      ),
    getGeminiAuthSession: (managementToken, sessionId) =>
      client.request(
        `${CONSOLE_ROOT}/gemini-auth-sessions/${encodeURIComponent(sessionId)}`,
        consoleGeminiAuthSessionResponseSchema,
        {
          managementToken,
        },
      ),
    probeCredential: (managementToken, secretGrant, credentialId) =>
      client.request(
        `/v1/internal/gateway/console/credentials/${encodeURIComponent(credentialId)}/probe`,
        consoleCredentialProbeResponseSchema,
        {
          method: "POST",
          managementToken,
          secretGrant,
        },
      ),
    validateRouteConfig: (managementToken, draft, secretGrant) =>
      client.request(
        `${CONSOLE_ROOT}/route-config/validate`,
        consoleRouteConfigValidationResponseSchema,
        {
          method: "POST",
          managementToken,
          secretGrant,
          body: draft,
        },
      ),
    commitRouteConfig: (managementToken, draft, secretGrant) =>
      client.request(`${CONSOLE_ROOT}/route-config`, consoleRouteConfigCommitResponseSchema, {
        method: "PUT",
        managementToken,
        secretGrant,
        body: draft,
      }),
    listRouteConfigRevisions: (managementToken) =>
      client.request(`${CONSOLE_ROOT}/revisions`, consoleRouteRevisionListResponseSchema, {
        managementToken,
      }),
    getRouteConfigRevision: (managementToken, revisionId) =>
      client.request(
        `${CONSOLE_ROOT}/revisions/${encodeURIComponent(revisionId)}`,
        consoleRouteRevisionDetailResponseSchema,
        {
          managementToken,
        },
      ),
  };
}
