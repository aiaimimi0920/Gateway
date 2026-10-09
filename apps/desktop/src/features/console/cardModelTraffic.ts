import type { ConsoleTelemetrySnapshot } from "./telemetry";
import { aggregateSuccessWindows, type ProviderSuccessWindow } from "./providerCardMetrics";

export type CardModelTrafficEntry = {
  requestCount: number | null;
  successCount: number | null;
  successWindows: readonly ProviderSuccessWindow[];
};

export type CardModelTraffic = {
  models: ReadonlyMap<string, CardModelTrafficEntry>;
  emptyRequestCount: 0 | null;
};

/** Company metrics sum only its visible models, never averages of percentages. */
export function aggregateCardModelTraffic(models: readonly string[], traffic?: CardModelTraffic): CardModelTrafficEntry {
  const entries = [...new Set(models)].map((model) => traffic?.models.get(model));
  const requests = entries.map((entry) => entry ? entry.requestCount : traffic?.emptyRequestCount ?? null);
  const successes = entries.map((entry, index) => entry?.successCount ?? (requests[index] === 0 ? 0 : null));
  const sumComplete = (values: readonly (number | null)[]) =>
    values.some((value) => value === null) ? null : values.reduce<number>((sum, value) => sum + (value ?? 0), 0);
  return {
    requestCount: sumComplete(requests),
    successCount: sumComplete(successes),
    successWindows: aggregateSuccessWindows(entries.map((entry) => ({ successWindows: [...entry?.successWindows ?? []] }))),
  };
}

export function providerCardModelTraffic(
  snapshot: ConsoleTelemetrySnapshot,
  providerIds: readonly string[],
): CardModelTraffic {
  const sources = [...new Set(providerIds)].map((id) => snapshot.providerAccounts.get(id));
  const models = new Set(sources.flatMap((source) => [...source?.modelStats.keys() ?? []]));
  const retained = retainedTraffic(snapshot, new Set(providerIds));
  if (retained) return joinRecentWindows(retained, sources.flatMap((source) => [...source?.modelStats ?? []]));
  return {
    emptyRequestCount: snapshot.hasCosts ? 0 : null,
    models: new Map([...models].map((model) => {
      const entries = sources.map((source) => source?.modelStats.get(model));
      // Missing cost endpoint or an audit-only row cannot prove a zero retained total.
      const complete = snapshot.hasCosts && entries.every((entry) => !entry || entry.requestCount !== null);
      return [model, {
        requestCount: complete ? entries.reduce((total, entry) => total + (entry?.requestCount ?? 0), 0) : null,
        successCount: null,
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
  const retained = retainedTraffic(snapshot, new Set([providerId]), credentialRef);
  const recent = (stats?.models ?? []).map((model) => [model.model, {
      successWindows: model.windows.map((window) => ({
        label: window.bucketStart, success: window.successCount, requests: window.totalRequests,
      })),
    }] as const);
  return joinRecentWindows(retained ?? { models: new Map(), emptyRequestCount: null }, recent);
}

function retainedTraffic(snapshot: ConsoleTelemetrySnapshot, providerIds: ReadonlySet<string>, credentialRef?: string): CardModelTraffic | null {
  if (snapshot.retainedModelTotals === undefined) return null;
  const models = new Map<string, CardModelTrafficEntry>();
  for (const row of snapshot.retainedModelTotals) {
    // Missing attribution can count for the pool, never for an individual credential.
    if (!providerIds.has(row.providerAccountId) || (credentialRef !== undefined && row.credentialRef !== credentialRef)) continue;
    const current = models.get(row.model);
    models.set(row.model, {
      requestCount: (current?.requestCount ?? 0) + row.requestCount,
      successCount: (current?.successCount ?? 0) + row.successCount,
      successWindows: [],
    });
  }
  return { models, emptyRequestCount: 0 };
}

function joinRecentWindows(traffic: CardModelTraffic, recent: readonly (readonly [string, { successWindows: readonly ProviderSuccessWindow[] }])[]): CardModelTraffic {
  const models = new Map(traffic.models);
  for (const [model, entry] of recent) {
    const current = models.get(model);
    models.set(model, {
      requestCount: current?.requestCount ?? traffic.emptyRequestCount,
      successCount: current?.successCount ?? (traffic.emptyRequestCount === 0 ? 0 : null),
      successWindows: aggregateSuccessWindows([{ successWindows: [...current?.successWindows ?? []] }, { successWindows: [...entry.successWindows] }]),
    });
  }
  return { ...traffic, models };
}
