import {
  gatewayDesktopCommands,
  type GatewayDesktopCommands,
} from "../lib/tauri";
import type { GatewayProfile } from "../lib/types";
import type { GatewayHostAdapter } from "./types";

const TAURI_CAPABILITIES = {
  sidecarLifecycle: true,
  profileManagement: true,
  localFileAccess: true,
  nativeLogAccess: true,
} as const;

const LOOPBACK_HOSTS = new Set(["127.0.0.1", "localhost", "[::1]"]);

function normalizeLoopbackOrigin(value: string): string {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new Error("Tauri Gateway API origin must be a valid loopback URL.");
  }

  if (!LOOPBACK_HOSTS.has(url.hostname.toLowerCase())) {
    throw new Error("Tauri Gateway API origin must use a loopback host.");
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new Error("Tauri Gateway API origin must use HTTP or HTTPS.");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error("Tauri Gateway API origin cannot contain credentials, query, or fragment data.");
  }
  if (url.pathname !== "/") {
    throw new Error("Tauri Gateway API origin cannot contain a path.");
  }

  return url.origin;
}

export type TauriHostOptions = {
  apiOrigin?: string;
  selectedProfile?: Pick<GatewayProfile, "port">;
  desktop?: GatewayDesktopCommands;
};

function loopbackOriginFromPort(port: number): string {
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    throw new Error("Selected Gateway profile must contain a valid port.");
  }
  return `http://127.0.0.1:${port}`;
}

export function createTauriHost(input: string | TauriHostOptions): GatewayHostAdapter {
  const options = typeof input === "string" ? { apiOrigin: input } : input;
  const apiOrigin =
    options.apiOrigin ??
    (options.selectedProfile ? loopbackOriginFromPort(options.selectedProfile.port) : undefined);
  if (!apiOrigin) {
    throw new Error("Tauri Gateway host requires a selected profile or explicit sidecar origin.");
  }
  return {
    kind: "tauri",
    apiOrigin: normalizeLoopbackOrigin(apiOrigin),
    routing: { mode: "hash" },
    capabilities: TAURI_CAPABILITIES,
    desktop: options.desktop ?? gatewayDesktopCommands,
  };
}
