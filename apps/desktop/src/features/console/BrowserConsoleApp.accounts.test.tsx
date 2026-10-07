import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders, waitForConsoleReady, openWorkspace } from "./BrowserConsoleApp.render-fixture";

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
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
    await openWorkspace(user, /凭据池/i);

    await user.click(screen.getByRole("button", { name: /^Managed OpenAI$/i }));

    const providerPanel = screen.getByRole("region", { name: /^Managed OpenAI 账号库$/i });
    const summaryAccount = providerPanel.querySelector('[data-account-card="acc-prod-1"]');
    expect(summaryAccount).not.toBeNull();
    expect(within(summaryAccount as HTMLElement).getByText("生产账号 A")).toBeInTheDocument();
    expect(within(summaryAccount as HTMLElement).getByRole("switch")).toHaveAttribute("aria-checked", "false");
    expect(
      within(summaryAccount as HTMLElement).getByRole("combobox", { name: /调整 生产账号 A 分组池/i }),
    ).toHaveAttribute("data-group-id", "group-vip");
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
    await openWorkspace(user, /凭据池/i);

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
    await openWorkspace(user, /凭据池/i);

    expect(screen.getByRole("button", { name: "Qwen OpenAI" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Qwen Web" })).toBeInTheDocument();
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
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /显示 Managed OpenAI 账号库/i }));
    await user.click(
      within(screen.getByRole("region", { name: /^Managed OpenAI 账号库$/i })).getByRole(
        "button",
        { name: /手动录入账号|Add account manually/i },
      ),
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

    expect(screen.queryByRole("dialog", { name: /新增账号/i })).not.toBeInTheDocument();

    await waitFor(() => expect(consoleApi.commitRouteConfig).toHaveBeenCalled(), { timeout: 3000 });
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
  }, 10_000);

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
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /^Managed OpenAI$/i }));
    await user.click(
      within(screen.getByRole("region", { name: /^Managed OpenAI 账号库$/i })).getByRole("button", {
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

    const editedAccount = screen
      .getByRole("region", { name: /^Managed OpenAI 账号库$/i })
      .querySelector('[data-account-card="acc-prod-1"]');
    expect(editedAccount).not.toBeNull();
    expect(within(editedAccount as HTMLElement).getByText("暂停")).toBeInTheDocument();
    expect(
      within(editedAccount as HTMLElement).getByRole("button", {
        name: /编辑账号 生产账号 A2/i,
      }),
    ).toBeInTheDocument();
    await waitFor(() => expect(consoleApi.commitRouteConfig).toHaveBeenCalled(), { timeout: 3000 });
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

  it("directs missing credential setup to the visible account-add actions", async () => {
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
    await openWorkspace(user, /凭据池/i);

    expect(screen.getByText(/请使用上方的“添加账号”/i)).toBeInTheDocument();
    expect(screen.queryByText(/高级 JSON/i)).not.toBeInTheDocument();

    await openWorkspace(user, /权益组/i);
    expect(screen.getByText(/当前还没有任何凭证分组/i)).toBeInTheDocument();
  });
});
