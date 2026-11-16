import { vi } from "vitest";
import { createProfileFromTemplate } from "../lib/profileTemplates";
import { validateGatewayProfile } from "../lib/profileValidation";
import type { GatewayDesktopState } from "../lib/types";

/** Complete synthetic state for isolated legacy desktop panel contracts. */
export function createDesktopPanelState(patch: Partial<GatewayDesktopState> = {}): GatewayDesktopState {
  const draftProfile = patch.draftProfile ?? createProfileFromTemplate("local-default");
  return {
    profileNames: ["local-default", "local-debug"], selectedProfileName: "local-default", draftProfile,
    processSnapshot: {
      running: false, pid: null, port: null, profileName: null, logPath: null, startedAt: null,
      startupState: null, shutdownState: null, lastError: null, recentLogLines: []
    },
    logTail: { path: null, lines: [] }, profileTransferText: "", importProfileText: "", onboardingSteps: [],
    hasUnsavedProfileChanges: false, startWillSaveDraftProfile: false, busy: false,
    apiTestInput: { apiKey: "", model: "synthetic-model", message: "synthetic message" },
    profileValidation: validateGatewayProfile(draftProfile), canSaveProfile: true, canStartGateway: true,
    baseUrl: "http://127.0.0.1:1", isTauriAvailable: true,
    refreshAll: vi.fn(async () => undefined), reloadProfiles: vi.fn(async () => undefined), selectProfile: vi.fn(async () => undefined),
    updateDraftProfile: vi.fn(), saveDraftProfile: vi.fn(async () => undefined), deleteSelectedProfile: vi.fn(async () => undefined),
    startGateway: vi.fn(async () => undefined), stopGateway: vi.fn(async () => undefined), refreshRuntimeSignals: vi.fn(async () => undefined),
    refreshModels: vi.fn(async () => undefined), refreshLogs: vi.fn(async () => undefined), openLogDirectory: vi.fn(async () => undefined),
    copyCurrentLogPath: vi.fn(async () => undefined), copyDiagnostics: vi.fn(async () => undefined), exportDraftProfile: vi.fn(async () => undefined),
    importDraftProfile: vi.fn(async () => undefined), updateImportProfileText: vi.fn(), checkProfilePaths: vi.fn(async () => undefined),
    applyProfileTemplate: vi.fn(), updateApiTestInput: vi.fn(), runApiTest: vi.fn(async () => undefined), clearNotice: vi.fn(),
    ...patch,
  };
}
