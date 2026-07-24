import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

  it("validates and saves edited route documents while preserving existing secret paths", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitFor(() =>
      expect(screen.getByRole("textbox", { name: /route document json/i })).toBeInTheDocument(),
    );
    const editor = screen.getByRole("textbox", { name: /route document json/i });
    fireEvent.change(editor, {
      target: {
        value: JSON.stringify(
          {
            providers: [{ id: "managed-provider" }],
            model_routes: [{ pattern: "gpt-5.4" }],
            aliases: { answer: "gpt-5.4-mini" },
          },
          null,
          2,
        ),
      },
    });

    await user.click(screen.getByRole("button", { name: /validate draft/i }));
    await waitFor(() =>
      expect(consoleApi.validateRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            aliases: { answer: "gpt-5.4-mini" },
          }),
          secretPatches: [{ path: "/providers/0/api_key", operation: "keep" }],
        }),
      ),
    );

    await user.click(screen.getByRole("button", { name: /save route config/i }));
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          expectedRevision: "r1-deadbeefcafe",
          document: expect.objectContaining({
            aliases: { answer: "gpt-5.4-mini" },
          }),
          secretPatches: [{ path: "/providers/0/api_key", operation: "keep" }],
        }),
      ),
    );
  });
});
