export type GatewayUiTarget = "web" | "tauri";

export type GatewayHostKind = "browser" | "tauri";

export type GatewayHostCapabilities = Readonly<{
  sidecarLifecycle: boolean;
  profileManagement: boolean;
  localFileAccess: boolean;
  nativeLogAccess: boolean;
}>;

export type GatewayHostRouting =
  | Readonly<{ mode: "browser"; basename: "/ui" }>
  | Readonly<{ mode: "hash" }>;

export type GatewayHostAdapter = Readonly<{
  kind: GatewayHostKind;
  apiOrigin: string;
  routing: GatewayHostRouting;
  capabilities: GatewayHostCapabilities;
  desktop?: GatewayDesktopCommands;
}>;
import type { GatewayDesktopCommands } from "../lib/tauri";
