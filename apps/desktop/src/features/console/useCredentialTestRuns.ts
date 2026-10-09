import { useCallback, useEffect, useRef, useState } from "react";
import type { ConsoleProviderProbeRequest, ConsoleProviderProbeResponse, CredentialTestScope } from "../../api/contracts";
import type { TestInvocation } from "./credentialTestSelection";

export type TestProbeAction = (request?: ConsoleProviderProbeRequest, providerId?: string) => Promise<ConsoleProviderProbeResponse | void>;
export type TestResultLoader = (providerId: string, scope?: ConsoleProviderProbeRequest["scope"], query?: import("../../api/contracts").CredentialTestResultQuery) => Promise<ConsoleProviderProbeResponse | void>;

// One serial round belongs to this dialog. Closing or changing scope invalidates later sends.
export function useCredentialTestRuns(selectionKey: string, targets: CredentialTestScope[], onProbe: TestProbeAction, onLoadResults?: TestResultLoader, planId?: string) {
  const generation = useRef(0);
  const running = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [responses, setResponses] = useState<ConsoleProviderProbeResponse[]>([]);
  useEffect(() => {
    const current = ++generation.current;
    setResponses([]);
    setError(null);
    setBusy(false);
    const load = async () => {
      if (!onLoadResults || running.current || targets.length > 128) return;
      const received: ConsoleProviderProbeResponse[] = [];
      for (const target of targets) {
        if (current !== generation.current) return;
        const scope = target.kind === "pool" ? { kind: "pool" as const } : { kind: target.kind, id: target.id };
        const response = planId ? await onLoadResults(target.providerId, scope, { planId }) : await onLoadResults(target.providerId, scope);
        if (current !== generation.current) return;
        if (response) received.push(response);
      }
      if (current === generation.current) setResponses(received);
    };
    void load().catch(() => { /* The authenticated request owner renders read errors. */ });
    return () => { generation.current += 1; };
  }, [selectionKey, onLoadResults, planId]);

  const cancel = useCallback(() => { generation.current += 1; }, []);
  const run = async (invocations: TestInvocation[]) => {
    if (running.current || !invocations.length || invocations.length > 128) return;
    const current = ++generation.current;
    const started = performance.now();
    running.current = true;
    setBusy(true); setError(null); setResponses([]);
    const received: ConsoleProviderProbeResponse[] = [];
    try {
      for (const invocation of invocations) {
        if (current !== generation.current) return;
        if (performance.now() - started >= 300_000) { setError("本轮超时"); return; }
        const response = await onProbe(invocation.request, invocation.providerId);
        if (current !== generation.current) return;
        // Undefined means auth, stale identity, or request failure; never continue another target.
        if (!response) return;
        received.push(response);
        setResponses([...received]);
      }
    } catch (cause) {
      if (current === generation.current) setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      running.current = false;
      if (current === generation.current) setBusy(false);
    }
  };
  return { busy, error, responses, run, cancel };
}
