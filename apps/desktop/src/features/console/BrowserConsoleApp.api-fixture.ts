import { vi } from "vitest";
import type { ConsoleApi } from "../../api/console";

// Multi-edit scenarios need refresh to return the previously committed document.
// Secret-vault behavior is outside this fixture; assertions inspect request patches.
export async function preserveCommittedRouteDocument(consoleApi: ConsoleApi): Promise<void> {
  let current = (await consoleApi.getRouteConfig("management-secret")).routeConfig;
  vi.mocked(consoleApi.commitRouteConfig).mockImplementation(async (_token, request) => {
    const sequence = current.revision.sequence + 1;
    current = {
      ...current,
      document: structuredClone(request.document),
      revision: { id: `fixture-r${sequence}`, sequence, message: "fixture autosave" },
    };
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({ routeConfig: current });
    return { routeConfig: current, committed: true };
  });
}

export function createConsoleApi(): ConsoleApi {
  return {
    getBootstrapStatus: vi.fn(),
    bootstrap: vi.fn(),
    verifySession: vi.fn(),
    confirmSecretAccess: vi.fn(),
    rotateSession: vi.fn(),
    logout: vi.fn(),
    probeCredential: vi.fn().mockResolvedValue({
      result: {
        credentialId: "acc-prod-1",
        providerId: "managed-provider",
        probePoint: "GET https://managed.example/v1/models",
        status: "passed",
        message: "Credential connectivity probe passed.",
        checkedAt: "2026-07-28T02:00:00Z",
      },
    }),
    probeProvider: vi.fn().mockResolvedValue({
      result: {
        providerId: "managed-provider",
        status: "passed",
        message: "Provider test completed: 1 passed, 0 failed, 0 unsupported.",
        checkedAt: "2026-07-28T02:00:00Z",
        totalCount: 1,
        passedCount: 1,
        failedCount: 0,
        unsupportedCount: 0,
        results: [
          {
            credentialId: "acc-prod-1",
            providerId: "managed-provider",
            probePoint: "GET https://managed.example/v1/models",
            status: "passed",
            message: "Credential connectivity probe passed.",
            checkedAt: "2026-07-28T02:00:00Z",
          },
        ],
      },
    }),
    getCredentialUsage: vi.fn().mockResolvedValue({ buckets: [] }),
    getAccountGroupSummary: vi.fn().mockRejectedValue(new Error("summary unavailable")),
    getCredentialPoolAutomation: vi.fn().mockRejectedValue(new Error("automation unavailable")),
    runCredentialPoolAutomation: vi.fn(),
    pruneCredentialPool: vi.fn(),
    purgeCredentialArchive: vi.fn(),
    getCredentialRefill: vi.fn().mockRejectedValue(new Error("refill unavailable")),
    requestCredentialRefill: vi.fn(),
    createGeminiAuthSession: vi.fn().mockResolvedValue({
      session: {
        id: "gemini-auth-session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Complete Gemini login in the opened browser window.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:00:00Z",
        generatedDrafts: [],
      },
    }),
    completeGeminiAuthSession: vi.fn().mockResolvedValue({
      session: {
        id: "gemini-auth-session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Manual Gemini import requested. Finishing capture.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:00:01Z",
        generatedDrafts: [],
      },
    }),
    getGeminiAuthSession: vi.fn().mockResolvedValue({
      session: {
        id: "gemini-auth-session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Complete Gemini login in the opened browser window.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:00:00Z",
        generatedDrafts: [],
      },
    }),
    commitRouteConfig: vi.fn().mockResolvedValue({
      routeConfig: {
        revision: { id: "r2-beadfeedcafe", sequence: 2, message: "save update" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [{ id: "managed-provider" }],
          model_routes: [{ pattern: "gpt-5.4" }],
          aliases: { answer: "gpt-5.4" },
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
      committed: true,
    }),
    getRouteConfig: vi.fn().mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [{ id: "managed-provider" }],
          model_routes: [{ pattern: "gpt-5.4" }],
          aliases: { answer: "gpt-5.4" },
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    }),
    validateRouteConfig: vi.fn().mockResolvedValue({
      validation: {
        document: {
          providers: [{ id: "managed-provider" }],
          model_routes: [{ pattern: "gpt-5.4" }],
          aliases: { answer: "gpt-5.4" },
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
      },
    }),
    listRouteConfigRevisions: vi.fn().mockResolvedValue({
      revisions: [
        {
          revision: {
            id: "r1-deadbeefcafe",
            sequence: 1,
            message: "initial import",
          },
          active: true,
          hasArchive: true,
          source: "redis",
        },
        {
          revision: {
            id: "r0-cafebabefeed",
            sequence: 0,
            message: "seed route",
          },
          active: false,
          hasArchive: true,
          source: "archived",
        },
      ],
    }),
    getRouteConfigRevision: vi.fn().mockResolvedValue({
      routeConfig: {
        revision: {
          id: "r0-cafebabefeed",
          sequence: 0,
          message: "seed route",
        },
        source: "archived",
        diagnostics: null,
        requiresRepair: false,
        document: {
          providers: [{ id: "legacy-provider" }],
          model_routes: [{ pattern: "gpt-4.1" }],
          aliases: { answer: "gpt-4.1" },
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
      active: false,
      hasArchive: true,
    }),
  } as unknown as ConsoleApi;
}
