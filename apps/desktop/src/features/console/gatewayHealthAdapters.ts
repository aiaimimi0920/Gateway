import type { ProbePolicy } from "./siteHealth";

/** Project families are user choices; legacy wire IDs remain pinned internal readers. */
export const gatewayHealthAdapters = {
  "new-api": "New API",
  sub2api: "sub2api",
  "one-api": "One API",
  "compatible-gateway": "兼容网关（类型未确认）",
} as const;

const legacyFamilies: Record<string, keyof typeof gatewayHealthAdapters> = {
  "new-api-perf-summary": "new-api",
  "sub2api-v2": "sub2api",
  "model-status-all": "compatible-gateway",
  "channel-health": "compatible-gateway",
  "model-status-embed": "compatible-gateway",
  "model-health-slot": "compatible-gateway",
};

export function gatewayHealthFamily(adapter: ProbePolicy["adapter"]): ProbePolicy["adapter"] {
  return Object.hasOwn(legacyFamilies, adapter) ? legacyFamilies[adapter] : adapter;
}

export function legacyHealthReader(adapter: ProbePolicy["adapter"]): string | null {
  return Object.hasOwn(legacyFamilies, adapter) ? adapter : null;
}
