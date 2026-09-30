import type { ConsoleTelemetrySnapshot } from "./telemetry";
import { aggregateSuccessWindows, type ProviderSuccessWindow } from "./providerCardMetrics";

export type CardModelTrafficEntry = {
  requestCount: number | null;
  successWindows: readonly ProviderSuccessWindow[];
};

export type CardModelTraffic = {
  models: ReadonlyMap<string, CardModelTrafficEntry>;
  emptyRequestCount: 0 | null;
  countScope: "retained" | "recent";
};

export function providerCardModelTraffic(
  snapshot: ConsoleTelemetrySnapshot,
  providerIds: readonly string[],
): CardModelTraffic {
  const sources = [...new Set(providerIds)].map((id) => snapshot.providerAccounts.get(id));
  const models = new Set(sources.flatMap((source) => [...source?.modelStats.keys() ?? []]));
  return {
    countScope: "retained",
    emptyRequestCount: snapshot.hasCosts ? 0 : null,
    models: new Map([...models].map((model) => {
      const entries = sources.map((source) => source?.modelStats.get(model));
      // Missing cost endpoint or an audit-only row cannot prove a zero retained total.
      const complete = snapshot.hasCosts && entries.every((entry) => !entry || entry.requestCount !== null);
      return [model, {
        requestCount: complete ? entries.reduce((total, entry) => total + (entry?.requestCount ?? 0), 0) : null,
        successWindows: aggregateSuccessWindows(entries.map((entry) => ({ successWindows: entry?.successWindows }))),
      }];
    })),
  };
}

export function credentialCardModelTraffic(
  snapshot: ConsoleTelemetrySnapshot,
  providerId: string,
  credentialRef: string,
): CardModelTraffic {
  const stats = snapshot.credentialAuditStats?.get(JSON.stringify([providerId, credentialRef]));
  return {
    countScope: "recent",
    emptyRequestCount: snapshot.credentialAuditStats === undefined ? null : 0,
    // Never attribute another credential's traffic or a whole provider's totals here.
    models: new Map((stats?.models ?? []).map((model) => [model.model, {
      requestCount: model.totalRequests,
      successWindows: model.windows.map((window) => ({
        label: window.label, success: window.successCount, requests: window.totalRequests,
      })),
    }])),
  };
}
