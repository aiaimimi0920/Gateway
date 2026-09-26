import type { GatewayApiClient } from "../client";
import {
  bootstrapStatusSchema,
  consoleAccountGroupSummaryResponseSchema,
  consoleRouteConfigResponseSchema,
  managementSessionSchema,
  operationSuccessSchema,
  secretGrantSchema,
} from "../schemas";
import type { ConsoleApi } from "./api";
import { CONSOLE_ROOT } from "./roots";

type ConsoleCoreApi = Pick<
  ConsoleApi,
  | "getBootstrapStatus"
  | "bootstrap"
  | "verifySession"
  | "confirmSecretAccess"
  | "rotateSession"
  | "logout"
  | "getRouteConfig"
  | "getAccountGroupSummary"
>;

export function createConsoleCoreApi(client: GatewayApiClient): ConsoleCoreApi {
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
        { managementToken },
      ),
  };
}
