import type {
  BootstrapStatus,
  ConsoleAccountGroupSummaryResponse,
  ConsoleCredentialProbeResponse,
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
