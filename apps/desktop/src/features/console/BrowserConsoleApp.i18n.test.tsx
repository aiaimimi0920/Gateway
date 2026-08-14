import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConsoleApi } from "../../api/console";
import { HostProvider } from "../../platform/HostProvider";
import { createBrowserHost } from "../../platform/browserHost";
import {
  ManagementSessionContext,
  type ManagementSessionContextValue,
} from "../../session/ManagementSessionProvider";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { BrowserConsoleApp } from "./BrowserConsoleApp";

function sessionValue(): ManagementSessionContextValue {
  return {
    phase: "authenticated",
    bootstrapStatus: null,
    session: {
      role: "administrator",
      capabilities: ["route-config:read", "route-config:write"],
      activeRevision: "r1-deadbeefcafe",
      secretAccessGranted: false,
    },
    managementToken: "management-secret",
    secretGrant: null,
    busy: false,
    error: null,
    bootstrap: async () => undefined,
    login: async () => undefined,
    logout: async () => undefined,
    rotate: async () => undefined,
    confirmSecretAccess: async () => undefined,
    clearSecretGrant: () => undefined,
    retryInitialization: async () => undefined,
  };
}

function renderWithProviders(children: ReactNode) {
  return render(
    <UiLocaleProvider>
      <HostProvider adapter={createBrowserHost()}>
        <ManagementSessionContext.Provider value={sessionValue()}>
          {children}
        </ManagementSessionContext.Provider>
      </HostProvider>
    </UiLocaleProvider>,
  );
}

function createConsoleApi(): ConsoleApi {
  return {
    getBootstrapStatus: vi.fn(),
    bootstrap: vi.fn(),
    verifySession: vi.fn(),
    confirmSecretAccess: vi.fn(),
    rotateSession: vi.fn(),
    logout: vi.fn(),
    probeCredential: vi.fn(),
    getAccountGroupSummary: vi.fn().mockRejectedValue(new Error("summary unavailable")),
    getCredentialPoolAutomation: vi.fn().mockResolvedValue({
      automation: {
        enabled: true,
        intervalSeconds: 60,
        drivers: [],
        providers: [],
        revisionId: "r1-deadbeefcafe",
      },
    }),
    runCredentialPoolAutomation: vi.fn(),
    getCredentialRefill: vi.fn().mockResolvedValue({
      refill: {
        enabled: true,
        streamKey: "gw:credential-pool:refill:requests",
        notificationIntervalSeconds: 30,
        defaultLeaseSeconds: 300,
        maxLeaseSeconds: 3_600,
        revisionId: "r1-deadbeefcafe",
        providers: [],
        recentTasks: [],
      },
    }),
    requestCredentialRefill: vi.fn(),
    createGeminiAuthSession: vi.fn(),
    completeGeminiAuthSession: vi.fn(),
    getGeminiAuthSession: vi.fn(),
    commitRouteConfig: vi.fn(),
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
        secrets: [],
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
      ],
    }),
    getRouteConfigRevision: vi.fn(),
  };
}

describe("BrowserConsoleApp localization", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("defaults the web console to Chinese and toggles to English", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitFor(() =>
      expect(screen.getByRole("navigation", { name: "Gateway console navigation" })).toBeInTheDocument(),
    );
    expect(screen.queryByRole("heading", { name: "Gateway 网页控制台" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "刷新" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "退出登录" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /账号台账/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /凭证分组/i })).toBeInTheDocument();
    expect(screen.getByText("English")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "切换界面语言" }));

    expect(screen.queryByRole("heading", { name: "Gateway Web Console" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Accounts/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Credential Groups/i })).toBeInTheDocument();
    expect(screen.getByText("中文")).toBeInTheDocument();
  });
});
