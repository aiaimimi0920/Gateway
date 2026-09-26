import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { modelRoutePriority } from "./modelPoolViewModel";


export type ModelRouteDraftRow = {
  id: string;
  pattern: string;
  route: Record<string, unknown>;
};


export function createModelRouteDraftRow(
  pattern = "",
  route: Record<string, unknown> = {},
): ModelRouteDraftRow {
  return {
    id: `route-${Math.random().toString(36).slice(2, 10)}`,
    pattern,
    route,
  };
}

export function modelRouteDraftRowsFromDocument(document: ConsoleRouteDocument): ModelRouteDraftRow[] {
  return document.model_routes.map((route) => {
    if (isRecord(route)) {
      return createModelRouteDraftRow(
        typeof route.pattern === "string" ? route.pattern : "",
        route,
      );
    }
    return createModelRouteDraftRow();
  });
}

/**
 * Rewrites — or drops, when `nextModel` is null — one model everywhere the
 * document names it outside `model_routes`: provider and credential
 * `supported_models` lists, each provider's `model_map`, and the alias table.
 * Without this a deleted model would keep its card, because the pool also
 * derives cards from what the accounts declare.
 */
export function rewriteModelReferences(
  document: ConsoleRouteDocument,
  model: string,
  nextModel: string | null,
): void {
  const rewriteSupportedModels = (owner: Record<string, unknown>) => {
    const list = owner.supported_models;
    if (!Array.isArray(list)) {
      return;
    }
    const next: string[] = [];
    for (const entry of list) {
      if (typeof entry !== "string") {
        continue;
      }
      if (entry.trim() !== model) {
        next.push(entry);
        continue;
      }
      if (nextModel) {
        next.push(nextModel);
      }
    }
    if (next.length > 0) {
      owner.supported_models = [...new Set(next)];
    } else {
      // An empty list would mean "serves nothing"; an absent one inherits.
      delete owner.supported_models;
    }
  };
  for (const provider of document.providers) {
    if (!isRecord(provider)) {
      continue;
    }
    rewriteSupportedModels(provider);
    const modelMap = provider.model_map;
    if (isRecord(modelMap) && Object.prototype.hasOwnProperty.call(modelMap, model)) {
      const mapped = modelMap[model];
      delete modelMap[model];
      if (nextModel) {
        // Model IDs are data keys, including names such as __proto__.
        Object.defineProperty(modelMap, nextModel, {
          value: mapped, enumerable: true, writable: true, configurable: true,
        });
      }
      if (Object.keys(modelMap).length === 0) {
        delete provider.model_map;
      }
    }
    if (Array.isArray(provider.credentials)) {
      for (const credential of provider.credentials) {
        if (isRecord(credential)) {
          rewriteSupportedModels(credential);
        }
      }
    }
  }
  for (const [alias, target] of Object.entries(document.aliases)) {
    if (typeof target !== "string" || target.trim() !== model) {
      continue;
    }
    if (nextModel) {
      document.aliases[alias] = nextModel;
    } else {
      delete document.aliases[alias];
    }
  }
}

/**
 * Declares the model on the providers its chain names so `/v1/models` lists it,
 * but only where the list already exists — an absent `supported_models` means
 * the provider inherits every model, and materialising one would narrow it.
 */
export function declareModelOnProviders(
  document: ConsoleRouteDocument,
  model: string,
  providerIds: readonly string[],
): void {
  const wanted = new Set(providerIds);
  for (const provider of document.providers) {
    if (!isRecord(provider)) {
      continue;
    }
    const providerId = typeof provider.id === "string" ? provider.id.trim() : "";
    if (!wanted.has(providerId)) {
      continue;
    }
    const list = provider.supported_models;
    if (!Array.isArray(list) || list.length === 0) {
      continue;
    }
    if (!list.some((entry) => typeof entry === "string" && entry.trim() === model)) {
      provider.supported_models = [...list, model];
    }
  }
}

/** The exact-pattern route that pins one model's provider chain, if any. */
export function findModelRoute(document: ConsoleRouteDocument, model: string): Record<string, unknown> | null {
  for (const route of document.model_routes) {
    if (isRecord(route) && typeof route.pattern === "string" && route.pattern.trim() === model) {
      return route;
    }
  }
  return null;
}

export function highestModelRoutePriority(document: ConsoleRouteDocument): number {
  return document.model_routes.reduce<number>(
    (highest, route) => (isRecord(route) ? Math.max(highest, modelRoutePriority(route)) : highest),
    0,
  );
}
