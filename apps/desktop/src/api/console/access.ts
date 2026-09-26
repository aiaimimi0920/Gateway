import type { GatewayApiClient } from "../client";
import {
  consoleAccessAffinityResponseSchema,
  consoleAccessBundleSchema,
  consoleAccessCatalogSchema,
  consoleAccessKeySchema,
  consoleApiAccessRotationSchema,
  consoleNullableAccessKeyBalanceSchema,
  consoleUserCredentialIssueResponseSchema,
  consoleVerifiedUserCredentialSchema,
  operationSuccessSchema,
} from "../schemas";
import type { ConsoleApi } from "./api";
import { buildQuery } from "./filters";
import { INTERNAL_ROOT } from "./roots";

type ConsoleAccessApi = Pick<
  ConsoleApi,
  | "getAccessCatalog"
  | "createAccessKey"
  | "rotateAccessKey"
  | "revokeAccessKey"
  | "getAccessKeyBalance"
  | "createAccessBundle"
  | "inspectAccessAffinity"
  | "resetAccessAffinity"
  | "rotateApiAccess"
  | "issueUserCredential"
  | "verifyUserCredential"
  | "revokeUserCredential"
>;

export function createConsoleAccessApi(client: GatewayApiClient): ConsoleAccessApi {
  return {
    getAccessCatalog: (managementToken) =>
      client.request(`${INTERNAL_ROOT}/access/catalog`, consoleAccessCatalogSchema, {
        managementToken,
      }),
    createAccessKey: (managementToken, input) =>
      client.request(`${INTERNAL_ROOT}/access/keys`, consoleAccessKeySchema, {
        method: "POST",
        managementToken,
        body: input,
      }),
    rotateAccessKey: (managementToken, accessKeyId) =>
      client.request(
        `${INTERNAL_ROOT}/access/keys/${encodeURIComponent(accessKeyId)}/rotate`,
        consoleAccessKeySchema,
        { method: "POST", managementToken },
      ),
    revokeAccessKey: (managementToken, accessKeyId, reason) =>
      client.request(
        `${INTERNAL_ROOT}/access/keys/${encodeURIComponent(accessKeyId)}/revoke`,
        operationSuccessSchema,
        { method: "POST", managementToken, body: { reason: reason ?? null } },
      ),
    getAccessKeyBalance: (managementToken, accessKeyId) =>
      client.request(
        `${INTERNAL_ROOT}/access/keys/${encodeURIComponent(accessKeyId)}/balance`,
        consoleNullableAccessKeyBalanceSchema,
        { managementToken },
      ),
    createAccessBundle: (managementToken, input) =>
      client.request(`${INTERNAL_ROOT}/access/bundles`, consoleAccessBundleSchema, {
        method: "POST",
        managementToken,
        body: input,
      }),
    inspectAccessAffinity: (managementToken, params) =>
      client.request(
        `${INTERNAL_ROOT}/access/affinity${buildQuery({ ...params })}`,
        consoleAccessAffinityResponseSchema,
        { managementToken },
      ),
    resetAccessAffinity: (managementToken, params) =>
      client.request(`${INTERNAL_ROOT}/access/affinity`, operationSuccessSchema, {
        method: "POST",
        managementToken,
        body: params,
      }),
    rotateApiAccess: (managementToken, input) =>
      client.request(`${INTERNAL_ROOT}/api-access/rotate`, consoleApiAccessRotationSchema, {
        method: "POST",
        managementToken,
        body: input,
      }),
    issueUserCredential: (managementToken, input) =>
      client.request(
        `${INTERNAL_ROOT}/user-credentials/issue`,
        consoleUserCredentialIssueResponseSchema,
        { method: "POST", managementToken, body: input },
      ),
    verifyUserCredential: (managementToken, credentialKey, scope) =>
      client.request(
        `${INTERNAL_ROOT}/user-credentials/verify`,
        consoleVerifiedUserCredentialSchema,
        { method: "POST", managementToken, body: { credentialKey, scope: scope ?? null } },
      ),
    revokeUserCredential: (managementToken, credentialKey, reason) =>
      client.request(`${INTERNAL_ROOT}/user-credentials/revoke`, operationSuccessSchema, {
        method: "POST",
        managementToken,
        body: { credentialKey, reason: reason ?? null },
      }),
  };
}
