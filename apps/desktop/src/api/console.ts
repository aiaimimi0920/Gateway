import type {
  BootstrapStatus,
  ManagementSession,
  OperationSuccess,
  SecretGrant,
} from "./contracts";
import type { GatewayApiClient } from "./client";
import {
  bootstrapStatusSchema,
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
  };
}
