import type { ConsoleRouteDocument } from "../../api/contracts";

export function routeDocument(
  providers: unknown[],
  accountGroups: unknown[] = [],
): ConsoleRouteDocument {
  return {
    providers,
    model_routes: [],
    aliases: {},
    account_groups: accountGroups,
  };
}
