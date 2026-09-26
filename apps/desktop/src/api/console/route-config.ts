import type { GatewayApiClient } from "../client";
import {
  consoleRouteConfigCommitResponseSchema,
  consoleRouteConfigValidationResponseSchema,
  consoleRouteRevisionDetailResponseSchema,
  consoleRouteRevisionListResponseSchema,
} from "../schemas";
import type { ConsoleApi } from "./api";
import { CONSOLE_ROOT } from "./roots";

type ConsoleRouteConfigApi = Pick<
  ConsoleApi,
  | "validateRouteConfig"
  | "commitRouteConfig"
  | "listRouteConfigRevisions"
  | "getRouteConfigRevision"
>;

export function createConsoleRouteConfigApi(client: GatewayApiClient): ConsoleRouteConfigApi {
  return {
    validateRouteConfig: (managementToken, draft, secretGrant) =>
      client.request(
        `${CONSOLE_ROOT}/route-config/validate`,
        consoleRouteConfigValidationResponseSchema,
        { method: "POST", managementToken, secretGrant, body: draft },
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
        { managementToken },
      ),
  };
}
