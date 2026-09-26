import type { GatewayApiClient } from "./client";
import { createConsoleAccessApi } from "./console/access";
import type { ConsoleApi } from "./console/api";
import { createConsoleCoreApi } from "./console/core";
import { createConsoleCredentialsApi } from "./console/credentials";
import { createConsoleOperationsApi } from "./console/operations";
import { createConsoleRouteConfigApi } from "./console/route-config";

export type { ConsoleApi } from "./console/api";
export type {
  ConsoleAnalysisExportFilters,
  ConsoleAnomalyFilters,
  ConsoleAnomalyPolicyFilters,
  ConsoleRemediationFilters,
  ConsoleRequestFilters,
} from "./console/filters";

/** Compose domain clients in the historical property insertion order. */
export function createConsoleApi(client: GatewayApiClient): ConsoleApi {
  return {
    ...createConsoleCoreApi(client),
    ...createConsoleCredentialsApi(client),
    ...createConsoleRouteConfigApi(client),
    ...createConsoleOperationsApi(client),
    ...createConsoleAccessApi(client),
  };
}
