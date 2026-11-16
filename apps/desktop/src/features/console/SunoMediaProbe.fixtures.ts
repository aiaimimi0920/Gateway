import type { ComponentProps } from "react";
import { vi } from "vitest";
import { PilotActionDialog } from "./PilotActionDialog";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";

export function sunoProbeProps(): ComponentProps<typeof PilotActionDialog> {
  const account = pilotAccount({ accountId: "exact", providerId: "suno", displayName: "Synthetic Suno" });
  const section = pilotSection({ providerId: "suno", providerIds: ["suno"], providerPreset: "suno", directAccounts: [account] });
  return { t: zh => zh, pilotActionDialog: { kind: "probe", providerId: "suno", account }, accountPilotSections: [section],
    testPolicyDocument: { providers: [{ id: "suno", preset: "suno", base_url: "https://studio-api-prod.suno.com",
      call_flow: { special_adapter: "suno_web" }, supported_models: ["chirp-v3-5", "chirp-auk"],
      model_map: { "my-song": "chirp-v3-5" }, credentials: [{ id: "exact", supported_models: ["chirp-v3-5", "chirp-auk"] }] }], model_routes: [], aliases: {} },
    closePilotActionDialog: vi.fn(), credentialProbeBusy: null, activePilotManagedAccount: null, activePilotProbeResult: null,
    draftDirty: false, draftMatchesActiveRevision: true, activeRevision: "synthetic-r1", startPilotProbe: vi.fn(), providerProbeBusy: false, providerProbeError: null, providerProbeResponse: null,
    startProviderProbe: vi.fn().mockResolvedValue(undefined), editorLocked: false, pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(),
    pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn(), telemetryError: null,
    activePilotStatsView: null, applyPilotProbeSchedule: vi.fn() };
}
