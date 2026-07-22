import { createBrowserHost } from "./browserHost";
import { createTauriHost } from "./tauriHost";
import type { GatewayHostAdapter, GatewayUiTarget } from "./types";
import type { GatewayProfile } from "../lib/types";

declare const __GATEWAY_UI_TARGET__: GatewayUiTarget | undefined;

export type CreateHostAdapterOptions = {
  target?: GatewayUiTarget;
  tauriApiOrigin?: string;
  selectedProfile?: Pick<GatewayProfile, "port">;
};

function configuredTarget(): GatewayUiTarget {
  if (typeof __GATEWAY_UI_TARGET__ !== "undefined") {
    return __GATEWAY_UI_TARGET__;
  }
  return "web";
}

export function createHostAdapter(options: CreateHostAdapterOptions = {}): GatewayHostAdapter {
  const target = options.target ?? configuredTarget();
  if (target === "tauri") {
    return createTauriHost({
      apiOrigin: options.tauriApiOrigin,
      selectedProfile: options.selectedProfile,
    });
  }
  return createBrowserHost();
}
