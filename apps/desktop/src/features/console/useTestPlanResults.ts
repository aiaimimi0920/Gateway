import { useCallback, useEffect, useRef, useState } from "react";
import type { ConsoleCredentialProbeResult, ConsoleProviderProbeResponse } from "../../api/contracts";
import type { TestResultLoader } from "./useCredentialTestRuns";
import type { TestPlanRow } from "./credentialTestPlansDocument";

export const TEST_RESULT_REFRESH_MS = 5_000;

// Polls are serial, bounded and paused in the editor/during model calls. Auth owns request aborts.
export function useTestPlanResults(providerIds: string[], loader: TestResultLoader | undefined, enabled: boolean) {
  const generation = useRef(0);
  const inFlight = useRef(false);
  const expanded = useRef(new Map<string, TestPlanRow>());
  const [responses, setResponses] = useState<ConsoleProviderProbeResponse[]>([]);
  const [summaryResponses, setSummaryResponses] = useState<ConsoleProviderProbeResponse[]>([]);
  const lastSummaries = useRef(new Map<string, ConsoleProviderProbeResponse>());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const key = JSON.stringify(providerIds);
  useEffect(() => { expanded.current.clear(); lastSummaries.current.clear(); setResponses([]); setSummaryResponses([]); }, [key]);
  useEffect(() => {
    const current = ++generation.current;
    if (!enabled || !loader) { setLoading(false); return; }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = async () => {
      if (current !== generation.current) return;
      if (typeof document !== "undefined" && document.visibilityState === "hidden") {
        timer = setTimeout(() => void load(), TEST_RESULT_REFRESH_MS);
        return;
      }
      if (inFlight.current) { timer = setTimeout(() => void load(), TEST_RESULT_REFRESH_MS); return; }
      inFlight.current = true; setLoading(true);
      try {
        const refreshed = new Map(lastSummaries.current);
        const details: ConsoleProviderProbeResponse[] = [];
        const replaced: [string, string][] = [];
        let firstFailure: string | null = null;
        const targetProviders = providerIds.slice(0, 32);
        for (let idx = 0; idx < targetProviders.length; idx += 4) {
          const batch = targetProviders.slice(idx, idx + 4);
          const settled = await Promise.allSettled(
            batch.map((providerId) => loader(providerId, undefined, { includePlans: true })),
          );
          if (current !== generation.current) return;
          for (const [i, item] of settled.entries()) {
            if (item.status === "fulfilled" && item.value) refreshed.set(batch[i], item.value);
            else if (!firstFailure) firstFailure = item.status === "rejected" ? (item.reason instanceof Error ? item.reason.message : String(item.reason)) : "测试结果暂不可用，将自动重试。";
          }
        }
        const expandedRows = [...expanded.current.values()].filter(
          (row) => providerIds.includes(row.providerId) && !row.legacy,
        );
        for (let idx = 0; idx < expandedRows.length; idx += 4) {
          const batch = expandedRows.slice(idx, idx + 4);
          const settled = await Promise.allSettled(
            batch.map((row) => loader(row.providerId, undefined, { planId: row.id })),
          );
          if (current !== generation.current) return;
          for (const [i, item] of settled.entries()) {
            if (item.status === "fulfilled" && item.value) {
              details.push(item.value);
              replaced.push([batch[i].providerId, batch[i].id]);
            }
            else if (!firstFailure) firstFailure = item.status === "rejected" ? (item.reason instanceof Error ? item.reason.message : String(item.reason)) : "计划结果暂不可用，将自动重试。";
          }
        }
        lastSummaries.current = refreshed;
        const summary = targetProviders.flatMap((id) => refreshed.get(id) ?? []);
        setSummaryResponses(summary);
        setResponses(mergeResponses(details, summary, replaced));
        setError(firstFailure);
      } catch (cause) { if (current === generation.current) setError(cause instanceof Error ? cause.message : String(cause)); }
      finally {
        inFlight.current = false;
        if (current === generation.current) {
          setLoading(false);
          timer = setTimeout(() => void load(), TEST_RESULT_REFRESH_MS);
        }
      }
    };
    void load();
    return () => { generation.current++; if (timer !== undefined) clearTimeout(timer); };
  }, [key, loader, enabled, refresh]);
  const expand = useCallback(async (row: TestPlanRow, open = true) => {
    if (!open) { expanded.current.delete(row.key); return; }
    if (row.legacy || !loader || !enabled) return;
    if (expanded.current.size >= 128 && !expanded.current.has(row.key)) return;
    expanded.current.set(row.key, row);
    if (inFlight.current) return;
    const current = generation.current;
    inFlight.current = true; setLoading(true);
    try {
      const response = await loader(row.providerId, undefined, { planId: row.id });
      if (current === generation.current && response) { setResponses((previous) => mergeResponses([response], previous, [[row.providerId, row.id]])); setError(null); }
    } catch (cause) { if (current === generation.current) setError(cause instanceof Error ? cause.message : String(cause)); }
    finally { inFlight.current = false; if (current === generation.current) setLoading(false); }
  }, [loader, enabled]);
  return { responses, summaryResponses, summaryResults: summaryResponses.flatMap((entry) => entry.result.results),
    results: responses.flatMap((entry) => entry.result.results), loading, error, expand,
    refresh: () => setRefresh((value) => value + 1) };
}

function mergeResponses(details: ConsoleProviderProbeResponse[], summary: ConsoleProviderProbeResponse[], replaced: [string, string][]) {
  return boundResponses([...details, ...summary.map((response) => ({ ...response, result: { ...response.result,
    results: response.result.results.filter((row) => !replaced.some(([providerId, planId]) => row.providerId === providerId && row.assessment?.planId === planId)),
  } }))]);
}

function boundResponses(responses: ConsoleProviderProbeResponse[]) {
  const counts = new Map<string, number>();
  const bounded = responses.map((response) => {
    const used = counts.get(response.result.providerId) ?? 0;
    const rows = response.result.results.slice(0, Math.max(0, 128 - used));
    counts.set(response.result.providerId, used + rows.length);
    return { ...response, result: { ...response.result, results: rows,
      truncated: response.result.truncated || rows.length < response.result.results.length } };
  });
  const truncated = bounded.some((response) => response.result.truncated);
  const retained = bounded.filter((response) => response.result.results.length > 0);
  if (retained[0] && truncated) retained[0].result.truncated = true;
  return retained;
}

export function testPlanResultRows(row: TestPlanRow, results: ConsoleCredentialProbeResult[]) {
  return results.filter((result) => {
    if (result.providerId !== row.providerId) return false;
    if (!row.legacy) return result.assessment?.planId === row.id;
    if (result.assessment?.planId) return false;
    const scope = row.scopes[0];
    if (scope.kind === "account" && result.credentialId !== scope.id) return false;
    const source = scope.kind === "pool" ? "pool" : scope.kind === "account" ? "account" : `subpool:${scope.id}`;
    return result.assessment?.policySource === source || result.assessment?.policySource === `${source}:temporary`;
  });
}
