import type { ComponentProps } from "react";
import { vi } from "vitest";
import { PilotActionDialog } from "./PilotActionDialog";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";

export function udioProbeProps(): ComponentProps<typeof PilotActionDialog> {
  const account = pilotAccount({ accountId: "exact", providerId: "udio", displayName: "Synthetic Udio" });
  const section = pilotSection({ providerId: "udio", providerIds: ["udio"], providerPreset: "udio", directAccounts: [account] });
  return { t: zh => zh, pilotActionDialog: { kind: "probe", providerId: "udio", account }, accountPilotSections: [section],
    testPolicyDocument: { providers: [{ id: "udio", preset: "udio", base_url: "https://www.udio.com",
      call_flow: { special_adapter: "udio_web" }, supported_models: ["fixture-native"],
      model_map: { "my-song": "fixture-native" }, credentials: [{ id: "exact", supported_models: ["fixture-native"] }] }], model_routes: [], aliases: {} },
    closePilotActionDialog: vi.fn(), credentialProbeBusy: null, activePilotManagedAccount: null, activePilotProbeResult: null,
    draftDirty: false, draftMatchesActiveRevision: true, activeRevision: "synthetic-r1", startPilotProbe: vi.fn(), providerProbeBusy: false, providerProbeError: null, providerProbeResponse: null,
    startProviderProbe: vi.fn().mockResolvedValue(undefined), editorLocked: false, pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(),
    pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn(), telemetryError: null,
    activePilotStatsView: null, applyPilotProbeSchedule: vi.fn() };
}
