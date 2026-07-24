import { render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import type { ConsoleApi } from "../../api/console";
import { HostProvider } from "../../platform/HostProvider";
import { createBrowserHost } from "../../platform/browserHost";
import {
  ManagementSessionContext,
  type ManagementSessionContextValue,
} from "../../session/ManagementSessionProvider";
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
    <HostProvider adapter={createBrowserHost()}>
      <ManagementSessionContext.Provider value={sessionValue()}>
        {children}
      </ManagementSessionContext.Provider>
    </HostProvider>,
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
        secrets: [],
        mutationSupported: true,
      },
    }),
    validateRouteConfig: vi.fn(),
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
  };
}

describe("BrowserConsoleApp", () => {
  it("renders the active route revision and revision history for authenticated browser sessions", async () => {
    const consoleApi = createConsoleApi();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    expect(screen.getByRole("status")).toHaveTextContent(/loading gateway console/i);
    await waitFor(() =>
      expect(screen.getByRole("heading", { name: /gateway web console/i })).toBeInTheDocument(),
    );
    expect(screen.getAllByText("r1-deadbeefcafe").length).toBeGreaterThan(0);
    expect(screen.getByText("managed-provider")).toBeInTheDocument();
    expect(screen.getByText("answer")).toBeInTheDocument();
    expect(screen.getByText("gpt-5.4")).toBeInTheDocument();
    expect(screen.getByText("seed route")).toBeInTheDocument();
    expect(consoleApi.getRouteConfig).toHaveBeenCalledWith("management-secret");
    expect(consoleApi.listRouteConfigRevisions).toHaveBeenCalledWith("management-secret");
  });
});
