import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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

type SessionOverrides = Omit<Partial<ManagementSessionContextValue>, "session"> & {
  session?: Partial<NonNullable<ManagementSessionContextValue["session"]>> | null;
};

function sessionValue(
  overrides: SessionOverrides = {},
): ManagementSessionContextValue {
  const base: ManagementSessionContextValue = {
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
  const mergedSession =
    overrides.session === null
      ? null
      : ({
          ...base.session,
          ...(overrides.session ?? {}),
        } as NonNullable<ManagementSessionContextValue["session"]>);

  return {
    ...base,
    ...overrides,
    session: mergedSession,
  };
}

function renderWithProviders(
  children: ReactNode,
  overrides: SessionOverrides = {},
) {
  return render(
    <HostProvider adapter={createBrowserHost()}>
      <ManagementSessionContext.Provider value={sessionValue(overrides)}>
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

  it("loads archived revision details and copies them into the route editor", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /inspect revision r0-cafebabefeed/i }),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /inspect revision r0-cafebabefeed/i }));

    await waitFor(() =>
      expect(consoleApi.getRouteConfigRevision).toHaveBeenCalledWith(
        "management-secret",
        "r0-cafebabefeed",
      ),
    );
    expect(screen.getByText("legacy-provider")).toBeInTheDocument();
    expect(screen.getByText("gpt-4.1")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /load revision into editor/i }));

    const editor = screen.getByRole("textbox", { name: /route document json/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"answer": "gpt-4.1"');
  });

  it("reviews a revision restore before committing it through the active commit path", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /inspect revision r0-cafebabefeed/i }),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /inspect revision r0-cafebabefeed/i }));

    await waitFor(() =>
      expect(screen.getByText(/active aliases: 1/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/selected aliases: 1/i)).toBeInTheDocument();
    expect(screen.getByText(/answer: gpt-5\.4 -> gpt-4\.1/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /restore revision as active config/i }));
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();

    await waitFor(() =>
      expect(screen.getByRole("dialog", { name: /review revision restore/i })).toBeInTheDocument(),
    );
    const dialog = screen.getByRole("dialog", { name: /review revision restore/i });
    expect(within(dialog).getByText(/current active revision/i)).toBeInTheDocument();
    expect(within(dialog).getByText("r1-deadbeefcafe")).toBeInTheDocument();
    expect(within(dialog).getByText(/revision to restore/i)).toBeInTheDocument();
    expect(within(dialog).getByText("r0-cafebabefeed")).toBeInTheDocument();
    expect(within(dialog).getByText(/alias changes/i)).toBeInTheDocument();
    expect(within(dialog).getByText(/provider changes/i)).toBeInTheDocument();
    expect(within(dialog).getByText(/model route changes/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /confirm restore/i }));

    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          expectedRevision: "r1-deadbeefcafe",
          document: expect.objectContaining({
            aliases: { answer: "gpt-4.1" },
          }),
          secretPatches: [{ path: "/providers/0/api_key", operation: "keep" }],
          message: "restore revision r0-cafebabefeed",
        }),
      ),
    );
    expect(screen.getByRole("status", { name: /gateway console last action/i })).toHaveTextContent(
      /restored revision r0-cafebabefeed as active revision r2-beadfeedcafe/i,
    );
  });

  it("renders side-by-side active and selected route document snapshots for revision review", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /inspect revision r0-cafebabefeed/i }),
      ).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /inspect revision r0-cafebabefeed/i }));

    await waitFor(() =>
      expect(
        screen.getByLabelText(/active route document snapshot/i),
      ).toBeInTheDocument(),
    );
    expect(screen.getByLabelText(/active route document snapshot/i)).toHaveTextContent(
      '"answer": "gpt-5.4"',
    );
    expect(screen.getByLabelText(/selected revision route document snapshot/i)).toHaveTextContent(
      '"answer": "gpt-4.1"',
    );
  });

  it("builds replace secret patches when secret access is already granted", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "short-lived-grant", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /replace \/providers\/0\/api_key/i })).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /replace \/providers\/0\/api_key/i }));
    await user.type(
      screen.getByLabelText(/replacement for \/providers\/0\/api_key/i),
      "sk-new-secret",
    );

    await user.click(screen.getByRole("button", { name: /validate draft/i }));
    await waitFor(() =>
      expect(consoleApi.validateRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          secretPatches: [
            {
              path: "/providers/0/api_key",
              operation: "replace",
              value: "sk-new-secret",
            },
          ],
        }),
      ),
    );
  });

  it("updates aliases through the structured alias editor and keeps the JSON draft in sync", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /add alias row/i })).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: /add alias row/i }));
    await user.type(screen.getByLabelText(/alias name 2/i), "fast");
    await user.type(screen.getByLabelText(/alias target 2/i), "gpt-5.4-mini");

    const editor = screen.getByRole("textbox", { name: /route document json/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"fast": "gpt-5.4-mini"');

    await user.click(screen.getByRole("button", { name: /save route config/i }));

    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            aliases: {
              answer: "gpt-5.4",
              fast: "gpt-5.4-mini",
            },
          }),
        }),
      ),
    );
  });

  it("updates model routes through the structured route editor and keeps the JSON draft in sync", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /add model route row/i })).toBeInTheDocument(),
    );

    await user.click(screen.getByRole("button", { name: /add model route row/i }));
    await user.type(screen.getByLabelText(/model route pattern 2/i), "gpt-5.4-mini");

    const editor = screen.getByRole("textbox", { name: /route document json/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"pattern": "gpt-5.4-mini"');

    await user.click(screen.getByRole("button", { name: /save route config/i }));

    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            model_routes: [
              { pattern: "gpt-5.4" },
              { pattern: "gpt-5.4-mini" },
            ],
          }),
        }),
      ),
    );
  });
});
