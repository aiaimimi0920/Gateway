import { useEffect, useRef } from "react";
import type { ConsoleApi } from "../../api/console";
import type { ConsoleRouteConfigResponse } from "../../api/contracts";

type Options = {
  api: ConsoleApi;
  managementToken: string | null;
  routeConfig: ConsoleRouteConfigResponse | null;
  editorText: string;
  blocked: boolean;
  setRouteConfig(value: ConsoleRouteConfigResponse): void;
};

export function hasPendingDiscovery(config: ConsoleRouteConfigResponse | null): boolean {
  return !!config?.routeConfig.document.providers.some((raw) => {
    const provider = raw as { credentials?: { discovery_job?: { status?: string } }[] } | null;
    return provider?.credentials?.some((c) => c.discovery_job?.status === "pending");
  });
}

/** Poll only saved discovery intents, without locking the editor or replacing in-progress edits. */
export function useBackgroundDiscoverySync(options: Options | null) {
  const current = useRef(options); current.current = options;
  const pending = hasPendingDiscovery(options?.routeConfig ?? null);
  useEffect(() => {
    if (!pending || !options?.managementToken) return;
    let disposed = false;
    let active = false;
    const timer = window.setInterval(async () => {
      const before = current.current;
      if (active || !before || before.blocked || document.querySelector('[role="dialog"]')) return;
      active = true;
      try {
        const result = await before.api.getRouteConfig(before.managementToken!);
        const after = current.current;
        if (!disposed && after && !after.blocked && !document.querySelector('[role="dialog"]') && after.editorText === before.editorText &&
          after.api === before.api && after.managementToken === before.managementToken &&
          after.routeConfig === before.routeConfig &&
          result.routeConfig.revision.id !== before.routeConfig?.routeConfig.revision.id) {
          after.setRouteConfig(result);
        }
      } catch { /* A temporary read failure does not cancel the server-owned job. */ }
      finally { active = false; }
    }, 3000);
    return () => { disposed = true; window.clearInterval(timer); };
  }, [options?.api, options?.managementToken, pending]);
}
