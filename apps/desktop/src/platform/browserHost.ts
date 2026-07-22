import type { GatewayHostAdapter } from "./types";

const BROWSER_CAPABILITIES = {
  sidecarLifecycle: false,
  profileManagement: false,
  localFileAccess: false,
  nativeLogAccess: false,
} as const;

export function createBrowserHost(location: Pick<Location, "origin"> = window.location): GatewayHostAdapter {
  const origin = location.origin.trim();
  if (origin.length === 0 || origin === "null") {
    throw new Error("Browser Gateway console requires an HTTP(S) origin.");
  }

  return {
    kind: "browser",
    apiOrigin: origin,
    routing: { mode: "browser", basename: "/ui" },
    capabilities: BROWSER_CAPABILITIES,
  };
}
