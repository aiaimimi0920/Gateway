import { useEffect, useRef, useState } from "react";
import type { AccountDiscovery, DiscoverAccount, DiscoveryInput } from "./accountDiscovery";

/** Dialog cancellation fences late results; no API key is retained beyond its request. */
export function useDiscoverySubmission(open: boolean, discover?: DiscoverAccount) {
  const active = useRef<AbortController | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { setError(null); return () => { active.current?.abort(); active.current = null; setBusy(false); }; }, [open]);
  const run = async (input: DiscoveryInput, done: (discovery: AccountDiscovery) => void) => {
    if (!discover || active.current) return;
    const request = new AbortController(); active.current = request; setBusy(true); setError(null);
    try {
      const result = await discover(input, request.signal);
      if (!request.signal.aborted && active.current === request) done(result);
    } catch (cause) {
      if (!request.signal.aborted) setError(cause instanceof Error ? cause.message : String(cause));
    } finally { if (active.current === request) { active.current = null; setBusy(false); } }
  };
  return { busy, error, run };
}
