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
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const key = JSON.stringify(providerIds);
  useEffect(() => { expanded.current.clear(); setResponses([]); setSummaryResponses([]); }, [key]);
  useEffect(() => {
    const current = ++generation.current;
    if (!enabled || !loader) { setLoading(false); return; }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = async () => {
      if (current !== generation.current) return;
      if (inFlight.current) { timer = setTimeout(() => void load(), TEST_RESULT_REFRESH_MS); return; }
      inFlight.current = true; setLoading(true);
      try {
        const summary: ConsoleProviderProbeResponse[] = [];
        const details: ConsoleProviderProbeResponse[] = [];
        for (const providerId of providerIds.slice(0, 32)) {
          const response = await loader(providerId, undefined, { includePlans: true });
          if (current !== generation.current) return;
          if (!response) throw new Error("测试结果暂不可用，将自动重试。");
          summary.push(response);
        }
        for (const row of expanded.current.values()) {
          if (!providerIds.includes(row.providerId) || row.legacy) continue;
          const response = await loader(row.providerId, undefined, { planId: row.id });
          if (current !== generation.current) return;
          if (!response) throw new Error("计划结果暂不可用，将自动重试。");
          details.push(response);
        }
        setSummaryResponses(summary); setResponses(mergeResponses(details, summary, [...expanded.current.values()].filter((row) => !row.legacy).map((row) => [row.providerId, row.id]))); setError(null);
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
