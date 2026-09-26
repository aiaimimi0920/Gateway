import type { ReactNode } from "react";
import type { ConsoleApi } from "../api/console";
import type { ManagementSession } from "../api/contracts";
import { HostProvider } from "../platform/HostProvider";
import { createBrowserHost } from "../platform/browserHost";
import type { GatewayHostAdapter } from "../platform/types";
import { ManagementSessionProvider } from "./ManagementSessionProvider";
import { useManagementSession } from "./useManagementSession";
import { vi } from "vitest";

export const authenticatedSession: ManagementSession = {
  role: "administrator",
  capabilities: ["route-config:read", "route-config:write"],
  activeRevision: "r1-deadbeef",
  secretAccessGranted: false,
};

export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

export function createApi(overrides: Partial<ConsoleApi> = {}): ConsoleApi {
  return {
    getBootstrapStatus: vi.fn().mockResolvedValue({
      needsBootstrap: false,
      managementConfigured: true,
      environmentOverride: false,
    }),
    bootstrap: vi.fn().mockResolvedValue({ success: true }),
    verifySession: vi.fn().mockResolvedValue(authenticatedSession),
    confirmSecretAccess: vi.fn().mockResolvedValue({
      grant: "short-lived-grant",
      expiresAt: "2099-01-01T00:00:00Z",
    }),
    rotateSession: vi.fn().mockResolvedValue({ success: true }),
    logout: vi.fn().mockResolvedValue({ success: true }),
    probeCredential: vi.fn().mockResolvedValue({
      result: {
        credentialId: "credential-1",
        providerId: "provider-1",
        probePoint: "fixed-model credential (unsupported)",
        status: "unsupported",
        message: "Credential probe is unavailable in this test.",
        checkedAt: "2099-01-01T00:00:00Z",
      },
    }),
    probeProvider: vi.fn().mockResolvedValue({
      result: {
        providerId: "provider-1",
        status: "unsupported",
        message: "Provider test completed: 0 passed, 0 failed, 1 unsupported.",
        checkedAt: "2099-01-01T00:00:00Z",
        totalCount: 1,
        passedCount: 0,
        failedCount: 0,
        unsupportedCount: 1,
        results: [
          {
            credentialId: "credential-1",
            providerId: "provider-1",
            probePoint: "fixed-model credential (unsupported)",
            status: "unsupported",
            message: "Credential probe is unavailable in this test.",
            checkedAt: "2099-01-01T00:00:00Z",
          },
        ],
      },
    }),
    getAccountGroupSummary: vi.fn().mockRejectedValue(new Error("summary unavailable")),
    getCredentialPoolAutomation: vi.fn().mockResolvedValue({
      automation: {
        enabled: true,
        intervalSeconds: 60,
        drivers: [],
        providers: [],
        revisionId: "r1-deadbeef",
      },
    }),
    runCredentialPoolAutomation: vi.fn(),
    pruneCredentialPool: vi.fn(),
    purgeCredentialArchive: vi.fn(),
    getCredentialRefill: vi.fn().mockResolvedValue({
      refill: {
        enabled: true,
        streamKey: "gw:credential-pool:refill:requests",
        notificationIntervalSeconds: 30,
        defaultLeaseSeconds: 300,
        maxLeaseSeconds: 3_600,
        revisionId: "r1-deadbeef",
        providers: [],
        recentTasks: [],
      },
    }),
    requestCredentialRefill: vi.fn(),
    createGeminiAuthSession: vi.fn().mockResolvedValue({
      session: {
        id: "session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Complete Gemini login in the opened browser window.",
        createdAt: "2099-01-01T00:00:00Z",
        updatedAt: "2099-01-01T00:00:00Z",
        generatedDrafts: [],
      },
    }),
    completeGeminiAuthSession: vi.fn().mockResolvedValue({
      session: {
        id: "session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Manual Gemini import requested. Finishing capture.",
        createdAt: "2099-01-01T00:00:00Z",
        updatedAt: "2099-01-01T00:01:00Z",
        generatedDrafts: [],
      },
    }),
    getGeminiAuthSession: vi.fn().mockResolvedValue({
      session: {
        id: "session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Complete Gemini login in the opened browser window.",
        createdAt: "2099-01-01T00:00:00Z",
        updatedAt: "2099-01-01T00:00:00Z",
        generatedDrafts: [],
      },
    }),
    getRouteConfig: vi.fn().mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeef", sequence: 1 },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: { providers: [], model_routes: [], aliases: {} },
        secrets: [],
        mutationSupported: true,
      },
    }),
    validateRouteConfig: vi.fn().mockResolvedValue({
      validation: {
        document: { providers: [], model_routes: [], aliases: {} },
        secrets: [],
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
      },
    }),
    commitRouteConfig: vi.fn().mockResolvedValue({
      routeConfig: {
        revision: { id: "r2-beadfeed", sequence: 2 },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: { providers: [], model_routes: [], aliases: {} },
        secrets: [],
        mutationSupported: true,
      },
      committed: true,
    }),
    getRouteConfigRevision: vi.fn().mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeef", sequence: 1 },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: { providers: [], model_routes: [], aliases: {} },
        secrets: [],
        mutationSupported: true,
      },
      active: true,
      hasArchive: true,
    }),
    listRouteConfigRevisions: vi.fn().mockResolvedValue({
      revisions: [],
    }),
    ...overrides,
  };
}

export function Providers({
  api,
  children,
  adapter = createBrowserHost(),
}: {
  api: ConsoleApi;
  children: ReactNode;
  adapter?: GatewayHostAdapter;
}) {
  return (
    <HostProvider adapter={adapter}>
      <ManagementSessionProvider api={api}>{children}</ManagementSessionProvider>
    </HostProvider>
  );
}

export function SessionHarness() {
  const session = useManagementSession();
  return (
    <div>
      <output data-testid="phase">{session.phase}</output>
      <output data-testid="grant">{session.secretGrant?.grant ?? "none"}</output>
      <output data-testid="secret-access">
        {String(session.session?.secretAccessGranted ?? false)}
      </output>
      <output data-testid="management-token">{session.managementToken ?? "none"}</output>
      <output data-testid="session-error">{session.error ?? "none"}</output>
      <button
        type="button"
        onClick={() => void session.login("profile-one-token").catch(() => undefined)}
      >
        Log in
      </button>
      <button type="button" onClick={() => void session.confirmSecretAccess("management-secret")}>
        Confirm secret
      </button>
      <button type="button" onClick={session.clearSecretGrant}>
        Clear secret
      </button>
      <button type="button" onClick={() => void session.rotate("replacement-token")}>
        Rotate
      </button>
      <button type="button" onClick={() => void session.logout()}>
        Log out
      </button>
    </div>
  );
}
