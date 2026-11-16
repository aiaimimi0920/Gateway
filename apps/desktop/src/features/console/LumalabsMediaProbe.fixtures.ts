import type { ComponentProps } from "react";
import { vi } from "vitest";
import { PilotActionDialog } from "./PilotActionDialog";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";

export function lumalabsProbeProps(): ComponentProps<typeof PilotActionDialog> {
  const account = pilotAccount({ accountId: "exact", providerId: "lumalabs", displayName: "Synthetic Lumalabs" });
  const section = pilotSection({ providerId: "lumalabs", providerIds: ["lumalabs"], providerPreset: "lumalabs", directAccounts: [account] });
  return { t: zh => zh, pilotActionDialog: { kind: "probe", providerId: "lumalabs", account }, accountPilotSections: [section],
    testPolicyDocument: { providers: [{ id: "lumalabs", preset: "lumalabs", base_url: "https://app.lumalabs.ai",
      call_flow: { special_adapter: "lumalabs_web" }, supported_models: ["fixture-native"],
      model_map: { "my-image": "fixture-native" }, credentials: [{ id: "exact", supported_models: ["fixture-native"] }] }], model_routes: [], aliases: {} },
    closePilotActionDialog: vi.fn(), credentialProbeBusy: null, activePilotManagedAccount: null, activePilotProbeResult: null,
    draftDirty: false, draftMatchesActiveRevision: true, activeRevision: "synthetic-r1", startPilotProbe: vi.fn(), providerProbeBusy: false, providerProbeError: null, providerProbeResponse: null,
    startProviderProbe: vi.fn().mockResolvedValue(undefined), editorLocked: false, pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(),
    pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn(), telemetryError: null,
    activePilotStatsView: null, applyPilotProbeSchedule: vi.fn() };
}
