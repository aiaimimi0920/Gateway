import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConsoleApi } from "../../api/console";
import { GatewayApiError } from "../../api/errors";
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
    expect(screen.getByRole("heading", { name: /Gateway 网页控制台/i })).toBeInTheDocument(),
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

async function mockCredentialRoute(
  consoleApi: ConsoleApi,
  enabled = true,
  includeSecondAccount = false,
) {
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
                enabled,
                supported_models: ["gpt-5.4"],
              },
              ...(includeSecondAccount
                ? [
                    {
                      id: "acc-prod-2",
                      account_name: "生产账号 B",
                      enabled,
                      supported_models: ["gpt-5.4-mini"],
                    },
                  ]
                : []),
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
    expect(workspaceButton(/分组策略/i)).toBeInTheDocument();
    expect(workspaceButton(/敏感信息/i)).toBeInTheDocument();
    expect(workspaceButton(/修订历史/i)).toBeInTheDocument();
    expect(workspaceButton(/高级 JSON/i)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Provider 概览/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Alias 概览/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /模型路由概览/i })).toBeInTheDocument();
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
    await user.type(screen.getByLabelText(/Provider ID 2/i), "backup-provider");
    await user.type(screen.getByLabelText(/Provider 预设 2/i), "openai");
    await user.type(
      screen.getByLabelText(/Provider 基础 URL 2/i),
      "https://api.backup.example.com",
    );
    await user.type(
      screen.getByLabelText(/Provider 支持模型 2/i),
      "gpt-5.4{enter}gpt-5.4-mini",
    );

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

  it("renders route-config accounts grouped by provider and shows group assignments in the accounts workspace", async () => {
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

    expect(screen.getAllByRole("heading", { name: /账号台账/i }).length).toBeGreaterThan(0);
    expect(screen.getByText("Managed OpenAI")).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: /Managed OpenAI · 2 个账号/i }).closest("details"),
    ).not.toBeNull();
    expect(screen.getByText("生产账号 A")).toBeInTheDocument();
    expect(screen.getByText("生产账号 B")).toBeInTheDocument();
    expect(screen.getAllByText("VIP 分组").length).toBeGreaterThan(0);
  });

  it("filters the accounts workspace by query, group membership, and enabled state", async () => {
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

    const accountSearch = screen.getByRole("searchbox", { name: /筛选账号/i });
    await user.type(accountSearch, "生产账号 B");
    expect(screen.queryByText("生产账号 A")).not.toBeInTheDocument();
    expect(screen.getByText("生产账号 B")).toBeInTheDocument();

    await user.clear(accountSearch);
    await user.selectOptions(screen.getByLabelText(/分组状态/i), "ungrouped");
    expect(screen.queryByText("生产账号 A")).not.toBeInTheDocument();
    expect(screen.getByText("生产账号 B")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText(/分组状态/i), "all");
    await user.selectOptions(screen.getByLabelText(/启用状态/i), "disabled");
    expect(screen.queryByText("生产账号 A")).not.toBeInTheDocument();
    const disabledAccount = screen.getByText("生产账号 B").closest("li");
    expect(disabledAccount).not.toBeNull();
    expect(within(disabledAccount as HTMLElement).getByText("已停用")).toBeInTheDocument();
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

    expect(screen.getAllByText("Managed OpenAI").length).toBeGreaterThan(0);
    expect(screen.getByText("生产账号 A")).toBeInTheDocument();
    const summaryAccount = screen.getByText("生产账号 A").closest("li");
    expect(summaryAccount).not.toBeNull();
    expect(within(summaryAccount as HTMLElement).getByText("已停用")).toBeInTheDocument();
    expect(screen.getAllByText("VIP 分组").length).toBeGreaterThan(0);
    expect(screen.getByText("分组数量").nextElementSibling).toHaveTextContent("1");
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

    expect(
      screen.getByRole("heading", { name: /Alibaba \/ Qwen.*2 个账号/i }),
    ).toBeInTheDocument();
    expect(screen.getByText("Qwen OpenAI")).toBeInTheDocument();
    expect(screen.getByText("Qwen Web")).toBeInTheDocument();
    expect(screen.getByText("服务商").nextElementSibling).toHaveTextContent("1");
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
    await user.click(screen.getByRole("button", { name: /添加账号/i }));

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

    expect(screen.getByText("生产账号 A")).toBeInTheDocument();
    expect(screen.getByRole("status", { name: /draft status/i })).toHaveTextContent(
      /有未保存修改/i,
    );

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
    await user.click(screen.getByRole("button", { name: /编辑账号 生产账号 A/i }));

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

    expect(screen.getByText("生产账号 A2")).toBeInTheDocument();
    const editedAccount = screen.getByText("生产账号 A2").closest("li");
    expect(editedAccount).not.toBeNull();
    expect(within(editedAccount as HTMLElement).getByText("已停用")).toBeInTheDocument();
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

  it("probes an active credential through the standalone gateway console", async () => {
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
      secretGrant: {
        grant: "grant-1",
        expiresAt: "2026-07-28T03:00:00Z",
      },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /测试账号 生产账号 A/i }));

    await waitFor(() =>
      expect(screen.getByText(/连通正常|Connectivity passed/i)).toBeInTheDocument(),
    );
    expect(consoleApi.probeCredential).toHaveBeenCalledWith(
      "management-secret",
      "grant-1",
      "acc-prod-1",
    );
  });

  it("renders a failed probe result without changing the route draft", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    await mockCredentialRoute(consoleApi);
    vi.mocked(consoleApi.probeCredential).mockResolvedValue({
      result: {
        credentialId: "acc-prod-1",
        providerId: "managed-provider",
        status: "failed",
        message: "Upstream rejected the credential.",
        checkedAt: "2026-07-28T02:01:00Z",
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /测试账号 生产账号 A/i }));

    expect(await screen.findByText("连接失败")).toBeInTheDocument();
    expect(screen.getByText("Upstream rejected the credential.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: /修订说明/i })).toHaveValue("initial import");
  });

  it("opens secret access confirmation instead of probing without a secret grant", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    await mockCredentialRoute(consoleApi);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /测试账号 生产账号 A/i }));

    expect(consoleApi.probeCredential).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toHaveTextContent(/确认敏感信息访问权限/i);
  });

  it("clears an existing probe result with a stale secret grant and reopens confirmation", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const clearSecretGrant = vi.fn();
    await mockCredentialRoute(consoleApi);
    vi.mocked(consoleApi.probeCredential)
      .mockResolvedValueOnce({
        result: {
          credentialId: "acc-prod-1",
          providerId: "managed-provider",
          status: "passed",
          message: "Credential connectivity probe passed.",
          checkedAt: "2026-07-28T02:00:00Z",
        },
      })
      .mockRejectedValueOnce(
        new GatewayApiError(
          "A fresh secret grant is required.",
          403,
          "console_secret_access_required",
        ),
      );

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "stale-grant", expiresAt: "2099-01-01T00:00:00Z" },
      clearSecretGrant,
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    const probeButton = screen.getByRole("button", { name: /测试账号 生产账号 A/i });
    await user.click(probeButton);
    expect(await screen.findByText(/连通正常|Connectivity passed/i)).toBeInTheDocument();

    await user.click(probeButton);

    await waitFor(() => expect(clearSecretGrant).toHaveBeenCalledOnce());
    expect(await screen.findByRole("dialog")).toHaveTextContent(/确认敏感信息访问权限/i);
    expect(screen.queryByText(/连通正常|Connectivity passed/i)).not.toBeInTheDocument();
    expect(screen.queryByText("Credential connectivity probe passed.")).not.toBeInTheDocument();
    expect(screen.queryByText("A fresh secret grant is required.")).not.toBeInTheDocument();
  });

  it("clears every account probe result when the shared secret grant expires", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const clearSecretGrant = vi.fn();
    await mockCredentialRoute(consoleApi, true, true);
    vi.mocked(consoleApi.probeCredential)
      .mockResolvedValueOnce({
        result: {
          credentialId: "acc-prod-1",
          providerId: "managed-provider",
          status: "passed",
          message: "Account A connectivity passed.",
          checkedAt: "2026-07-28T02:00:00Z",
        },
      })
      .mockResolvedValueOnce({
        result: {
          credentialId: "acc-prod-2",
          providerId: "managed-provider",
          status: "passed",
          message: "Account B connectivity passed.",
          checkedAt: "2026-07-28T02:01:00Z",
        },
      })
      .mockRejectedValueOnce(
        new GatewayApiError(
          "A fresh secret grant is required.",
          403,
          "console_secret_access_required",
        ),
      );

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "shared-stale-grant", expiresAt: "2099-01-01T00:00:00Z" },
      clearSecretGrant,
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    const accountAButton = screen.getByRole("button", { name: /测试账号 生产账号 A/i });
    const accountBButton = screen.getByRole("button", { name: /测试账号 生产账号 B/i });
    await user.click(accountAButton);
    expect(await screen.findByText("Account A connectivity passed.")).toBeInTheDocument();
    await user.click(accountBButton);
    expect(await screen.findByText("Account B connectivity passed.")).toBeInTheDocument();

    await user.click(accountBButton);

    await waitFor(() => expect(clearSecretGrant).toHaveBeenCalledOnce());
    expect(await screen.findByRole("dialog")).toHaveTextContent(/确认敏感信息访问权限/i);
    expect(screen.queryByText("Account A connectivity passed.")).not.toBeInTheDocument();
    expect(screen.queryByText("Account B connectivity passed.")).not.toBeInTheDocument();
  });

  it("keeps the secret grant when a probe returns another 403 error", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const clearSecretGrant = vi.fn();
    await mockCredentialRoute(consoleApi);
    vi.mocked(consoleApi.probeCredential).mockRejectedValue(
      new GatewayApiError(
        "Remote console access is forbidden.",
        403,
        "console_remote_access_forbidden",
      ),
    );

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
      clearSecretGrant,
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /测试账号 生产账号 A/i }));

    expect(await screen.findByText("Remote console access is forbidden.")).toBeInTheDocument();
    expect(screen.getByText(/测试异常|Probe error/i)).toBeInTheDocument();
    expect(clearSecretGrant).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("keeps the secret grant when a probe returns 401 with the secret-access code", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const clearSecretGrant = vi.fn();
    await mockCredentialRoute(consoleApi);
    vi.mocked(consoleApi.probeCredential).mockRejectedValue(
      new GatewayApiError(
        "Management authentication expired.",
        401,
        "console_secret_access_required",
      ),
    );

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
      clearSecretGrant,
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /测试账号 生产账号 A/i }));

    expect(await screen.findByText("Management authentication expired.")).toBeInTheDocument();
    expect(screen.getByText(/测试异常|Probe error/i)).toBeInTheDocument();
    expect(clearSecretGrant).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("disables probing for a disabled credential", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    await mockCredentialRoute(consoleApi, false);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    const probeButton = screen.getByRole("button", { name: /测试账号 生产账号 A/i });
    expect(probeButton).toBeDisabled();
    expect(consoleApi.probeCredential).not.toHaveBeenCalled();
  });

  it("probes a provider default account using its stable default identity", async () => {
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
              enabled: true,
              supported_models: ["gpt-5.4"],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [],
        },
      },
    });
    vi.mocked(consoleApi.probeCredential).mockResolvedValue({
      result: {
        credentialId: "managed-provider::default",
        providerId: "managed-provider",
        status: "unsupported",
        message: "Provider default probe is unsupported.",
        checkedAt: "2026-07-28T02:02:00Z",
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /测试账号 Managed OpenAI/i }));

    expect(await screen.findByText("暂不支持测试")).toBeInTheDocument();
    expect(consoleApi.probeCredential).toHaveBeenCalledWith(
      "management-secret",
      "grant-1",
      "managed-provider::default",
    );
  });

  it("ignores a credential probe response that completes after an authoritative refresh", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const pendingProbe = deferred<Awaited<ReturnType<ConsoleApi["probeCredential"]>>>();
    await mockCredentialRoute(consoleApi);
    vi.mocked(consoleApi.probeCredential).mockReturnValue(pendingProbe.promise);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /测试账号 生产账号 A/i }));
    await user.click(screen.getByRole("button", { name: /^刷新$/i }));

    await act(async () => {
      pendingProbe.resolve({
        result: {
          credentialId: "acc-prod-1",
          providerId: "managed-provider",
          status: "passed",
          message: "Stale probe result must stay hidden.",
          checkedAt: "2026-07-28T02:05:00Z",
        },
      });
      await pendingProbe.promise;
    });

    expect(screen.queryByText("Stale probe result must stay hidden.")).not.toBeInTheDocument();
    expect(screen.queryByText(/连通正常|Connectivity passed/i)).not.toBeInTheDocument();
  });

  it("does not revoke secret access when a stale probe fails after refresh", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    const clearSecretGrant = vi.fn();
    const pendingProbe = deferred<Awaited<ReturnType<ConsoleApi["probeCredential"]>>>();
    await mockCredentialRoute(consoleApi);
    vi.mocked(consoleApi.probeCredential).mockReturnValue(pendingProbe.promise);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
      clearSecretGrant,
    });

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /测试账号 生产账号 A/i }));
    await user.click(screen.getByRole("button", { name: /^刷新$/i }));

    await act(async () => {
      pendingProbe.reject(
        new GatewayApiError(
          "Stale grant must not be cleared.",
          403,
          "console_secret_access_required",
        ),
      );
      await Promise.resolve();
    });

    expect(clearSecretGrant).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByText("Stale grant must not be cleared.")).not.toBeInTheDocument();
  });

  it("rebases keep patches when deleting a credential before another account", async () => {
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
                { id: "acc-prod-1", account_name: "生产账号 A" },
                { id: "acc-prod-2", account_name: "生产账号 B" },
              ],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [
            {
              id: "group-vip",
              name: "VIP 分组",
              billing_multiplier: 1.5,
              provider_credential_ids: ["acc-prod-2"],
            },
          ],
        },
        secrets: [
          {
            path: "/providers/0/credentials/0/api_key",
            configured: true,
            preview: "sk-a***",
          },
          {
            path: "/providers/0/credentials/1/api_key",
            configured: true,
            preview: "sk-b***",
          },
        ],
      },
    });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /账号台账/i);
    await user.click(screen.getByRole("button", { name: /删除账号 生产账号 A/i }));

    expect(confirm).toHaveBeenCalled();
    expect(screen.queryByText("生产账号 A")).not.toBeInTheDocument();
    expect(screen.getByText("生产账号 B")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /保存路由配置/i }));
    await waitFor(() =>
      expect(consoleApi.commitRouteConfig).toHaveBeenCalledWith(
        "management-secret",
        expect.objectContaining({
          document: expect.objectContaining({
            providers: [
              expect.objectContaining({
                credentials: [
                  expect.objectContaining({ id: "acc-prod-2", account_name: "生产账号 B" }),
                ],
              }),
            ],
            account_groups: [
              expect.objectContaining({
                id: "group-vip",
                provider_credential_ids: ["acc-prod-2"],
              }),
            ],
          }),
          secretPatches: [
            { path: "/providers/0/credentials/0/api_key", operation: "keep" },
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
  });

  it("updates account groups through the structured groups workspace and keeps the JSON draft in sync", async () => {
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
    await openWorkspace(user, /分组策略/i);

    expect(screen.getByRole("checkbox", { name: /VIP 分组.*启用状态/i })).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: /VIP 分组.*Managed OpenAI.*生产账号 A/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /VIP 分组.*可选账号/i })).toHaveAttribute(
      "tabindex",
      "0",
    );

    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组 ID 2/i), "group-team-b");
    await user.type(screen.getByLabelText(/分组名称 2/i), "Team B");
    await user.clear(screen.getByLabelText(/计费倍率 2/i));
    await user.type(screen.getByLabelText(/计费倍率 2/i), "0.8");
    const teamBCard = screen.getByRole("heading", { name: "Team B" }).closest("article");
    expect(teamBCard).not.toBeNull();
    const accountPickerSearch = within(teamBCard as HTMLElement).getByRole("searchbox", {
      name: /筛选关联账号 2/i,
    });
    await user.type(accountPickerSearch, "生产账号 B");
    expect(within(teamBCard as HTMLElement).queryByText("生产账号 A")).not.toBeInTheDocument();
    expect(within(teamBCard as HTMLElement).getByText("生产账号 B")).toBeInTheDocument();
    await user.click(within(teamBCard as HTMLElement).getByRole("checkbox", { name: /生产账号 B/i }));

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
    await openWorkspace(user, /分组策略/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组名称 1/i), "Incomplete group");

    expect(screen.getByRole("status", { name: /draft status/i })).toHaveTextContent(/有未保存修改/i);
    expect(screen.getByLabelText(/分组 ID 1/i)).toHaveAttribute("aria-invalid", "true");
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
    await openWorkspace(user, /分组策略/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组 ID 1/i), "temporary-group");
    await waitFor(() =>
      expect(screen.getByRole("status", { name: /draft status/i })).toHaveTextContent(
        /有未保存修改/i,
      ),
    );
    expect(screen.getByLabelText(/分组 ID 1/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^刷新$/i }));

    const discardDialog = screen.getByRole("dialog", { name: /丢弃未保存修改/i });
    expect(discardDialog).toBeInTheDocument();
    expect(screen.getByLabelText(/分组 ID 1/i)).toHaveValue("temporary-group");
    await user.click(within(discardDialog).getByRole("button", { name: /继续编辑/i }));
    expect(screen.getByLabelText(/分组 ID 1/i)).toHaveValue("temporary-group");

    await user.click(screen.getByRole("button", { name: /^刷新$/i }));
    await user.click(
      within(screen.getByRole("dialog", { name: /丢弃未保存修改/i })).getByRole("button", {
        name: /丢弃并刷新/i,
      }),
    );

    await waitFor(() => {
      expect(screen.queryByLabelText(/分组 ID 1/i)).not.toBeInTheDocument();
      expect(screen.getByRole("status", { name: /draft status/i })).toHaveTextContent(
        /草稿已同步/i,
      );
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

    await openWorkspace(user, /分组策略/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组 ID 1/i), "temporary-group");

    const dirtyEvent = new Event("beforeunload", { cancelable: true });
    expect(window.dispatchEvent(dirtyEvent)).toBe(false);
    expect(dirtyEvent.defaultPrevented).toBe(true);
  });

  it("keeps an unsaved draft intact when only the interface language changes", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /分组策略/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组 ID 1/i), "temporary-group");
    await waitFor(() =>
      expect(screen.getByRole("status", { name: /draft status/i })).toHaveTextContent(
        /有未保存修改/i,
      ),
    );
    expect(consoleApi.getRouteConfig).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: /切换界面语言/i }));

    expect(consoleApi.getRouteConfig).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText(/Group ID 1/i)).toHaveValue("temporary-group");
    expect(screen.getByRole("status", { name: /draft status/i })).toHaveTextContent(
      /Unsaved changes/i,
    );
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
    await openWorkspace(user, /分组策略/i);
    const multiplier = screen.getByLabelText(/计费倍率 1/i);

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

    await openWorkspace(user, /分组策略/i);
    expect(screen.getByRole("button", { name: /添加分组/i })).toBeDisabled();
    expect(screen.getByLabelText(/分组 ID 1/i)).toBeDisabled();
    expect(screen.getByLabelText(/分组名称 1/i)).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: /Account 1/i })).toBeDisabled();

    await openWorkspace(user, /高级 JSON/i);
    expect(screen.getByRole("textbox", { name: /路由配置 JSON/i })).toBeDisabled();
  });
});
