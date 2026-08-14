import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConsoleApi } from "../../api/console";
import { GatewayApiError } from "../../api/errors";
import { AppToastViewport } from "../../components/AppToast";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

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
  const host = createBrowserHost();
  const renderTree = (nextOverrides: SessionOverrides) => (
    <UiLocaleProvider>
      <HostProvider adapter={host}>
        <ManagementSessionContext.Provider value={sessionValue(nextOverrides)}>
          {children}
          <AppToastViewport />
        </ManagementSessionContext.Provider>
      </HostProvider>
    </UiLocaleProvider>
  );
  const result = render(renderTree(overrides));
  return {
    ...result,
    rerenderWithProviders(nextOverrides: SessionOverrides) {
      result.rerender(renderTree(nextOverrides));
    },
  };
}

function createConsoleApi(): ConsoleApi {
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
        status: "passed",
        message: "Credential connectivity probe passed.",
        checkedAt: "2026-07-28T02:00:00Z",
      },
    }),
    getAccountGroupSummary: vi.fn().mockRejectedValue(new Error("summary unavailable")),
    getCredentialPoolAutomation: vi.fn().mockRejectedValue(new Error("automation unavailable")),
    runCredentialPoolAutomation: vi.fn(),
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

async function waitForConsoleReady() {
  await waitFor(() =>
    expect(screen.getByRole("navigation", { name: /Gateway console navigation/i })).toBeInTheDocument(),
  );
}

function consoleNavigation() {
  return screen.getByRole("navigation", { name: /Gateway console navigation/i });
}

function workspaceButton(name: RegExp) {
  return within(consoleNavigation()).getByRole("button", { name });
}

