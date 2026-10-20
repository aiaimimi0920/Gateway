import { useCallback } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { parseRouteDocument } from "./routeDocument";
import { applySiteHealth, type SiteHealthConfig } from "./siteHealth";

export function useSiteHealthEditor(text: string, replace: (document: ConsoleRouteDocument, sync?: boolean) => void,
  setError: (error: string | null) => void) {
  return useCallback((id: string, config: SiteHealthConfig | null) => {
    try { replace(applySiteHealth(parseRouteDocument(text), id, config), true); setError(null); }
    catch (error) { setError(error instanceof Error ? error.message : String(error)); }
  }, [text, replace, setError]);
}