async function openWorkspace(user: ReturnType<typeof userEvent.setup>, name: RegExp) {
  await user.click(workspaceButton(name));
}

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("renders the active route revision and revision history for authenticated browser sessions", async () => {
    const consoleApi = createConsoleApi();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    expect(screen.getByRole("status")).toHaveTextContent(/正在加载 Gateway 控制台/i);
    await waitForConsoleReady();
    expect(screen.getAllByText("r1-deadbeefcafe").length).toBeGreaterThan(0);
    expect(screen.getByText("managed-provider")).toBeInTheDocument();
    expect(screen.getByText("answer")).toBeInTheDocument();
    expect(screen.getAllByText("gpt-5.4").length).toBeGreaterThan(0);
    expect(screen.getByText("seed route")).toBeInTheDocument();
    expect(consoleApi.getRouteConfig).toHaveBeenCalledWith("management-secret");
    expect(consoleApi.listRouteConfigRevisions).toHaveBeenCalledWith("management-secret");
  });

  it("surfaces active route diagnostics instead of hiding repair causes", async () => {
    const consoleApi = createConsoleApi();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        diagnostics: {
          diagnostics: [
            {
              code: "secret_document_identity_invalid",
              severity: "error",
              path: "/providers/0/credentials/0/id",
              message: "effective credential IDs must be globally unique",
            },
          ],
        },
        requiresRepair: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(screen.getByRole("alert", { name: /active route diagnostics/i })).toHaveTextContent(
      "/providers/0/credentials/0/id",
    );
  });

  it("explains when the backend account summary is unavailable", async () => {
    const consoleApi = createConsoleApi();
    vi.mocked(consoleApi.getAccountGroupSummary).mockRejectedValue(
      new Error("account inventory unavailable"),
    );

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(screen.getByRole("status", { name: /account summary status/i })).toHaveTextContent(
      /account inventory unavailable/i,
    );
  });

  it("renders a management sidebar and overview snapshots for the browser console", async () => {
    const consoleApi = createConsoleApi();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(workspaceButton(/总览/i)).toBeInTheDocument();
    expect(workspaceButton(/路由编辑/i)).toBeInTheDocument();
    expect(workspaceButton(/账号台账/i)).toBeInTheDocument();
    expect(workspaceButton(/凭证分组/i)).toBeInTheDocument();
    expect(workspaceButton(/敏感信息/i)).toBeInTheDocument();
    expect(workspaceButton(/修订历史/i)).toBeInTheDocument();
    expect(workspaceButton(/高级 JSON/i)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Provider 概览/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Alias 概览/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /模型路由概览/i })).toBeInTheDocument();
  });

  it("removes the browser-console hero and the accounts ledger overview shell chrome", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(screen.queryByRole("heading", { name: /Gateway 网页控制台/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/直接通过 Gateway 本体托管的浏览器控制台/i)).not.toBeInTheDocument();

    await openWorkspace(user, /账号台账/i);
    expect(screen.queryByRole("heading", { name: /账号台账/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/统一查看 Gateway 当前可路由账号与启用状态/i)).not.toBeInTheDocument();
    expect(screen.queryByText("账号总数")).not.toBeInTheDocument();
    expect(screen.queryByRole("searchbox", { name: /搜索账号/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/^当前显示$/)).not.toBeInTheDocument();
  });

  it("switches browser console workspaces without losing the current route draft", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
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

    await openWorkspace(user, /路由编辑/i);
    expect(screen.getByRole("button", { name: /添加 Alias 行/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/Alias 目标 1/i)).toHaveValue("gpt-5.4-mini");

    await openWorkspace(user, /敏感信息/i);
    expect(screen.getByRole("button", { name: /确认敏感信息访问权限/i })).toBeInTheDocument();

    await openWorkspace(user, /修订历史/i);
    expect(
      screen.getByRole("button", { name: /查看修订 r0-cafebabefeed/i }),
    ).toBeInTheDocument();

    await openWorkspace(user, /高级 JSON/i);
    expect(
      (screen.getByRole("textbox", { name: /路由配置 JSON/i }) as HTMLTextAreaElement).value,
    ).toContain('"answer": "gpt-5.4-mini"');
  });

  it("validates and saves edited route documents while preserving existing secret paths", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
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

    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
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

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));
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

    await waitForConsoleReady();
    await openWorkspace(user, /修订历史/i);
    await user.click(screen.getByRole("button", { name: /查看修订 r0-cafebabefeed/i }));

    await waitFor(() =>
      expect(consoleApi.getRouteConfigRevision).toHaveBeenCalledWith(
        "management-secret",
        "r0-cafebabefeed",
      ),
    );
    expect(screen.getByText("legacy-provider")).toBeInTheDocument();
    expect(screen.getByLabelText(/selected revision route document snapshot/i)).toHaveTextContent(
      '"answer": "gpt-4.1"',
    );

    await user.click(screen.getByRole("button", { name: /将修订装载到编辑器/i }));

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"answer": "gpt-4.1"');
  });

  it("reviews a revision restore before committing it through the active commit path", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /修订历史/i);
    await user.click(screen.getByRole("button", { name: /查看修订 r0-cafebabefeed/i }));

    await waitFor(() =>
      expect(screen.getByText(/当前 Alias 数: 1/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/选中 Alias 数: 1/i)).toBeInTheDocument();
    expect(screen.getByText(/answer: gpt-5\.4 -> gpt-4\.1/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /恢复为激活配置/i }));
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();

    await waitFor(() =>
      expect(screen.getByRole("dialog", { name: /确认恢复修订/i })).toBeInTheDocument(),
    );
    const dialog = screen.getByRole("dialog", { name: /确认恢复修订/i });
    expect(within(dialog).getByText(/当前激活修订/i)).toBeInTheDocument();
    expect(within(dialog).getByText("r1-deadbeefcafe")).toBeInTheDocument();
    expect(within(dialog).getByText(/准备恢复的修订/i)).toBeInTheDocument();
    expect(within(dialog).getByText("r0-cafebabefeed")).toBeInTheDocument();
    expect(within(dialog).getByText(/Alias 变更/i)).toBeInTheDocument();
    expect(within(dialog).getByText(/Provider 变更/i)).toBeInTheDocument();
    expect(within(dialog).getByText(/模型路由变更/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /确认恢复/i }));

    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          expectedRevision: "r1-deadbeefcafe",
          document: expect.objectContaining({
            aliases: { answer: "gpt-4.1" },
          }),
          secretPatches: [{ path: "/providers/0/api_key", operation: "keep" }],
          message: "恢复修订 r0-cafebabefeed",
        }),
      ),
    );
    expect(screen.getByRole("status", { name: /gateway console last action/i })).toHaveTextContent(
      /已将修订 r0-cafebabefeed 恢复为激活修订 r2-beadfeedcafe/i,
    );
  });

  it("reconciles an inspected active revision after a newer revision becomes active", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialRouteConfig = await consoleApi.getRouteConfig("management-secret");
    const initialRevisions = await consoleApi.listRouteConfigRevisions("management-secret");
    const nextRouteConfig = {
      routeConfig: {
        ...initialRouteConfig.routeConfig,
        revision: { id: "r2-beadfeedcafe", sequence: 2, message: "save update" },
      },
    };

    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialRouteConfig)
      .mockResolvedValue(nextRouteConfig);
    vi.mocked(consoleApi.listRouteConfigRevisions)
      .mockResolvedValueOnce(initialRevisions)
      .mockResolvedValue({
        revisions: [
          {
            revision: nextRouteConfig.routeConfig.revision,
            active: true,
            hasArchive: true,
            source: "redis",
          },
          {
            revision: initialRouteConfig.routeConfig.revision,
            active: false,
            hasArchive: true,
            source: "archived",
          },
        ],
      });
    vi.mocked(consoleApi.getRouteConfigRevision).mockResolvedValue({
      routeConfig: initialRouteConfig.routeConfig,
      active: true,
      hasArchive: false,
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /修订历史/i);
    await user.click(screen.getByRole("button", { name: /查看修订 r1-deadbeefcafe/i }));

    const selectedDetail = await screen.findByLabelText(/selected revision detail/i);
    expect(within(selectedDetail).getByText("激活中")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /恢复为激活配置/i })).toBeDisabled();

    await openWorkspace(user, /高级 JSON/i);
    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));
    await openWorkspace(user, /修订历史/i);

    await waitFor(() => {
      const refreshedDetail = screen.getByLabelText(/selected revision detail/i);
      expect(within(refreshedDetail).getByText("已归档")).toBeInTheDocument();
      expect(within(refreshedDetail).getByText("可用")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /恢复为激活配置/i })).toBeEnabled();
    });
  });

  it("filters restore secret patches to paths that exist in the selected revision schema", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "active with backup" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [{ id: "managed-provider" }, { id: "backup-provider" }],
          model_routes: [{ pattern: "gpt-5.4" }],
          aliases: { answer: "gpt-5.4" },
        },
        secrets: [
          { path: "/providers/0/api_key", configured: true, preview: "sk-***" },
          { path: "/providers/0/auth_token", configured: false, preview: null },
          { path: "/providers/1/api_key", configured: false, preview: null },
          { path: "/providers/1/auth_token", configured: false, preview: null },
        ],
        mutationSupported: true,
      },
    });
    vi.mocked(consoleApi.getRouteConfigRevision).mockResolvedValue({
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
          providers: [{ id: "managed-provider" }],
          model_routes: [{ pattern: "gpt-5.4" }],
          aliases: { answer: "gpt-5.4" },
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
      active: false,
      hasArchive: true,
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /修订历史/i);
    await user.click(screen.getByRole("button", { name: /查看修订 r0-cafebabefeed/i }));

    await waitFor(() =>
      expect(screen.getByText(/移除 Provider：backup-provider/i)).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /恢复为激活配置/i }));
    await waitFor(() =>
      expect(screen.getByRole("dialog", { name: /确认恢复修订/i })).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /确认恢复/i }));

    await waitFor(() => expect(consoleApi.commitRouteConfig).toHaveBeenCalled());
    const request = vi.mocked(consoleApi.commitRouteConfig).mock.calls.at(-1)?.[1];
    expect(request?.secretPatches).toEqual([
      { path: "/providers/0/api_key", operation: "keep" },
    ]);
  });
  it("renders side-by-side active and selected route document snapshots for revision review", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /修订历史/i);
    await user.click(screen.getByRole("button", { name: /查看修订 r0-cafebabefeed/i }));

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

  it("includes account-group metadata changes in the revision restore review", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "managed-provider",
              credentials: [{ id: "acc-prod-1", api_key: "sk-a" }],
            },
          ],
          model_routes: [{ pattern: "gpt-5.4" }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 当前分组",
              description: "当前描述",
              billing_multiplier: 1.5,
              enabled: true,
              notes: "当前备注",
              provider_credential_ids: ["acc-prod-1"],
            },
          ],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    });
    vi.mocked(consoleApi.getRouteConfigRevision).mockResolvedValue({
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
          providers: [
            {
              id: "managed-provider",
              credentials: [{ id: "acc-prod-1", api_key: "sk-a" }],
            },
          ],
          model_routes: [{ pattern: "gpt-5.4" }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 历史分组",
              description: "历史描述",
              billing_multiplier: 1.5,
              enabled: false,
              notes: "历史备注",
              provider_credential_ids: ["acc-prod-1"],
            },
          ],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
      active: false,
      hasArchive: true,
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /修订历史/i);
    await user.click(screen.getByRole("button", { name: /查看修订 r0-cafebabefeed/i }));

    const change = await screen.findByText(/更新分组：group-vip/i);
    expect(change).toHaveTextContent(/名称 VIP 当前分组 -> VIP 历史分组/i);
    expect(change).toHaveTextContent(/描述 当前描述 -> 历史描述/i);
    expect(change).toHaveTextContent(/状态 启用 -> 停用/i);
    expect(change).toHaveTextContent(/备注 当前备注 -> 历史备注/i);
  });

  it("ignores account-group member order and duplicate entries in revision diffs", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const activeRouteConfig = await consoleApi.getRouteConfig("management-secret");

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...activeRouteConfig.routeConfig,
        document: {
          ...activeRouteConfig.routeConfig.document,
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: ["acc-prod-2", "acc-prod-1", "acc-prod-1"],
            },
          ],
        },
      },
    });
    vi.mocked(consoleApi.getRouteConfigRevision).mockResolvedValue({
      routeConfig: {
        ...activeRouteConfig.routeConfig,
        revision: { id: "r0-cafebabefeed", sequence: 0, message: "seed route" },
        source: "archived",
        document: {
          ...activeRouteConfig.routeConfig.document,
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: ["acc-prod-1", "acc-prod-2"],
            },
          ],
        },
      },
      active: false,
      hasArchive: true,
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /修订历史/i);
    await user.click(screen.getByRole("button", { name: /查看修订 r0-cafebabefeed/i }));
    await screen.findByLabelText(/selected revision detail/i);
    expect(screen.queryByText(/更新分组：group-vip/i)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /恢复为激活配置/i }));
    const dialog = await screen.findByRole("dialog", { name: /确认恢复修订/i });
    expect(within(dialog).getByText(/未检测到账号分组变更/i)).toBeInTheDocument();
  });

  it("builds replace secret patches when secret access is already granted", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "short-lived-grant", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /敏感信息/i);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /替换 \/providers\/0\/api_key/i })).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /替换 \/providers\/0\/api_key/i }));
    await user.type(
      screen.getByLabelText(/\/providers\/0\/api_key 的替换值/i),
      "sk-new-secret",
    );

    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
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
        "short-lived-grant",
      ),
    );
  });

  it("reopens secret confirmation and preserves the sensitive draft when validation grant expires", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const clearSecretGrant = vi.fn();
    const pendingValidation =
      deferred<Awaited<ReturnType<ConsoleApi["validateRouteConfig"]>>>();
    vi.mocked(consoleApi.validateRouteConfig).mockReturnValue(pendingValidation.promise);

    const { rerenderWithProviders } = renderWithProviders(
      <BrowserConsoleApp consoleApi={consoleApi} />,
      {
        session: { secretAccessGranted: true },
        secretGrant: { grant: "expired-grant", expiresAt: "2099-01-01T00:00:00Z" },
        clearSecretGrant,
      },
    );

    await waitForConsoleReady();
    await openWorkspace(user, /敏感信息/i);
    await user.click(screen.getByRole("button", { name: /替换 \/providers\/0\/api_key/i }));
    const replacementInput = screen.getByLabelText(/\/providers\/0\/api_key 的替换值/i);
    await user.type(replacementInput, "sk-unsaved-secret");

    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
    await waitFor(() => expect(consoleApi.validateRouteConfig).toHaveBeenCalled());
    rerenderWithProviders({
      session: { secretAccessGranted: false },
      secretGrant: null,
      clearSecretGrant,
    });
    await act(async () => {
      pendingValidation.reject(
        new GatewayApiError(
          "A fresh secret grant is required.",
          403,
          "console_secret_access_required",
        ),
      );
      await Promise.resolve();
    });

    await waitFor(() => expect(clearSecretGrant).toHaveBeenCalledOnce());
    expect(await screen.findByRole("dialog", { name: /确认敏感信息访问权限/i })).toBeInTheDocument();
    expect(replacementInput).toHaveValue("sk-unsaved-secret");
    expect(screen.getByRole("status", { name: /draft status/i })).toHaveTextContent(
      /有未保存修改/i,
    );
    expect(screen.queryByText("A fresh secret grant is required.")).not.toBeInTheDocument();
  });

  it("ignores an expired validation response after a newer grant lifecycle replaces the request grant", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const clearSecretGrant = vi.fn();
    const pendingValidation =
      deferred<Awaited<ReturnType<ConsoleApi["validateRouteConfig"]>>>();
    vi.mocked(consoleApi.validateRouteConfig).mockReturnValue(pendingValidation.promise);

    const { rerenderWithProviders } = renderWithProviders(
      <BrowserConsoleApp consoleApi={consoleApi} />,
      {
        session: { secretAccessGranted: true },
        secretGrant: { grant: "request-grant", expiresAt: "2099-01-01T00:00:00Z" },
        clearSecretGrant,
      },
    );

    await waitForConsoleReady();
    await openWorkspace(user, /敏感信息/i);
    await user.click(screen.getByRole("button", { name: /替换 \/providers\/0\/api_key/i }));
    const replacementInput = screen.getByLabelText(/\/providers\/0\/api_key 的替换值/i);
    await user.type(replacementInput, "sk-current-draft");
    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
    await waitFor(() => expect(consoleApi.validateRouteConfig).toHaveBeenCalled());

    rerenderWithProviders({
      session: { secretAccessGranted: true },
      secretGrant: { grant: "fresh-grant", expiresAt: "2099-01-01T01:00:00Z" },
      clearSecretGrant,
    });
    rerenderWithProviders({
      session: { secretAccessGranted: false },
      secretGrant: null,
      clearSecretGrant,
    });
    await act(async () => {
      pendingValidation.reject(
        new GatewayApiError(
          "The old secret grant expired.",
          403,
          "console_secret_access_required",
        ),
      );
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /校验草稿/i })).toBeEnabled(),
    );
    expect(clearSecretGrant).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog", { name: /确认敏感信息访问权限/i })).not.toBeInTheDocument();
    expect(screen.queryByText("The old secret grant expired.")).not.toBeInTheDocument();
    expect(replacementInput).toHaveValue("sk-current-draft");
  });

  it("ignores an ordinary validation response after the request grant is cleared", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const pendingValidation =
      deferred<Awaited<ReturnType<ConsoleApi["validateRouteConfig"]>>>();
    vi.mocked(consoleApi.validateRouteConfig).mockReturnValue(pendingValidation.promise);

    const { rerenderWithProviders } = renderWithProviders(
      <BrowserConsoleApp consoleApi={consoleApi} />,
      {
        session: { secretAccessGranted: true },
        secretGrant: { grant: "request-grant", expiresAt: "2099-01-01T00:00:00Z" },
      },
    );

    await waitForConsoleReady();
    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
    await waitFor(() => expect(consoleApi.validateRouteConfig).toHaveBeenCalled());

    rerenderWithProviders({
      session: { secretAccessGranted: false },
      secretGrant: null,
    });
    await act(async () => {
      pendingValidation.resolve({
        validation: {
          document: {
            providers: [{ id: "stale-provider" }],
            model_routes: [],
            aliases: {},
          },
          secrets: [],
          diagnostics: {
            diagnostics: [
              {
                code: "stale_validation_result",
                severity: "warning",
                path: "/providers/0",
                message: "stale validation result must be ignored",
              },
            ],
          },
          requiresRepair: false,
        },
      });
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /校验草稿/i })).toBeEnabled(),
    );
    await openWorkspace(user, /高级 JSON/i);
    expect(screen.queryByText("stale validation result must be ignored")).not.toBeInTheDocument();
  });

  it("ignores an ordinary save response after the request grant is cleared", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const pendingSave = deferred<Awaited<ReturnType<ConsoleApi["commitRouteConfig"]>>>();
    vi.mocked(consoleApi.commitRouteConfig).mockReturnValue(pendingSave.promise);

    const { rerenderWithProviders } = renderWithProviders(
      <BrowserConsoleApp consoleApi={consoleApi} />,
      {
        session: { secretAccessGranted: true },
        secretGrant: { grant: "request-grant", expiresAt: "2099-01-01T00:00:00Z" },
      },
    );

    await waitForConsoleReady();
    await openWorkspace(user, /敏感信息/i);
    await user.click(screen.getByRole("button", { name: /替换 \/providers\/0\/api_key/i }));
    await user.type(
      screen.getByLabelText(/\/providers\/0\/api_key 的替换值/i),
      "sk-preserve-cleared-grant-draft",
    );
    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));
    await waitFor(() => expect(consoleApi.commitRouteConfig).toHaveBeenCalled());

    rerenderWithProviders({
      session: { secretAccessGranted: false },
      secretGrant: null,
    });
    await act(async () => {
      pendingSave.resolve({
        routeConfig: {
          revision: { id: "r2-stale-save", sequence: 2, message: "stale save" },
          source: "redis",
          diagnostics: { diagnostics: [] },
          requiresRepair: false,
          document: {
            providers: [{ id: "stale-provider" }],
            model_routes: [],
            aliases: {},
          },
          secrets: [],
          mutationSupported: true,
        },
        committed: true,
      });
      await Promise.resolve();
    });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /保存路由配置/i })).toBeEnabled(),
    );
    expect(screen.getByLabelText(/\/providers\/0\/api_key 的替换值/i)).toHaveValue(
      "sk-preserve-cleared-grant-draft",
    );
    expect(screen.queryByText(/r2-stale-save/i)).not.toBeInTheDocument();
  });

  it("reopens secret confirmation and preserves the sensitive draft when save grant expires", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const clearSecretGrant = vi.fn();
    vi.mocked(consoleApi.commitRouteConfig).mockRejectedValue(
      new GatewayApiError(
        "A fresh secret grant is required for saving.",
        403,
        "console_secret_access_required",
      ),
    );

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "expired-save-grant", expiresAt: "2099-01-01T00:00:00Z" },
      clearSecretGrant,
    });

    await waitForConsoleReady();
    await openWorkspace(user, /敏感信息/i);
    await user.click(screen.getByRole("button", { name: /替换 \/providers\/0\/api_key/i }));
    const replacementInput = screen.getByLabelText(/\/providers\/0\/api_key 的替换值/i);
    await user.type(replacementInput, "sk-unsaved-after-save");
    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));

    await waitFor(() => expect(clearSecretGrant).toHaveBeenCalledOnce());
    expect(await screen.findByRole("dialog", { name: /确认敏感信息访问权限/i })).toBeInTheDocument();
    expect(replacementInput).toHaveValue("sk-unsaved-after-save");
    expect(screen.getByRole("status", { name: /draft status/i })).toHaveTextContent(
      /有未保存修改/i,
    );
    expect(screen.queryByText("A fresh secret grant is required for saving.")).not.toBeInTheDocument();
  });

  it("reopens secret confirmation and preserves the draft when revision restore grant expires", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const clearSecretGrant = vi.fn();
    vi.mocked(consoleApi.commitRouteConfig).mockRejectedValue(
      new GatewayApiError(
        "A fresh secret grant is required for restoring.",
        403,
        "console_secret_access_required",
      ),
    );

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "expired-restore-grant", expiresAt: "2099-01-01T00:00:00Z" },
      clearSecretGrant,
    });

    await waitForConsoleReady();
    await openWorkspace(user, /敏感信息/i);
    await user.click(screen.getByRole("button", { name: /替换 \/providers\/0\/api_key/i }));
    await user.type(
      screen.getByLabelText(/\/providers\/0\/api_key 的替换值/i),
      "sk-unsaved-after-restore",
    );
    await openWorkspace(user, /修订历史/i);
    await user.click(screen.getByRole("button", { name: /查看修订 r0-cafebabefeed/i }));
    await screen.findByLabelText(/selected revision detail/i);
    await user.click(screen.getByRole("button", { name: /恢复为激活配置/i }));
    await user.click(
      within(screen.getByRole("dialog", { name: /确认恢复修订/i })).getByRole("button", {
        name: /确认恢复/i,
      }),
    );

    await waitFor(() => expect(clearSecretGrant).toHaveBeenCalledOnce());
    expect(await screen.findByRole("dialog", { name: /确认敏感信息访问权限/i })).toBeInTheDocument();
    expect(screen.getByRole("status", { name: /draft status/i })).toHaveTextContent(
      /有未保存修改/i,
    );
    expect(
      screen.queryByText("A fresh secret grant is required for restoring."),
    ).not.toBeInTheDocument();
  });

  it("updates aliases through the structured alias editor and keeps the JSON draft in sync", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /路由编辑/i);

    await user.click(screen.getByRole("button", { name: /添加 Alias 行/i }));
    await user.type(screen.getByLabelText(/Alias 名称 2/i), "fast");
    await user.type(screen.getByLabelText(/Alias 目标 2/i), "gpt-5.4-mini");

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"fast": "gpt-5.4-mini"');

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));

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

    await waitForConsoleReady();
    await openWorkspace(user, /路由编辑/i);

    await user.click(screen.getByRole("button", { name: /添加模型路由行/i }));
    await user.type(screen.getByLabelText(/模型路由模式 2/i), "gpt-5.4-mini");

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"pattern": "gpt-5.4-mini"');

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));

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

  it("updates providers through the structured provider editor and keeps the JSON draft in sync", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /路由编辑/i);

    await user.click(screen.getByRole("button", { name: /添加 Provider 行/i }));
    await user.type(screen.getByLabelText(/Provider ID 2/i), "backup-provider");
    await user.type(
      screen.getByLabelText(/Provider 基础 URL 2/i),
      "https://api.backup.example.com",
    );

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"id": "backup-provider"');
    expect((editor as HTMLTextAreaElement).value).toContain(
      '"base_url": "https://api.backup.example.com"',
    );

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));

    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: expect.arrayContaining([
              expect.objectContaining({
                id: "backup-provider",
                base_url: "https://api.backup.example.com",
              }),
            ]),
          }),
        }),
      ),
    );
  });

  it("updates provider preset and supported models through the structured provider editor", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /路由编辑/i);

    await user.click(screen.getByRole("button", { name: /添加 Provider 行/i }));
    fireEvent.change(screen.getByLabelText(/Provider ID 2/i), {
      target: { value: "backup-provider" },
    });
    fireEvent.change(screen.getByLabelText(/Provider 预设 2/i), {
      target: { value: "openai" },
    });
    fireEvent.change(screen.getByLabelText(/Provider 基础 URL 2/i), {
      target: { value: "https://api.backup.example.com" },
    });
    fireEvent.change(screen.getByLabelText(/Provider 支持模型 2/i), {
      target: { value: "gpt-5.4\ngpt-5.4-mini" },
    });

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"preset": "openai"');
    expect((editor as HTMLTextAreaElement).value).toContain('"supported_models": [');
    expect((editor as HTMLTextAreaElement).value).toContain('"gpt-5.4-mini"');

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));

    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: expect.arrayContaining([
              expect.objectContaining({
                id: "backup-provider",
                preset: "openai",
                base_url: "https://api.backup.example.com",
                supported_models: ["gpt-5.4", "gpt-5.4-mini"],
              }),
            ]),
          }),
        }),
      ),
    );
  });

  it("updates provider vendor metadata through the structured provider editor", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /路由编辑/i);
    await user.type(screen.getByLabelText(/Provider 服务商标识 1/i), "openai");
    await user.type(screen.getByLabelText(/Provider 服务商名称 1/i), "OpenAI");

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"vendor_key": "openai"');
    expect((editor as HTMLTextAreaElement).value).toContain('"vendor_name": "OpenAI"');
  });

  it("renders non-codex providers in the same compact tree ledger layout", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              preset: "openai",
              base_url: "https://api.example.com/v1",
              credentials: [
                {
                  id: "acc-prod-1",
                  account_name: "生产账号 A",
                  api_key: "sk-prod-a",
                  supported_models: ["gpt-5.4"],
                },
                {
                  id: "acc-prod-2",
                  account_name: "生产账号 B",
                  api_key: "sk-prod-b",
                  supported_models: ["gpt-5.4-mini"],
                },
              ],
            },
          ],
          model_routes: [{ pattern: "gpt-5.4", provider_ids: ["managed-provider"] }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: ["acc-prod-1"],
            },
          ],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);

    expect(screen.queryByRole("heading", { name: /账号台账/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/统一查看 Gateway 当前可路由账号与启用状态/i)).not.toBeInTheDocument();
    expect(screen.queryByText("账号总数")).not.toBeInTheDocument();
    expect(screen.queryByRole("searchbox", { name: /搜索账号/i })).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/凭证分组/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/服务商/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("table", { name: /账号台账表/i })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Managed OpenAI$/i })).toBeInTheDocument();
    expect(screen.queryByText("生产账号 A")).not.toBeInTheDocument();
    expect(screen.queryByText("生产账号 B")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Managed OpenAI$/i }));

    const providerPanel = screen.getByRole("region", { name: /Managed OpenAI 账号/i });
    const providerTable = within(providerPanel).getByRole("table", {
      name: /Managed OpenAI 账号表/i,
    });
    const firstRow = within(providerPanel).getByText("acc-prod-1").closest(".nt-provider-account-row");
    const secondRow = within(providerPanel).getByText("acc-prod-2").closest(".nt-provider-account-row");

    expect(firstRow).not.toBeNull();
    expect(secondRow).not.toBeNull();
    expect(within(providerTable).getByRole("columnheader", { name: /账号 ID|Account ID/i })).toBeInTheDocument();
    expect(within(providerTable).getByRole("columnheader", { name: /分组|Group/i })).toBeInTheDocument();
    expect(within(providerTable).getByRole("columnheader", { name: /容量|Capacity/i })).toBeInTheDocument();
    expect(within(providerTable).getByRole("columnheader", { name: /调度|Dispatch/i })).toBeInTheDocument();
    expect(within(firstRow as HTMLElement).getByText("VIP 分组")).toBeInTheDocument();
    expect(within(firstRow as HTMLElement).getByRole("switch", { name: /调度 acc-prod-1|Dispatch acc-prod-1/i })).toHaveAttribute("aria-checked", "true");
    expect(within(secondRow as HTMLElement).getByText(/未分组|Ungrouped/i)).toBeInTheDocument();
    expect(within(secondRow as HTMLElement).getByRole("switch", { name: /调度 acc-prod-2|Dispatch acc-prod-2/i })).toHaveAttribute("aria-checked", "true");
    expect(screen.queryByRole("button", { name: /测试账号 生产账号 A|Test account 生产账号 A/i })).not.toBeInTheDocument();
    expect(within(firstRow as HTMLElement).getByRole("button", { name: /删除账号 生产账号 A|Delete account 生产账号 A/i })).toBeInTheDocument();
  });

  it("creates a custom compatible provider, first account, secret patch, and aggregate model routes", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: {
        grant: "test-secret-grant",
        expiresAt: "2099-01-01T00:00:00Z",
      },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /添加服务商/i }));
    const dialog = screen.getByRole("dialog", { name: /添加服务商与账号/i });
    await user.click(within(dialog).getByRole("button", { name: /自定义 OpenAI-compatible/i }));
    await user.clear(within(dialog).getByLabelText("Provider ID"));
    await user.type(within(dialog).getByLabelText("Provider ID"), "partner-openai");
    await user.clear(within(dialog).getByLabelText("显示名称"));
    await user.type(within(dialog).getByLabelText("显示名称"), "Partner OpenAI");
    await user.clear(within(dialog).getByLabelText("服务商标识"));
    await user.type(within(dialog).getByLabelText("服务商标识"), "partner");
    await user.clear(within(dialog).getByLabelText("服务商名称"));
    await user.type(within(dialog).getByLabelText("服务商名称"), "Partner");
    await user.type(within(dialog).getByLabelText("Base URL"), "https://partner.example.test/v1");
    await user.type(within(dialog).getByLabelText("支持模型与聚合路由"), "shared-model\npartner-model");
    await user.clear(within(dialog).getByLabelText("首个账号 ID"));
    await user.type(within(dialog).getByLabelText("首个账号 ID"), "partner-account-1");
    await user.type(within(dialog).getByLabelText("API Key"), "test-provider-api-key");
    await user.click(within(dialog).getByRole("button", { name: /创建服务商与首个账号/i }));

    expect(await screen.findByText(/服务商 Partner OpenAI、首个账号和 2 条模型聚合路由已写入草稿/i)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));

    await waitFor(() => expect(consoleApi.commitRouteConfig).toHaveBeenCalledTimes(1));
    const commitRequest = vi.mocked(consoleApi.commitRouteConfig).mock.calls[0]?.[1];
    expect(commitRequest?.document.providers).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: "partner-openai",
          adapter: "openai_compatible",
          protocol_profile: "openai_compatible_generic",
          credentials: [
            expect.objectContaining({ id: "partner-account-1", enabled: true }),
          ],
        }),
      ]),
    );
    expect(commitRequest?.document.model_routes).toEqual(
      expect.arrayContaining([
        { pattern: "shared-model", provider_ids: ["partner-openai"] },
        { pattern: "partner-model", provider_ids: ["partner-openai"] },
      ]),
    );
    expect(commitRequest?.secretPatches).toEqual(
      expect.arrayContaining([
        {
          path: "/providers/1/credentials/0/api_key",
          operation: "replace",
          value: "test-provider-api-key",
        },
      ]),
    );
  }, 15_000);

  it("groups internal Gemini providers into three operator-visible channels", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-gemini-channels", sequence: 1, message: "Gemini channels" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "gemini-web-secondary",
              label: "Gemini Web /u/1/",
              preset: "gemini-web-chat-modular",
              credentials: [{ id: "gemini-web-account", account_name: "Gemini Web Account" }],
            },
            {
              id: "gemini-business",
              preset: "gemini-business",
              credentials: [{ id: "gemini-business-account", account_name: "Business Account" }],
            },
            {
              id: "gemini-canvas",
              preset: "gemini-canvas-program-relay",
              credentials: [{ id: "gemini-canvas-account", account_name: "Canvas Account" }],
            },
            {
              id: "gemini-canvas-chat",
              preset: "gemini-canvas-chat",
              credentials: [{ id: "gemini-chat-account", account_name: "Gemini Chat Account" }],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        secrets: [],
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);

    expect(screen.getByRole("button", { name: /^Gemini$/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Gemini Business$/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Gemini Canvas$/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Gemini Web \/u\/1\/$/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^gemini-canvas-chat$/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Gemini$/ }));
    const geminiPanel = screen.getByRole("region", { name: /^Gemini 账号$/ });
    expect(within(geminiPanel).getByText("gemini-web-account")).toBeInTheDocument();
    expect(within(geminiPanel).getByText("gemini-chat-account")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Gemini Canvas$/ }));
    const canvasPanel = screen.getByRole("region", { name: /^Gemini Canvas 账号$/ });
    expect(within(canvasPanel).getByText("gemini-canvas-account")).toBeInTheDocument();
  });

  it("edits provider-level pool policy when a provider has no identity subcategories", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              preset: "openai",
              base_url: "https://api.example.com/v1",
              credentials: [
                {
                  id: "acc-prod-1",
                  account_name: "生产账号 A",
                  enabled: true,
                },
              ],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);

    const providerRow = screen
      .getByRole("button", { name: /^Managed OpenAI$/i })
      .closest(".nt-provider-subtab-row");
    expect(providerRow).not.toBeNull();
    expect(within(providerRow as HTMLElement).getByText(/^号池$/)).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText(/^1\/$/)).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText(/^补号$/)).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText(/^剔号$/)).toBeInTheDocument();

    const targetInput = within(providerRow as HTMLElement).getByRole("spinbutton", {
      name: /Managed OpenAI 目标号池容量|Managed OpenAI target pool size/i,
    });
    const autoRefillSwitch = within(providerRow as HTMLElement).getByRole("switch", {
      name: /Managed OpenAI 自动补号|Managed OpenAI auto refill/i,
    });
    const autoPruneSwitch = within(providerRow as HTMLElement).getByRole("switch", {
      name: /Managed OpenAI 自动剔号|Managed OpenAI auto prune/i,
    });

    expect(targetInput).toHaveValue(30);
    expect(autoRefillSwitch).toHaveAttribute("aria-checked", "false");
    expect(autoPruneSwitch).toHaveAttribute("aria-checked", "false");

    await user.clear(targetInput);
    await user.type(targetInput, "80");
    await user.click(autoRefillSwitch);
    await user.click(autoPruneSwitch);

    expect(targetInput).toHaveValue(80);
    expect(autoRefillSwitch).toHaveAttribute("aria-checked", "true");
    expect(autoPruneSwitch).toHaveAttribute("aria-checked", "true");

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"pool_target_size": 80');
    expect((editor as HTMLTextAreaElement).value).toContain('"auto_refill_enabled": true');
    expect((editor as HTMLTextAreaElement).value).toContain('"auto_prune_enabled": true');
  });

  it("uses the refill queue without a direct driver and still blocks automatic pruning", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [{ id: "managed-provider", label: "Managed OpenAI", credentials: [] }],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
      },
    });
    vi.mocked(consoleApi.getCredentialPoolAutomation).mockResolvedValue({
      automation: {
        enabled: true,
        intervalSeconds: 60,
        drivers: [],
        revisionId: "r1-deadbeefcafe",
        providers: [
          {
            providerId: "managed-provider",
            providerLabel: "Managed OpenAI",
            targetSize: 1,
            credentialCount: 0,
            activeCredentialCount: 0,
            autoRefillEnabled: false,
            autoPruneEnabled: false,
            driverId: null,
            driverMode: null,
            driverConfigured: false,
            state: "not_configured",
            lastRunAt: null,
            nextRunAt: null,
            lastAction: null,
            createdCount: 0,
            prunedCount: 0,
            message: null,
            revisionId: "r1-deadbeefcafe",
          },
        ],
      },
    });
    vi.mocked(consoleApi.getCredentialRefill).mockResolvedValue({
      refill: {
        enabled: true,
        streamKey: "gw:credential-pool:refill:requests",
        notificationIntervalSeconds: 30,
        defaultLeaseSeconds: 300,
        maxLeaseSeconds: 3_600,
        revisionId: "r1-deadbeefcafe",
        providers: [
          {
            providerId: "managed-provider",
            providerLabel: "Managed OpenAI",
            targetSize: 1,
            credentialCount: 0,
            activeCredentialCount: 0,
            deficit: 1,
            needsRefill: true,
            autoRefillEnabled: false,
            directDriverConfigured: false,
            notificationEnabled: false,
            inquiryEnabled: true,
            userRequestEnabled: true,
            outstandingTaskId: null,
            outstandingTaskState: null,
            revisionId: "r1-deadbeefcafe",
          },
        ],
        recentTasks: [],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    const refillSwitch = screen.getByRole("switch", {
      name: /Managed OpenAI 自动补号|Managed OpenAI auto refill/i,
    });
    const pruneSwitch = screen.getByRole("switch", {
      name: /Managed OpenAI 自动剔号|Managed OpenAI auto prune/i,
    });
    expect(screen.getByText(/补号队列已就绪|Refill queue ready/i)).toBeInTheDocument();
    expect(screen.getByText(/通知型关闭|Notify off/i)).toBeInTheDocument();
    expect(screen.getByText(/询问型开启|Inquiry on/i)).toBeInTheDocument();
    expect(screen.getByText(/主动型开启|User request on/i)).toBeInTheDocument();

    await user.click(refillSwitch);
    await user.click(pruneSwitch);

    expect(refillSwitch).toHaveAttribute("aria-checked", "true");
    expect(pruneSwitch).toHaveAttribute("aria-checked", "false");
    expect(
      screen.getByRole("status", { name: /尚未配置受信任的自动剔号驱动器/i }),
    ).toBeInTheDocument();

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"auto_refill_enabled": true');
    expect((editor as HTMLTextAreaElement).value).not.toContain('"auto_prune_enabled": true');
  });

  it("publishes a user-requested refill task and reports outstanding-task deduplication", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              credentials: [{ id: "managed-account", account_name: "Managed account" }],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
      },
    });
    vi.mocked(consoleApi.getCredentialRefill).mockResolvedValue({
      refill: {
        enabled: true,
        streamKey: "gw:credential-pool:refill:requests",
        notificationIntervalSeconds: 30,
        defaultLeaseSeconds: 300,
        maxLeaseSeconds: 3_600,
        revisionId: "r1-deadbeefcafe",
        providers: [
          {
            providerId: "managed-provider",
            providerLabel: "Managed OpenAI",
            targetSize: 2,
            credentialCount: 1,
            activeCredentialCount: 1,
            deficit: 1,
            needsRefill: true,
            autoRefillEnabled: false,
            directDriverConfigured: false,
            notificationEnabled: false,
            inquiryEnabled: true,
            userRequestEnabled: true,
            outstandingTaskId: null,
            outstandingTaskState: null,
            revisionId: "r1-deadbeefcafe",
          },
        ],
        recentTasks: [],
      },
    });
    const task = {
      id: "task-user-1",
      providerId: "managed-provider",
      providerLabel: "Managed OpenAI",
      trigger: "user_requested" as const,
      state: "pending" as const,
      requestedCount: 1,
      targetSize: 2,
      activeCredentialCount: 1,
      routeRevision: "r1-deadbeefcafe",
      createdAt: "2026-08-14T08:00:00Z",
      updatedAt: "2026-08-14T08:00:00Z",
      workerId: null,
      leaseUntil: null,
      attempt: 0,
      deliveryMode: null,
      createdCount: 0,
      message: null,
      revisionId: null,
    };
    vi.mocked(consoleApi.requestCredentialRefill)
      .mockResolvedValueOnce({ task, created: true })
      .mockResolvedValueOnce({ task, created: false });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    const requestButton = screen.getByRole("button", { name: /主动补号|Request refill/i });
    await user.click(requestButton);

    await waitFor(() =>
      expect(consoleApi.requestCredentialRefill).toHaveBeenCalledWith(
        "management-secret",
        "managed-provider",
      ),
    );
    expect(
      screen.getByRole("status", { name: /主动补号任务已投递|user-requested refill task was published/i }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /主动补号|Request refill/i }));
    await waitFor(() => expect(consoleApi.requestCredentialRefill).toHaveBeenCalledTimes(2));
    expect(
      screen.getByRole("status", { name: /已有未完成的补号任务|already has an outstanding refill task/i }),
    ).toBeInTheDocument();
  });

  it("runs configured provider automation through the management API", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              auto_refill_enabled: true,
              credentials: [{ id: "managed-account", account_name: "Managed account" }],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
      },
    });
    const automationProvider = {
      providerId: "managed-provider",
      providerLabel: "Managed OpenAI",
      targetSize: 2,
      credentialCount: 1,
      activeCredentialCount: 1,
      autoRefillEnabled: true,
      autoPruneEnabled: false,
      driverId: "managed-refill",
      driverMode: "http" as const,
      driverConfigured: true,
      state: "idle" as const,
      lastRunAt: null,
      nextRunAt: "2026-08-14T08:01:00Z",
      lastAction: null,
      createdCount: 0,
      prunedCount: 0,
      message: null,
      revisionId: "r1-deadbeefcafe",
    };
    vi.mocked(consoleApi.getCredentialPoolAutomation).mockResolvedValue({
      automation: {
        enabled: true,
        intervalSeconds: 60,
        drivers: [{ id: "managed-refill", mode: "http", providerIds: ["managed-provider"] }],
        providers: [automationProvider],
        revisionId: "r1-deadbeefcafe",
      },
    });
    vi.mocked(consoleApi.runCredentialPoolAutomation).mockResolvedValue({
      provider: {
        ...automationProvider,
        state: "succeeded",
        lastRunAt: "2026-08-14T08:00:00Z",
        createdCount: 1,
        message: "Pool reconciled.",
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /运行直连|Run direct/i }));

    await waitFor(() =>
      expect(consoleApi.runCredentialPoolAutomation).toHaveBeenCalledWith(
        "management-secret",
        "managed-provider",
      ),
    );
    expect(screen.getByRole("status", { name: /Pool reconciled/i })).toBeInTheDocument();
  });

  it("renders a collapsed codex provider row before expanding its identity subcategories", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              enabled: true,
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);

    expect(screen.getByRole("button", { name: /^codex$/i })).toBeInTheDocument();
    expect(screen.queryByText("Free")).not.toBeInTheDocument();
    expect(screen.queryByText("Plus")).not.toBeInTheDocument();
    expect(screen.queryByRole("table", { name: /账号台账表/i })).not.toBeInTheDocument();
  });

  it("expands codex identity subcategories and shows logical labels on account rows", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              credentials: [
                {
                  id: "codex-free-1",
                  account_name: "Codex Free 1",
                  enabled: true,
                  credential_identity_category_id: "free",
                  preview_capacity: "3 / 4",
                  preview_status: "正常",
                  preview_usage_window_badges: ["32 req", "0", "A $0.00", "U $0.00"],
                  preview_recent_use: "2 分钟前",
                },
              ],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [
            {
              id: "vip-users",
              name: "VIP 用户",
              billing_multiplier: 1,
              provider_credential_ids: ["codex-free-1"],
            },
          ],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));

    const providerRow = screen.getByRole("button", { name: /^codex$/i }).closest(".nt-provider-tree-item");
    expect(providerRow).not.toBeNull();
    expect(within(providerRow as HTMLElement).getByText("统计 1")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText(/容量 3\s*\/\s*4/)).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText("正常 1")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText("调度中 1")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText("32 req")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).queryByText(/最近 /)).not.toBeInTheDocument();
    expect((providerRow as HTMLElement).querySelectorAll(".nt-chip").length).toBe(0);

    expect(screen.getByText("Free")).toBeInTheDocument();
    expect(screen.getByText("Plus")).toBeInTheDocument();
    expect(screen.queryByText(/默认入口 codex::default/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/OpenAI\s*·\s*chatgpt\.com/i)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Free$/i }));

    const freePanel = screen.getByRole("region", { name: /codex Free 账号/i });
    const freeTable = within(freePanel).getByRole("table", { name: /codex Free 账号表/i });
    const freeRow = within(freePanel).getByText("codex-free-1").closest(".nt-provider-account-row");
    expect(freeRow).not.toBeNull();
    expect(within(freeTable).getByRole("columnheader", { name: /账号 ID|Account ID/i })).toBeInTheDocument();
    expect(within(freeTable).getByRole("columnheader", { name: /分组|Group/i })).toBeInTheDocument();
    expect(within(freeTable).getByRole("columnheader", { name: /容量|Capacity/i })).toBeInTheDocument();
    expect(within(freeTable).getByRole("columnheader", { name: /调度|Dispatch/i })).toBeInTheDocument();
    expect(within(freeTable).getByRole("columnheader", { name: /用量窗口|Usage window/i })).toBeInTheDocument();
    expect(within(freeTable).getByRole("columnheader", { name: /最近使用|Recent use/i })).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("Codex Free 1")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("逻辑分类")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("容量")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("状态")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("调度")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("用量窗口")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("最近使用")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("VIP 用户")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("3 / 4")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("正常")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("VIP 优先")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("32 req")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("0")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("A $0.00")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("U $0.00")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("2 分钟前")).toBeInTheDocument();
    expect(
      within(freeRow as HTMLElement).getByRole("switch", { name: /调度 codex-free-1|Dispatch codex-free-1/i }),
    ).toHaveAttribute("aria-checked", "true");
  });

  it("toggles codex dispatch to pause an explicit credential in the draft", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              credentials: [
                {
                  id: "codex-free-1",
                  account_name: "Codex Free 1",
                  enabled: true,
                  credential_identity_category_id: "free",
                },
              ],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));

    const providerRow = screen.getByRole("button", { name: /^codex$/i }).closest(".nt-provider-tree-item");
    expect(providerRow).not.toBeNull();
    expect(within(providerRow as HTMLElement).getByText("统计 1")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText(/容量 1\s*\/\s*1/)).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText("正常 1")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText("调度中 1")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText("0 req")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^Free$/i }));

    const freePanel = screen.getByRole("region", { name: /codex Free 账号/i });
    const dispatchSwitch = within(freePanel).getByRole("switch", {
      name: /调度 codex-free-1|Dispatch codex-free-1/i,
    });

    expect(dispatchSwitch).toHaveAttribute("aria-checked", "true");
    await user.click(dispatchSwitch);
    expect(dispatchSwitch).toHaveAttribute("aria-checked", "false");

    const pausedRow = within(freePanel).getByText("codex-free-1").closest(".nt-provider-account-row");
    expect(pausedRow).not.toBeNull();
    expect(within(pausedRow as HTMLElement).getByText("暂停")).toBeInTheDocument();

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"id": "codex-free-1"');
    expect((editor as HTMLTextAreaElement).value).toContain('"enabled": false');
  });

  it("opens the codex more menu and starts a connection test from the dialog", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              credentials: [
                {
                  id: "codex-free-1",
                  account_name: "Codex Free 1",
                  enabled: true,
                  credential_identity_category_id: "free",
                },
              ],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));

    const providerRow = screen.getByRole("button", { name: /^codex$/i }).closest(".nt-provider-tree-item");
    expect(providerRow).not.toBeNull();
    expect(within(providerRow as HTMLElement).getByText("统计 1")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText(/容量 1\s*\/\s*1/)).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText("正常 1")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText("调度中 1")).toBeInTheDocument();
    expect(within(providerRow as HTMLElement).getByText("0 req")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^Free$/i }));

    const freePanel = screen.getByRole("region", { name: /codex Free 账号/i });
    const moreButton = within(freePanel).getByRole("button", {
      name: /更多操作 codex-free-1|More actions codex-free-1/i,
    });
    await user.click(moreButton);

    expect(screen.getByRole("menuitem", { name: /测试连接|Test connection/i })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /查看统计|View stats/i })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /定时测试|Scheduled tests/i })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /复制账号|Duplicate account/i })).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menuitem", { name: /测试连接|Test connection/i })).not.toBeInTheDocument();
    expect(moreButton).toHaveFocus();

    await user.click(moreButton);
    await user.click(within(freePanel).getByText("codex-free-1"));
    expect(screen.queryByRole("menuitem", { name: /测试连接|Test connection/i })).not.toBeInTheDocument();

    await user.click(moreButton);
    await user.click(screen.getByRole("menuitem", { name: /测试连接|Test connection/i }));

    const probeDialog = screen.getByRole("dialog", { name: /测试账号连接|Test account connection/i });
    expect(within(probeDialog).getByText("Codex Free 1")).toBeInTheDocument();
    await user.click(within(probeDialog).getByRole("button", { name: /开始测试|Start test/i }));

    await waitFor(() =>
      expect(consoleApi.probeCredential).toHaveBeenCalledWith(
        "management-secret",
        "grant-1",
        "codex-free-1",
      ),
    );
    expect(await within(probeDialog).findByText(/Credential connectivity probe passed\./i)).toBeInTheDocument();
  });

  it("shows a sub2api-style statistics workspace from the codex more menu", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              credentials: [
                {
                  id: "codex-free-1",
                  account_name: "Codex Free 1",
                  enabled: true,
                  credential_identity_category_id: "free",
                  preview_capacity: "3 / 4",
                  preview_recent_use: "2 分钟前",
                  preview_usage_window_badges: ["32 req", "0", "A $0.00", "U $0.00"],
                },
              ],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));

    const providerRow = screen.getByRole("button", { name: /^codex$/i }).closest(".nt-provider-tree-item");
    expect(providerRow).not.toBeNull();
    expect(within(providerRow as HTMLElement).queryByText(/最近 /)).not.toBeInTheDocument();
    expect((providerRow as HTMLElement).querySelectorAll(".nt-chip").length).toBe(0);

    const plusSummaryRow = screen.getByRole("button", { name: /^Plus$/i }).closest(".nt-provider-tree-item");
    expect(plusSummaryRow).not.toBeNull();
    expect(within(plusSummaryRow as HTMLElement).queryByText(/最近 /)).not.toBeInTheDocument();
    expect((plusSummaryRow as HTMLElement).querySelectorAll(".nt-chip").length).toBe(0);
    await user.click(screen.getByRole("button", { name: /^Free$/i }));

    const freePanel = screen.getByRole("region", { name: /codex Free 账号/i });
    await user.click(
      within(freePanel).getByRole("button", {
        name: /更多操作 codex-free-1|More actions codex-free-1/i,
      }),
    );
    await user.click(screen.getByRole("menuitem", { name: /查看统计|View stats/i }));

    const statsDialog = screen.getByRole("dialog", { name: /查看账号统计|View account stats/i });
    expect(within(statsDialog).getByText("Codex Free 1")).toBeInTheDocument();
    expect(within(statsDialog).getByText(/近30天使用统计/i)).toBeInTheDocument();
    expect(within(statsDialog).getByText("30天总费用")).toBeInTheDocument();
    expect(within(statsDialog).getByText("30天总请求")).toBeInTheDocument();
    expect(within(statsDialog).getByText("日均费用")).toBeInTheDocument();
    expect(within(statsDialog).getByText("日均请求")).toBeInTheDocument();
    expect(within(statsDialog).getByText("今日概览")).toBeInTheDocument();
    expect(within(statsDialog).getByText("最高费用日")).toBeInTheDocument();
    expect(within(statsDialog).getByText("最高请求日")).toBeInTheDocument();
    expect(within(statsDialog).getByText("累计 Token")).toBeInTheDocument();
    expect(within(statsDialog).getByText("性能")).toBeInTheDocument();
    expect(within(statsDialog).getByText("最近统计")).toBeInTheDocument();
    expect(within(statsDialog).getByText("30天费用与请求趋势")).toBeInTheDocument();
    expect(within(statsDialog).getByText("模型分布")).toBeInTheDocument();
    expect(within(statsDialog).getByText("入站端点")).toBeInTheDocument();
    expect(within(statsDialog).getByText("上游端点")).toBeInTheDocument();
    expect(within(statsDialog).getAllByText(/暂无数据|No data/i).length).toBeGreaterThanOrEqual(4);
  });

  it("opens the codex scheduled-test dialog and duplicates an account from the more menu", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              credentials: [
                {
                  id: "codex-free-1",
                  account_name: "Codex Free 1",
                  enabled: true,
                  credential_identity_category_id: "free",
                },
              ],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));
    await user.click(screen.getByRole("button", { name: /^Free$/i }));

    const freePanel = screen.getByRole("region", { name: /codex Free 账号/i });
    const moreButton = within(freePanel).getByRole("button", {
      name: /更多操作 codex-free-1|More actions codex-free-1/i,
    });

    await user.click(moreButton);
    await user.click(screen.getByRole("menuitem", { name: /定时测试|Scheduled tests/i }));

    const scheduleDialog = screen.getByRole("dialog", { name: /定时测试|Scheduled tests/i });
    expect(within(scheduleDialog).getByRole("button", { name: /添加计划|Add schedule/i })).toBeInTheDocument();
    expect(within(scheduleDialog).getByText(/暂无定时测试计划|No scheduled test plans/i)).toBeInTheDocument();
    await user.click(within(scheduleDialog).getByRole("button", { name: /关闭|Close/i }));

    await user.click(moreButton);
    await user.click(screen.getByRole("menuitem", { name: /复制账号|Duplicate account/i }));

    const duplicateDialog = screen.getByRole("dialog", { name: /新增账号|Add account/i });
    expect(within(duplicateDialog).getByLabelText(/账号 ID/i)).toHaveValue("codex-free-1-copy");
    expect(within(duplicateDialog).getByLabelText(/账号名称/i)).toHaveValue("Codex Free 1 Copy");
  });

  it("keeps multiple codex identity subcategories expanded at the same time", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              enabled: true,
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));
    await user.click(screen.getByRole("button", { name: /^Plus$/i }));
    expect(screen.getByRole("region", { name: /codex Plus 账号/i })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Free$/i }));

    expect(screen.getByRole("region", { name: /codex Free 账号/i })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /codex Plus 账号/i })).toBeInTheDocument();
  });

  it("renders the codex account column header only once when multiple identity subcategories are expanded", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              enabled: true,
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));
    await user.click(screen.getByRole("button", { name: /^Plus$/i }));
    await user.click(screen.getByRole("button", { name: /^Free$/i }));

    expect(screen.getByRole("region", { name: /codex Free 账号/i })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /codex Plus 账号/i })).toBeInTheDocument();
    expect(screen.getAllByRole("columnheader", { name: /账号 ID/i })).toHaveLength(1);
    expect(screen.getAllByRole("columnheader", { name: /操作/i })).toHaveLength(1);
  });

  it("edits codex identity category pool policy from the category row", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              enabled: true,
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));

    const plusRow = screen
      .getByRole("button", { name: /^Plus$/i })
      .closest(".nt-provider-subtab-row");
    expect(plusRow).not.toBeNull();
    expect(within(plusRow as HTMLElement).queryByText(/可用号池容量/i)).not.toBeInTheDocument();
    expect(within(plusRow as HTMLElement).getByText(/^号池$/)).toBeInTheDocument();
    expect(within(plusRow as HTMLElement).getByText(/^2\/$/)).toBeInTheDocument();
    expect(within(plusRow as HTMLElement).getByText(/^补号$/)).toBeInTheDocument();
    expect(within(plusRow as HTMLElement).getByText(/^剔号$/)).toBeInTheDocument();

    const targetInput = within(plusRow as HTMLElement).getByRole("spinbutton", {
      name: /Plus 目标号池容量|Plus target pool size/i,
    });
    const autoRefillSwitch = within(plusRow as HTMLElement).getByRole("switch", {
      name: /Plus 自动补号|Plus auto refill/i,
    });
    const autoPruneSwitch = within(plusRow as HTMLElement).getByRole("switch", {
      name: /Plus 自动剔号|Plus auto prune/i,
    });

    expect(targetInput).toHaveValue(30);
    expect(autoRefillSwitch).toHaveAttribute("aria-checked", "false");
    expect(autoPruneSwitch).toHaveAttribute("aria-checked", "false");

    await user.clear(targetInput);
    await user.type(targetInput, "50");
    await user.click(autoRefillSwitch);
    await user.click(autoPruneSwitch);

    expect(targetInput).toHaveValue(50);
    expect(autoRefillSwitch).toHaveAttribute("aria-checked", "true");
    expect(autoPruneSwitch).toHaveAttribute("aria-checked", "true");

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"pool_target_size": 50');
    expect((editor as HTMLTextAreaElement).value).toContain('"auto_refill_enabled": true');
    expect((editor as HTMLTextAreaElement).value).toContain('"auto_prune_enabled": true');
  });

  it("lets the expanded codex pilot append a provider-specific identity subcategory", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              enabled: true,
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });
    const prompt = vi.spyOn(window, "prompt").mockReturnValue("Enterprise");

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));
    await user.click(screen.getByRole("button", { name: /添加账号类别/i }));

    expect(prompt).toHaveBeenCalled();
    expect(screen.getByText("Enterprise")).toBeInTheDocument();
  });

  it("shows seeded codex demo accounts when only the default codex entry exists", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "codex",
              label: "codex",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              base_url: "https://chatgpt.com/backend-api",
              enabled: true,
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));
    await user.click(screen.getByRole("button", { name: /^Free$/i }));

    const freePanel = screen.getByRole("region", { name: /codex Free 账号/i });
    const freeTable = within(freePanel).getByRole("table", { name: /codex Free 账号表/i });
    const freeRow = within(freePanel)
      .getByText("codex-free-demo-a")
      .closest(".nt-provider-account-row");
    expect(freeRow).not.toBeNull();
    expect(within(freeTable).getByRole("columnheader", { name: /账号 ID|Account ID/i })).toBeInTheDocument();
    expect(within(freeTable).getByRole("columnheader", { name: /分组|Group/i })).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("Codex Free Demo A")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("逻辑分类")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).queryByText("容量")).not.toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("普通用户")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("1 / 1")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("正常")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("18 req")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("0")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("A $0.00")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("U $0.00")).toBeInTheDocument();
    expect(within(freeRow as HTMLElement).getByText("12 分钟前")).toBeInTheDocument();
    const freeDispatchSwitch = within(freeRow as HTMLElement).getByRole("switch", {
      name: /调度 codex-free-demo-a|Dispatch codex-free-demo-a/i,
    });
    expect(freeDispatchSwitch).toBeEnabled();
    expect(freeDispatchSwitch).toHaveAttribute("aria-checked", "true");

    await user.click(freeDispatchSwitch);

    await waitFor(() => expect(freeDispatchSwitch).toHaveAttribute("aria-checked", "false"));
    expect(screen.getByRole("status", { name: /gateway console last action/i })).toHaveTextContent(
      /账号 codex-free-demo-a 已暂停调度，保存路由配置后生效/i,
    );

    await user.click(screen.getByRole("button", { name: /^Plus$/i }));
    const plusPanel = screen.getByRole("region", { name: /codex Plus 账号/i });
    const plusTable = within(plusPanel).getByRole("table", { name: /codex Plus 账号表/i });
    const plusRowA = within(plusPanel)
      .getByText("codex-plus-demo-a")
      .closest(".nt-provider-account-row");
    const plusRowB = within(plusPanel)
      .getByText("codex-plus-demo-b")
      .closest(".nt-provider-account-row");
    expect(plusRowA).not.toBeNull();
    expect(plusRowB).not.toBeNull();
    expect(screen.getAllByRole("columnheader", { name: /调度|Dispatch/i })).toHaveLength(1);
    expect(screen.getAllByRole("columnheader", { name: /用量窗口|Usage window/i })).toHaveLength(1);
    expect(within(plusTable).queryByRole("columnheader", { name: /调度|Dispatch/i })).not.toBeInTheDocument();
    expect(within(plusTable).queryByRole("columnheader", { name: /用量窗口|Usage window/i })).not.toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).queryByText("Codex Plus Demo A")).not.toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).queryByText("逻辑分类")).not.toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).queryByText("容量")).not.toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).getByText("VIP 用户")).toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).getByText("2 / 3")).toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).getByText("正常")).toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).queryByText("VIP 优先")).not.toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).getByText("32 req")).toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).getByText("A $0.00")).toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).getByText("U $0.00")).toBeInTheDocument();
    expect(within(plusRowA as HTMLElement).getByText("2 分钟前")).toBeInTheDocument();
    expect(within(plusRowB as HTMLElement).queryByText("Codex Plus Demo B")).not.toBeInTheDocument();
    expect(within(plusRowB as HTMLElement).getByText("普通用户")).toBeInTheDocument();
    expect(within(plusRowB as HTMLElement).getByText("1 / 2")).toBeInTheDocument();
    expect(within(plusRowB as HTMLElement).getByText("正常")).toBeInTheDocument();
    expect(within(plusRowB as HTMLElement).queryByText("共享轮询")).not.toBeInTheDocument();
    expect(within(plusRowB as HTMLElement).getByText("9 req")).toBeInTheDocument();
    expect(within(plusRowB as HTMLElement).getByText("A $0.00")).toBeInTheDocument();
    expect(within(plusRowB as HTMLElement).getByText("U $0.00")).toBeInTheDocument();
    expect(within(plusRowB as HTMLElement).getByText("8 分钟前")).toBeInTheDocument();
  });

  it("removes the old accounts filter controls from the ledger workspace", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              preset: "openai",
              base_url: "https://api.example.com/v1",
              credentials: [
                {
                  id: "acc-prod-1",
                  account_name: "生产账号 A",
                  enabled: true,
                  api_key: "sk-prod-a",
                  supported_models: ["gpt-5.4"],
                },
                {
                  id: "acc-prod-2",
                  account_name: "生产账号 B",
                  enabled: false,
                  api_key: "sk-prod-b",
                  supported_models: ["gpt-5.4-mini"],
                },
              ],
            },
          ],
          model_routes: [{ pattern: "gpt-5.4", provider_ids: ["managed-provider"] }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: ["acc-prod-1"],
            },
          ],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);

    expect(screen.getByRole("button", { name: /^Managed OpenAI$/i })).toBeInTheDocument();
    expect(screen.queryByText("acc-prod-1")).not.toBeInTheDocument();
    expect(screen.queryByText("acc-prod-2")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/凭证分组/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/服务商/i)).not.toBeInTheDocument();
  });

  it("prefers the active backend account summary when the draft still matches the active revision", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              preset: "openai",
              base_url: "https://api.example.com/v1",
            },
          ],
          model_routes: [{ pattern: "gpt-5.4", provider_ids: ["managed-provider"] }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    });
    vi.mocked(consoleApi.getAccountGroupSummary).mockResolvedValue({
      summary: {
        routeConfigRevision: "r1-deadbeefcafe",
        source: "redis",
        accountGroups: [
          {
            id: "group-vip",
            name: "VIP 分组",
            description: null,
            billingMultiplier: 1.5,
            configuredBillingMultiplier: 1.5,
            enabled: true,
            notes: null,
            memberCount: 1,
            providerCredentialIds: ["acc-prod-1"],
            providers: ["managed-provider"],
          },
        ],
        accounts: [
          {
            id: "acc-prod-1",
            displayName: "生产账号 A",
            providerId: "managed-provider",
            providerLabel: "Managed OpenAI",
            providerPreset: "openai",
            credentialId: "acc-prod-1",
            baseUrl: "https://api.example.com/v1",
            mode: "credential",
            enabled: false,
            supportedModels: ["gpt-5.4"],
            groupIds: ["group-vip"],
          },
        ],
        providers: [
          {
            id: "managed-provider",
            label: "Managed OpenAI",
            preset: "openai",
            baseUrl: "https://api.example.com/v1",
            accountIds: ["acc-prod-1"],
            supportedModels: ["gpt-5.4"],
          },
        ],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);

    await user.click(screen.getByRole("button", { name: /^Managed OpenAI$/i }));

    const providerPanel = screen.getByRole("region", { name: /Managed OpenAI 账号/i });
    const summaryAccount = within(providerPanel).getByText("acc-prod-1").closest('[role="row"]');
    expect(summaryAccount).not.toBeNull();
    expect(within(summaryAccount as HTMLElement).getByText("暂停")).toBeInTheDocument();
    expect(within(summaryAccount as HTMLElement).getByText("VIP 分组")).toBeInTheDocument();
    expect(screen.queryByText("分组数量")).not.toBeInTheDocument();
  });

  it("drops orphaned summary accounts whose provider is no longer active", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [{ id: "managed-provider", label: "Managed OpenAI", preset: "openai" }],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
      },
    });
    vi.mocked(consoleApi.getAccountGroupSummary).mockResolvedValue({
      summary: {
        routeConfigRevision: "r1-deadbeefcafe",
        source: "redis",
        accountGroups: [],
        accounts: [
          {
            id: "managed-account",
            displayName: "Managed account",
            providerId: "managed-provider",
            providerLabel: "Managed OpenAI",
            providerPreset: "openai",
            credentialId: "managed-account",
            baseUrl: null,
            mode: "credential",
            enabled: true,
            supportedModels: [],
            groupIds: [],
          },
          {
            id: "legacy-media-account",
            displayName: "Legacy media account",
            providerId: "gemini-canvas-legacy-media",
            providerLabel: "Gemini Canvas legacy media",
            providerPreset: "gemini-canvas-legacy-media",
            credentialId: "legacy-media-account",
            baseUrl: null,
            mode: "credential",
            enabled: true,
            supportedModels: [],
            groupIds: [],
          },
        ],
        providers: [
          {
            id: "managed-provider",
            label: "Managed OpenAI",
            preset: "openai",
            baseUrl: null,
            accountIds: ["managed-account"],
            supportedModels: [],
          },
        ],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);

    expect(screen.getByRole("button", { name: /^Managed OpenAI$/i })).toBeInTheDocument();
    expect(screen.queryByText("legacy-media-account")).not.toBeInTheDocument();
    expect(screen.queryByText("Legacy media account")).not.toBeInTheDocument();
    expect(screen.queryByText("Gemini Canvas legacy media")).not.toBeInTheDocument();
  });

  it("groups accounts by explicit vendor metadata before expanding individual providers", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          ...initialResponse.routeConfig.document,
          providers: [
            {
              id: "qwen-openai",
              label: "Qwen OpenAI",
              vendor_key: "alibaba-qwen",
              vendor_name: "Alibaba / Qwen",
              credentials: [{ id: "qwen-account-1", account_name: "Qwen account 1" }],
            },
            {
              id: "qwen-web",
              label: "Qwen Web",
              vendor_key: "alibaba-qwen",
              vendor_name: "Alibaba / Qwen",
              credentials: [{ id: "qwen-account-2", account_name: "Qwen account 2" }],
            },
          ],
        },
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);

    expect(screen.getByText("Qwen OpenAI")).toBeInTheDocument();
    expect(screen.getByText("Qwen Web")).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "Alibaba / Qwen" })).not.toBeInTheDocument();
  });

  it("adds an explicit credential from Accounts and commits its API key as a secret patch", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              vendor_key: "openai",
              vendor_name: "OpenAI",
              preset: "openai",
              base_url: "https://api.example.com/v1",
              supported_models: ["gpt-5.4"],
            },
          ],
          model_routes: [{ pattern: "gpt-5.4", provider_ids: ["managed-provider"] }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: ["managed-provider::default"],
            },
          ],
        },
        secrets: [
          { path: "/providers/0/api_key", configured: true, preview: "sk-***" },
        ],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "credential-write-grant", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^Managed OpenAI$/i }));
    await user.click(
      within(screen.getByRole("region", { name: /Managed OpenAI 账号/i })).getByRole("button", {
        name: /为 Managed OpenAI 添加显式账号|Add explicit account for Managed OpenAI/i,
      }),
    );

    const dialog = screen.getByRole("dialog", { name: /新增账号/i });
    expect(
      within(dialog).getByRole("checkbox", { name: /账号启用状态/i }),
    ).toBeChecked();
    await user.selectOptions(within(dialog).getByLabelText(/^Provider$/i), "managed-provider");
    await user.type(within(dialog).getByLabelText(/账号 ID/i), "acc-prod-1");
    await user.type(within(dialog).getByLabelText(/账号名称/i), "生产账号 A");
    await user.type(
      within(dialog).getByLabelText(/账号覆盖地址/i),
      "https://account.example.com/v1",
    );
    await user.type(within(dialog).getByLabelText(/支持模型/i), "gpt-5.4\ngpt-5.4-mini");
    await user.type(within(dialog).getByLabelText(/^API Key$/i), "sk-new-account");
    await user.click(within(dialog).getByRole("button", { name: /保存到草稿/i }));

    expect(screen.getByText("acc-prod-1")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: [
              expect.objectContaining({
                id: "managed-provider",
                credentials: [
                  expect.objectContaining({
                    id: "acc-prod-1",
                    account_name: "生产账号 A",
                    enabled: true,
                    base_url: "https://account.example.com/v1",
                    supported_models: ["gpt-5.4", "gpt-5.4-mini"],
                  }),
                ],
              }),
            ],
            account_groups: [
              expect.objectContaining({
                id: "group-vip",
                provider_credential_ids: ["acc-prod-1"],
              }),
            ],
          }),
          secretPatches: [
            { path: "/providers/0/api_key", operation: "keep" },
            {
              path: "/providers/0/credentials/0/api_key",
              operation: "replace",
              value: "sk-new-account",
            },
          ],
        }),
        "credential-write-grant",
      ),
    );
  });

  it("edits credential metadata while keeping its stable identity", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              credentials: [
                {
                  id: "acc-prod-1",
                  account_name: "生产账号 A",
                  enabled: true,
                  supported_models: ["gpt-5.4"],
                },
              ],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        secrets: [
          {
            path: "/providers/0/credentials/0/api_key",
            configured: true,
            preview: "sk-***",
          },
        ],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^Managed OpenAI$/i }));
    await user.click(
      within(screen.getByRole("region", { name: /Managed OpenAI 账号/i })).getByRole("button", {
        name: /编辑账号 生产账号 A/i,
      }),
    );

    const dialog = screen.getByRole("dialog", { name: /编辑账号/i });
    expect(within(dialog).getByLabelText(/账号 ID/i)).toBeDisabled();
    const enabledToggle = within(dialog).getByRole("checkbox", {
      name: /账号启用状态/i,
    });
    expect(enabledToggle).toBeChecked();
    await user.click(enabledToggle);
    const nameInput = within(dialog).getByLabelText(/账号名称/i);
    await user.clear(nameInput);
    await user.type(nameInput, "生产账号 A2");
    await user.selectOptions(within(dialog).getByLabelText(/API Key 操作/i), "replace");
    await user.type(within(dialog).getByLabelText(/^API Key$/i), "sk-replaced-account");
    await user.click(within(dialog).getByRole("button", { name: /保存到草稿/i }));

    const editedAccount = within(
      screen.getByRole("region", { name: /Managed OpenAI 账号/i }),
    )
      .getByText("acc-prod-1")
      .closest('[role="row"]');
    expect(editedAccount).not.toBeNull();
    expect(within(editedAccount as HTMLElement).getByText("暂停")).toBeInTheDocument();
    expect(
      within(editedAccount as HTMLElement).getByRole("button", {
        name: /编辑账号 生产账号 A2/i,
      }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: [
              expect.objectContaining({
                credentials: [
                  expect.objectContaining({
                    id: "acc-prod-1",
                    account_name: "生产账号 A2",
                    enabled: false,
                  }),
                ],
              }),
            ],
          }),
          secretPatches: [
            {
              path: "/providers/0/credentials/0/api_key",
              operation: "replace",
              value: "sk-replaced-account",
            },
          ],
        }),
      ),
    );
  });

  it("directs missing credential setup to Advanced JSON without overstating structured editor support", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        document: {
          ...initialResponse.routeConfig.document,
          providers: [],
        },
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);

    expect(
      screen.getByText(/请在高级 JSON 中添加 credentials，再回到这里管理账号池/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/路由编辑或高级 JSON 中补充 credentials/i)).not.toBeInTheDocument();

    await openWorkspace(user, /凭证分组|分组策略/i);
    expect(screen.getByText(/当前还没有任何凭证分组/i)).toBeInTheDocument();
  });

  it("renders credential groups as a directory plus detail editor", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              preset: "openai",
              base_url: "https://api.example.com/v1",
              credentials: [
                {
                  id: "acc-prod-1",
                  account_name: "生产账号 A",
                  api_key: "sk-prod-a",
                  supported_models: ["gpt-5.4"],
                },
                {
                  id: "acc-prod-2",
                  account_name: "生产账号 B",
                  api_key: "sk-prod-b",
                  supported_models: ["gpt-5.4-mini"],
                },
              ],
            },
          ],
          model_routes: [{ pattern: "gpt-5.4", provider_ids: ["managed-provider"] }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: ["acc-prod-1"],
            },
          ],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭证分组|分组策略/i);

    expect(screen.getByRole("navigation", { name: /分组列表/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /VIP 分组/i })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /分组详情/i })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: /分组 ID/i })).toHaveValue("group-vip");
    expect(screen.getByRole("region", { name: /成员管理/i })).toBeInTheDocument();
  });

  it("adds a group and manages members from compact candidate rows while keeping the JSON draft in sync", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "managed-provider",
              label: "Managed OpenAI",
              preset: "openai",
              base_url: "https://api.example.com/v1",
              credentials: [
                {
                  id: "acc-prod-1",
                  account_name: "生产账号 A",
                  api_key: "sk-prod-a",
                  supported_models: ["gpt-5.4"],
                },
                {
                  id: "acc-prod-2",
                  account_name: "生产账号 B",
                  api_key: "sk-prod-b",
                  supported_models: ["gpt-5.4-mini"],
                },
              ],
            },
          ],
          model_routes: [{ pattern: "gpt-5.4", provider_ids: ["managed-provider"] }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: ["acc-prod-1"],
            },
          ],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭证分组|分组策略/i);

    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组 ID/i), "group-team-b");
    await user.type(screen.getByLabelText(/分组名称/i), "Team B");
    await user.clear(screen.getByLabelText(/计费倍率/i));
    await user.type(screen.getByLabelText(/计费倍率/i), "0.8");
    await user.type(screen.getByRole("searchbox", { name: /筛选候选账号/i }), "生产账号 B");
    await user.click(screen.getByRole("button", { name: /加入.*生产账号 B/i }));

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"account_groups": [');
    expect((editor as HTMLTextAreaElement).value).toContain('"group-team-b"');
    expect((editor as HTMLTextAreaElement).value).toContain('"acc-prod-2"');

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));

    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            account_groups: expect.arrayContaining([
              expect.objectContaining({
                id: "group-team-b",
                name: "Team B",
                billing_multiplier: 0.8,
                provider_credential_ids: ["acc-prod-2"],
              }),
            ]),
          }),
        }),
      ),
    );
  });

  it("marks incomplete account-group drafts and blocks saving until the group ID is filled", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭证分组|分组策略/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组名称/i), "Incomplete group");

    expect(screen.getByLabelText(/分组 ID/i)).toHaveAttribute("aria-invalid", "true");
    expect(screen.getAllByText(/分组 ID 必须填写/i).length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent(/分组 ID 必须填写/i);
  });

  it("refreshes draft state even when the active revision ID is unchanged", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const activeResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockClear();
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue(activeResponse);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭证分组|分组策略/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组 ID/i), "temporary-group");
    expect(screen.getByLabelText(/分组 ID/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^刷新$/i }));

    const discardDialog = screen.getByRole("dialog", { name: /丢弃未保存修改/i });
    expect(discardDialog).toBeInTheDocument();
    expect(screen.getByLabelText(/分组 ID/i)).toHaveValue("temporary-group");
    await user.click(within(discardDialog).getByRole("button", { name: /继续编辑/i }));
    expect(screen.getByLabelText(/分组 ID/i)).toHaveValue("temporary-group");

    await user.click(screen.getByRole("button", { name: /^刷新$/i }));
    await user.click(
      within(screen.getByRole("dialog", { name: /丢弃未保存修改/i })).getByRole("button", {
        name: /丢弃并刷新/i,
      }),
    );

    await waitFor(() => {
      expect(screen.queryByLabelText(/分组 ID/i)).not.toBeInTheDocument();
    });
  });

  it("registers a beforeunload guard only while the route draft is dirty", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    const cleanEvent = new Event("beforeunload", { cancelable: true });
    expect(window.dispatchEvent(cleanEvent)).toBe(true);
    expect(cleanEvent.defaultPrevented).toBe(false);

    await openWorkspace(user, /凭证分组|分组策略/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组 ID/i), "temporary-group");

    const dirtyEvent = new Event("beforeunload", { cancelable: true });
    expect(window.dispatchEvent(dirtyEvent)).toBe(false);
    expect(dirtyEvent.defaultPrevented).toBe(true);
  });

  it("keeps an unsaved draft intact when only the interface language changes", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭证分组|分组策略/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组 ID/i), "temporary-group");
    expect(consoleApi.getRouteConfig).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: /切换界面语言/i }));

    expect(consoleApi.getRouteConfig).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText(/Group ID/i)).toHaveValue("temporary-group");
  });

  it("clears validation feedback when refresh replaces the authoritative draft", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
    expect(await screen.findByText(/草稿校验通过/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^刷新$/i }));

    await waitFor(() =>
      expect(screen.queryByText(/草稿校验通过/i)).not.toBeInTheDocument(),
    );
  });

  it("disables manual refresh while an authoritative refresh is in flight", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    const pendingRefresh = deferred<typeof initialResponse>();
    vi.mocked(consoleApi.getRouteConfig).mockReset();
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialResponse)
      .mockReturnValueOnce(pendingRefresh.promise);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    const refreshButton = screen.getByRole("button", { name: /^刷新$/i });
    await user.click(refreshButton);

    expect(refreshButton).toBeDisabled();
    pendingRefresh.resolve(JSON.parse(JSON.stringify(initialResponse)));
    await waitFor(() => expect(refreshButton).toBeEnabled());
  });

  it("ignores stale authoritative refresh responses that finish out of order", async () => {
    const consoleApi = createConsoleApi();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    const staleRefresh = deferred<typeof initialResponse>();
    const latestRefresh = deferred<typeof initialResponse>();
    const responseWithProvider = (providerId: string, revisionId: string) => ({
      routeConfig: {
        ...initialResponse.routeConfig,
        revision: {
          ...initialResponse.routeConfig.revision,
          id: revisionId,
        },
        document: {
          ...initialResponse.routeConfig.document,
          providers: [{ id: providerId }],
        },
      },
    });
    vi.mocked(consoleApi.getRouteConfig).mockReset();
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialResponse)
      .mockReturnValueOnce(staleRefresh.promise)
      .mockReturnValueOnce(latestRefresh.promise);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    const refreshButton = screen.getByRole("button", { name: /^刷新$/i });
    act(() => {
      refreshButton.click();
      refreshButton.click();
    });
    await waitFor(() => expect(consoleApi.getRouteConfig).toHaveBeenCalledTimes(3));

    latestRefresh.resolve(responseWithProvider("latest-provider", "r3-latest"));
    await waitFor(() => expect(screen.getAllByText("latest-provider").length).toBeGreaterThan(0));

    await act(async () => {
      staleRefresh.resolve(responseWithProvider("stale-provider", "r2-stale"));
      await staleRefresh.promise;
      await Promise.resolve();
    });

    expect(screen.getAllByText("latest-provider").length).toBeGreaterThan(0);
    expect(screen.queryByText("stale-provider")).not.toBeInTheDocument();
  });

  it("keeps the latest refresh busy and preserves validation when an older refresh fails", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    const staleRefresh = deferred<typeof initialResponse>();
    const latestRefresh = deferred<typeof initialResponse>();
    vi.mocked(consoleApi.getRouteConfig).mockReset();
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialResponse)
      .mockReturnValueOnce(staleRefresh.promise)
      .mockReturnValueOnce(latestRefresh.promise);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
    expect(await screen.findByText(/草稿校验通过/i)).toBeInTheDocument();

    const refreshButton = screen.getByRole("button", { name: /^刷新$/i });
    act(() => {
      refreshButton.click();
      refreshButton.click();
    });
    await waitFor(() => expect(consoleApi.getRouteConfig).toHaveBeenCalledTimes(3));

    await act(async () => {
      staleRefresh.reject(new Error("stale refresh failed"));
      await Promise.resolve();
    });

    expect(screen.queryByText("stale refresh failed")).not.toBeInTheDocument();
    expect(screen.getByText(/草稿校验通过/i)).toBeInTheDocument();
    expect(refreshButton).toBeDisabled();

    latestRefresh.resolve(initialResponse);
    await waitFor(() => expect(refreshButton).toBeEnabled());
    expect(screen.queryByText(/草稿校验通过/i)).not.toBeInTheDocument();
  });

  it("preserves validation feedback when the latest refresh fails", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
    expect(await screen.findByText(/草稿校验通过/i)).toBeInTheDocument();

    vi.mocked(consoleApi.getRouteConfig).mockRejectedValueOnce(new Error("latest refresh failed"));
    await user.click(screen.getByRole("button", { name: /^刷新$/i }));

    expect(await screen.findByText("latest refresh failed")).toBeInTheDocument();
    expect(screen.getByText(/草稿校验通过/i)).toBeInTheDocument();
  });

  it("strictly validates account-group billing multipliers before validating or saving", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [{ id: "managed-provider" }],
          model_routes: [{ pattern: "gpt-5.4", provider_ids: ["managed-provider"] }],
          aliases: { answer: "gpt-5.4" },
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: [],
            },
          ],
        },
        secrets: [{ path: "/providers/0/api_key", configured: true, preview: "sk-***" }],
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭证分组|分组策略/i);
    const multiplier = screen.getByLabelText(/计费倍率/i);

    fireEvent.change(multiplier, { target: { value: "1abc" } });
    expect(multiplier).toHaveValue("1abc");
    expect(multiplier).toHaveAttribute("aria-invalid", "true");
    expect(multiplier).toHaveAttribute("aria-describedby");
    expect(screen.getByRole("alert")).toHaveTextContent(/计费倍率必须是大于等于 0 的数字/i);

    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
    expect(consoleApi.validateRouteConfig).not.toHaveBeenCalled();

    fireEvent.change(multiplier, { target: { value: "-1" } });
    expect(multiplier).toHaveValue("-1");
    expect(screen.getByRole("alert")).toHaveTextContent(/计费倍率必须是大于等于 0 的数字/i);

    fireEvent.change(multiplier, { target: { value: "1e-2" } });
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(multiplier).toHaveAttribute("aria-invalid", "false");

    await openWorkspace(user, /高级 JSON/i);
    expect(
      (screen.getByRole("textbox", { name: /路由配置 JSON/i }) as HTMLTextAreaElement).value,
    ).toContain('"billing_multiplier": 0.01');

    await user.click(screen.getByRole("button", { name: /校验草稿/i }));
    await waitFor(() =>
      expect(consoleApi.validateRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            account_groups: [
              expect.objectContaining({
                id: "group-vip",
                billing_multiplier: 0.01,
              }),
            ],
          }),
        }),
      ),
    );
  });

  it("locks every draft editing control when the active route is read-only", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const initialResponse = await consoleApi.getRouteConfig("management-secret");
    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...initialResponse.routeConfig,
        mutationSupported: false,
        document: {
          ...initialResponse.routeConfig.document,
          providers: [
            {
              id: "managed-provider",
              credentials: [{ id: "account-1", account_name: "Account 1" }],
            },
          ],
          account_groups: [
            {
              id: "group-1",
              name: "Group 1",
              billing_multiplier: 1,
              provider_credential_ids: ["account-1"],
            },
          ],
        },
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(screen.getByRole("button", { name: /校验草稿/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /保存路由配置/i })).toBeDisabled();
    expect(screen.getByLabelText(/修订说明/i)).toBeDisabled();

    await openWorkspace(user, /路由编辑/i);
    expect(screen.getByRole("button", { name: /添加 Alias 行/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /添加模型路由行/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /添加 Provider 行/i })).toBeDisabled();
    expect(screen.getByLabelText(/Alias 名称 1/i)).toBeDisabled();
    expect(screen.getByLabelText(/模型路由模式 1/i)).toBeDisabled();
    expect(screen.getByLabelText(/Provider ID 1/i)).toBeDisabled();

    await openWorkspace(user, /凭证分组|分组策略/i);
    expect(screen.getByRole("button", { name: /添加分组/i })).toBeDisabled();
    expect(screen.getByLabelText(/分组 ID/i)).toBeDisabled();
    expect(screen.getByLabelText(/分组名称/i)).toBeDisabled();
    expect(screen.getByRole("button", { name: /移除.*Account 1/i })).toBeDisabled();

    await openWorkspace(user, /高级 JSON/i);
    expect(screen.getByRole("textbox", { name: /路由配置 JSON/i })).toBeDisabled();
  });

  it("starts a browser-first Gemini Canvas manual-add flow and merges both Canvas families into the draft", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    const initialRouteConfig = {
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis" as const,
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "gemini-canvas",
              label: "gemini-canvas",
              vendor_key: "google-gemini",
              vendor_name: "Google / Gemini",
              preset: "gemini-canvas",
              base_url: "https://gemini.google.com",
              supported_models: ["gemini-2.5-flash-image-preview"],
            },
            {
              id: "gemini-canvas-chat",
              label: "gemini-canvas-chat",
              vendor_key: "google-gemini",
              vendor_name: "Google / Gemini",
              preset: "gemini-canvas-chat",
              base_url: "https://gemini.google.com",
              supported_models: ["gemini-2.5-flash"],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        secrets: [],
        mutationSupported: true,
      },
    };
    const committedRouteConfig = {
      routeConfig: {
        ...initialRouteConfig.routeConfig,
        revision: { id: "r2-gemini-import", sequence: 2, message: "Import Gemini manual credentials (gemini-session-1)" },
        document: {
          ...initialRouteConfig.routeConfig.document,
          providers: [
            {
              ...initialRouteConfig.routeConfig.document.providers[0],
              credentials: [
                {
                  id: "gemini-canvas-manual-1",
                  account_name: "Gemini Canvas Manual 1",
                  runtime_state_object_key:
                    "credential-runtime/gemini-canvas/manual-1/storage-state.json",
                  extra_body: {
                    shareId: "fe24c455a570",
                  },
                },
              ],
            },
            {
              ...initialRouteConfig.routeConfig.document.providers[1],
              credentials: [
                {
                  id: "gemini-canvas-chat-manual-1",
                  account_name: "Gemini Canvas Chat Manual 1",
                  runtime_state_object_key:
                    "credential-runtime/gemini-canvas/manual-1/storage-state.json",
                  extra_body: {
                    shareId: "fe24c455a570",
                    apiBaseUrl: "https://generativelanguage.googleapis.com/v1beta",
                  },
                },
              ],
            },
          ],
        },
      },
    };
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialRouteConfig)
      .mockResolvedValue(committedRouteConfig);
    vi.mocked(consoleApi.commitRouteConfig).mockResolvedValue({
      routeConfig: committedRouteConfig.routeConfig,
      committed: true,
    });
    vi.mocked(consoleApi.createGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Complete Gemini login in the opened browser window.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:00:00Z",
        generatedDrafts: [],
      },
    });
    vi.mocked(consoleApi.getGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-session-1",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "succeeded",
        message: "Gemini Canvas runtime captured.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:02:00Z",
        generatedDrafts: [
          {
            providerId: "gemini-canvas",
            credential: {
              id: "gemini-canvas-manual-1",
              account_name: "Gemini Canvas Manual 1",
              runtime_state_object_key:
                "credential-runtime/gemini-canvas/manual-1/storage-state.json",
              extra_body: {
                shareId: "fe24c455a570",
              },
            },
            secretEdits: [],
          },
          {
            providerId: "gemini-canvas-chat",
            credential: {
              id: "gemini-canvas-chat-manual-1",
              account_name: "Gemini Canvas Chat Manual 1",
              runtime_state_object_key:
                "credential-runtime/gemini-canvas/manual-1/storage-state.json",
              extra_body: {
                shareId: "fe24c455a570",
                apiBaseUrl: "https://generativelanguage.googleapis.com/v1beta",
              },
            },
            secretEdits: [],
          },
        ],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^Gemini Canvas$/i }));

    const canvasArticle = screen.getByRole("button", { name: /^Gemini Canvas$/i }).closest("article");
    expect(canvasArticle).not.toBeNull();
    await user.click(
      within(canvasArticle as HTMLElement).getByRole("button", { name: /手动添加|Manual add/i }),
    );

    await waitFor(() =>
      expect(consoleApi.createGeminiAuthSession).toHaveBeenCalledWith("management-secret", {
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
      }),
    );

    const dialog = screen.getByRole("dialog", { name: /Gemini 手动添加|Gemini manual add/i });
    expect(within(dialog).getByText("gemini-canvas")).toBeInTheDocument();

    await waitFor(() =>
      expect(consoleApi.getGeminiAuthSession).toHaveBeenCalledWith(
        "management-secret",
        "gemini-session-1",
      ),
    );
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: expect.arrayContaining([
              expect.objectContaining({
                id: "gemini-canvas",
                credentials: expect.arrayContaining([
                  expect.objectContaining({ id: "gemini-canvas-manual-1" }),
                ]),
              }),
              expect.objectContaining({
                id: "gemini-canvas-chat",
                credentials: expect.arrayContaining([
                  expect.objectContaining({ id: "gemini-canvas-chat-manual-1" }),
                ]),
              }),
            ]),
          }),
        }),
      ),
    );

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"id": "gemini-canvas-manual-1"');
    expect((editor as HTMLTextAreaElement).value).toContain('"id": "gemini-canvas-chat-manual-1"');
    expect((editor as HTMLTextAreaElement).value).toContain('"shareId": "fe24c455a570"');
    expect((editor as HTMLTextAreaElement).value).toContain(
      '"apiBaseUrl": "https://generativelanguage.googleapis.com/v1beta"',
    );
  });

  it("captures Gemini Business runtime material and keeps its JWT in a secret patch", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "gemini-business",
              label: "gemini-business",
              vendor_key: "google-gemini",
              vendor_name: "Google / Gemini",
              preset: "gemini-business",
              base_url: "https://biz-discoveryengine.googleapis.com/v1alpha",
              supported_models: ["nano-banana-pro"],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        secrets: [],
        mutationSupported: true,
      },
    });
    vi.mocked(consoleApi.createGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-business-session-1",
        targetFamily: "gemini-business",
        providerId: "gemini-business",
        status: "waiting_user",
        message:
          "Complete Gemini Business login, then trigger one Gemini Business request in the opened browser window.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:00:00Z",
        generatedDrafts: [],
      },
    });
    vi.mocked(consoleApi.getGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-business-session-1",
        targetFamily: "gemini-business",
        providerId: "gemini-business",
        status: "succeeded",
        message: "Gemini Business runtime captured.",
        createdAt: "2026-07-30T09:00:00Z",
        updatedAt: "2026-07-30T09:00:03Z",
        generatedDrafts: [
          {
            providerId: "gemini-business",
            credential: {
              id: "gemini-business-manual-1",
              account_name: "Gemini Business Manual 1",
              api_key: "",
              extra_body: {
                configId: "cfg-123",
                session: "projects/demo/sessions/abc",
              },
            },
            secretEdits: [
              {
                field: "api_key",
                operation: "replace",
                value: "ey.demo.jwt",
              },
            ],
          },
        ],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "gemini-business-grant", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^Gemini Business$/i }));

    const businessArticle = screen
      .getByRole("button", { name: /^Gemini Business$/i })
      .closest("article");
    expect(businessArticle).not.toBeNull();
    await user.click(
      within(businessArticle as HTMLElement).getByRole("button", { name: /手动添加|Manual add/i }),
    );

    await waitFor(() =>
      expect(consoleApi.createGeminiAuthSession).toHaveBeenCalledWith("management-secret", {
        targetFamily: "gemini-business",
        providerId: "gemini-business",
      }),
    );

    const dialog = screen.getByRole("dialog", { name: /Gemini 手动添加|Gemini manual add/i });
    expect(within(dialog).getByText("gemini-business")).toBeInTheDocument();

    await waitFor(() =>
      expect(consoleApi.getGeminiAuthSession).toHaveBeenCalledWith(
        "management-secret",
        "gemini-business-session-1",
      ),
    );

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain('"id": "gemini-business-manual-1"');
    expect((editor as HTMLTextAreaElement).value).toContain('"configId": "cfg-123"');
    expect((editor as HTMLTextAreaElement).value).toContain(
      '"session": "projects/demo/sessions/abc"',
    );

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          secretPatches: expect.arrayContaining([
            expect.objectContaining({
              operation: "replace",
              value: "ey.demo.jwt",
            }),
          ]),
        }),
        "gemini-business-grant",
      ),
    );
  });

  it("allows the operator to manually continue Gemini Canvas import after finishing login", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    const initialRouteConfig = {
      routeConfig: {
        revision: { id: "r1-deadbeefcafe", sequence: 1, message: "initial import" },
        source: "redis" as const,
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "gemini-canvas",
              label: "gemini-canvas",
              vendor_key: "google-gemini",
              vendor_name: "Google / Gemini",
              preset: "gemini-canvas",
              base_url: "https://gemini.google.com",
              supported_models: ["gemini-2.5-flash"],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
        secrets: [],
        mutationSupported: true,
      },
    };
    const committedRouteConfig = {
      routeConfig: {
        ...initialRouteConfig.routeConfig,
        revision: {
          id: "r2-gemini-manual-complete",
          sequence: 2,
          message: "Import Gemini manual credentials (gemini-session-manual-complete)",
        },
        document: {
          ...initialRouteConfig.routeConfig.document,
          providers: [
            {
              ...initialRouteConfig.routeConfig.document.providers[0],
              credentials: [
                {
                  id: "gemini-canvas-manual-complete-1",
                  account_name: "Gemini Canvas Manual Complete 1",
                  runtime_state_object_key:
                    "credential-runtime/gemini-canvas/manual-complete/storage-state.json",
                  extra_body: {
                    shareId: "fe24c455a570",
                  },
                },
              ],
            },
          ],
        },
      },
    };
    vi.mocked(consoleApi.getRouteConfig)
      .mockResolvedValueOnce(initialRouteConfig)
      .mockResolvedValue(committedRouteConfig);
    vi.mocked(consoleApi.commitRouteConfig).mockResolvedValue({
      routeConfig: committedRouteConfig.routeConfig,
      committed: true,
    });
    vi.mocked(consoleApi.createGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-session-manual-complete",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Complete Gemini login in the opened browser window.",
        createdAt: "2026-07-30T12:00:00Z",
        updatedAt: "2026-07-30T12:00:00Z",
        generatedDrafts: [],
      },
    });
    vi.mocked(consoleApi.completeGeminiAuthSession).mockResolvedValue({
      session: {
        id: "gemini-session-manual-complete",
        targetFamily: "gemini-canvas",
        providerId: "gemini-canvas",
        status: "waiting_user",
        message: "Manual Gemini import requested. Finishing capture.",
        createdAt: "2026-07-30T12:00:00Z",
        updatedAt: "2026-07-30T12:00:02Z",
        generatedDrafts: [],
      },
    });
    vi.mocked(consoleApi.getGeminiAuthSession)
      .mockResolvedValueOnce({
        session: {
          id: "gemini-session-manual-complete",
          targetFamily: "gemini-canvas",
          providerId: "gemini-canvas",
          status: "waiting_user",
          message: "Complete Gemini login in the opened browser window.",
          createdAt: "2026-07-30T12:00:00Z",
          updatedAt: "2026-07-30T12:00:00Z",
          generatedDrafts: [],
        },
      })
      .mockResolvedValue({
        session: {
          id: "gemini-session-manual-complete",
          targetFamily: "gemini-canvas",
          providerId: "gemini-canvas",
          status: "succeeded",
          message: "Gemini Canvas runtime captured.",
          createdAt: "2026-07-30T12:00:00Z",
          updatedAt: "2026-07-30T12:00:03Z",
          generatedDrafts: [
            {
              providerId: "gemini-canvas",
              credential: {
                id: "gemini-canvas-manual-complete-1",
                account_name: "Gemini Canvas Manual Complete 1",
                runtime_state_object_key:
                  "credential-runtime/gemini-canvas/manual-complete/storage-state.json",
                extra_body: {
                  shareId: "fe24c455a570",
                },
              },
              secretEdits: [],
            },
          ],
        },
      });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /^Gemini Canvas$/i }));

    const canvasArticle = screen.getByRole("button", { name: /^Gemini Canvas$/i }).closest("article");
    expect(canvasArticle).not.toBeNull();
    await user.click(
      within(canvasArticle as HTMLElement).getByRole("button", { name: /手动添加|Manual add/i }),
    );

    const dialog = screen.getByRole("dialog", { name: /Gemini 手动添加|Gemini manual add/i });
    const continueButton = within(dialog).getByRole("button", {
      name: /已完成登录.*导入|I finished login.*import/i,
    });
    await user.click(continueButton);

    await waitFor(() =>
      expect(consoleApi.completeGeminiAuthSession).toHaveBeenCalledWith(
        "management-secret",
        "gemini-session-manual-complete",
      ),
    );

    await waitFor(() =>
      expect(consoleApi.getGeminiAuthSession).toHaveBeenCalledWith(
        "management-secret",
        "gemini-session-manual-complete",
      ),
    );
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: expect.arrayContaining([
              expect.objectContaining({
                id: "gemini-canvas",
                credentials: expect.arrayContaining([
                  expect.objectContaining({ id: "gemini-canvas-manual-complete-1" }),
                ]),
              }),
            ]),
          }),
        }),
      ),
    );

    await openWorkspace(user, /高级 JSON/i);
    const editor = screen.getByRole("textbox", { name: /路由配置 JSON/i });
    expect((editor as HTMLTextAreaElement).value).toContain(
      '"id": "gemini-canvas-manual-complete-1"',
    );
  });
});
