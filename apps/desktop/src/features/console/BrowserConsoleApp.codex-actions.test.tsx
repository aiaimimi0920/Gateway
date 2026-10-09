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
import { createConsoleApi, preserveCommittedRouteDocument } from "./BrowserConsoleApp.api-fixture";
import {
  renderWithProviders,
  waitForConsoleReady,
  openWorkspace,
  waitForCommittedRouteDraft,
  providerCard,
} from "./BrowserConsoleApp.render-fixture";

async function useCodexProbeResult(api: ReturnType<typeof createConsoleApi>) {
  const response = await createConsoleApi().probeProvider("fixture", "fixture", "codex");
  response.result.providerId = "codex";
  response.result.results[0].providerId = "codex";
  response.result.results[0].credentialId = "codex-free-1";
  vi.mocked(api.probeProvider).mockResolvedValue(response);
}

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
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
    await openWorkspace(user, /凭据池/i);

    expect(screen.getByRole("button", { name: /^codex$/i })).toBeInTheDocument();
    expect(screen.queryByText("Free")).not.toBeInTheDocument();
    expect(screen.queryByText("Plus")).not.toBeInTheDocument();
    expect(screen.queryByRole("table", { name: /账号台账表/i })).not.toBeInTheDocument();
  });

  it("expands the codex account library and shows routing metadata on account cards", async () => {
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
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));

    const providerRow = screen.getByRole("button", { name: /^codex$/i }).closest(".nt-provider-tree-item");
    expect(providerRow).not.toBeNull();
    expect(within(providerRow as HTMLElement).queryByText("32 req")).not.toBeInTheDocument();

    const accountLibrary = screen.getByRole("region", { name: /^codex 账号库$/i });
    const freeLibrary = within(accountLibrary).getByRole("tabpanel");
    const freeCard = freeLibrary.querySelector('[data-account-card="codex-free-1"]');
    expect(freeCard).not.toBeNull();
    expect(within(freeCard as HTMLElement).getByText("Codex Free 1")).toBeInTheDocument();
    expect(
      within(freeCard as HTMLElement).getByRole("combobox", { name: /调整 Codex Free 1 分组池/i }),
    ).toHaveAttribute("data-group-id", "vip-users");
    expect(within(freeCard as HTMLElement).queryByText("容量")).not.toBeInTheDocument();
    expect(within(freeCard as HTMLElement).queryByText("2 分钟前")).not.toBeInTheDocument();
    expect(
      within(freeCard as HTMLElement).getByRole("switch", { name: /调度 codex-free-1|Dispatch codex-free-1/i }),
    ).toHaveAttribute("aria-checked", "true");
    expect(within(accountLibrary).queryByRole("table")).not.toBeInTheDocument();
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
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));

    const providerRow = screen.getByRole("button", { name: /^codex$/i }).closest(".nt-provider-tree-item");
    expect(providerRow).not.toBeNull();
    const accountLibrary = screen.getByRole("region", { name: /^codex 账号库$/i });
    const freeCard = accountLibrary.querySelector('[data-account-card="codex-free-1"]');
    expect(freeCard).not.toBeNull();
    const dispatchSwitch = within(freeCard as HTMLElement).getByRole("switch", {
      name: /调度 codex-free-1|Dispatch codex-free-1/i,
    });

    expect(dispatchSwitch).toHaveAttribute("aria-checked", "true");
    await user.click(dispatchSwitch);
    expect(dispatchSwitch).toHaveAttribute("aria-checked", "false");

    expect(within(freeCard as HTMLElement).queryByText("暂停")).not.toBeInTheDocument();

    const draft = JSON.stringify(await waitForCommittedRouteDraft(consoleApi));
    expect(draft).toContain('"id":"codex-free-1"');
    expect(draft).toContain('"enabled":false');
  });

  it("opens the direct codex test action and runs an account single-point test", async () => {
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
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /显示 codex 账号库/i }));

    const accountLibrary = screen.getByRole("region", { name: /^codex 账号库$/i });
    const moreButton = within(accountLibrary).getByRole("button", { name: /测试账号 Codex Free 1/i });
    expect(within(accountLibrary).queryByRole("button", { name: /更多操作/ })).not.toBeInTheDocument();
    await user.click(moreButton);

    const probeDialog = screen.getByRole("dialog", { name: /测试 · codex|Test · codex/i });
    expect(within(probeDialog).getByLabelText(/固定账户|Fixed account/i)).toHaveTextContent("Codex Free 1");
    expect(within(probeDialog).queryByRole("radio")).not.toBeInTheDocument();
    expect(within(probeDialog).queryByLabelText(/选择账户|Select account/i)).not.toBeInTheDocument();
    await useCodexProbeResult(consoleApi);
    await user.click(within(probeDialog).getByRole("button", { name: /运行测试|Run tests/i }));

    await waitFor(() =>
      expect(consoleApi.probeProvider).toHaveBeenCalledWith(
        "management-secret",
        "grant-1",
        "codex",
        expect.objectContaining({ scope: { kind: "account", id: "codex-free-1" }, credentialIds: ["codex-free-1"] }),
        expect.objectContaining({ signal: expect.any(AbortSignal) }),
      ),
    );
    expect(consoleApi.probeCredential).not.toHaveBeenCalled();
    expect(within(probeDialog).queryByText("GET https://managed.example/v1/models")).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(moreButton).toHaveFocus());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("runs a provider-wide test and writes its automatic schedule into the route draft", async () => {
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
              adapter: "openai_compatible",
              base_url: "https://api.example.test/v1",
              credentials: [
                {
                  id: "codex-free-1",
                  account_name: "Codex Free 1",
                  enabled: true,
                  credential_identity_category_id: "free",
                },
                {
                  id: "codex-plus-1",
                  account_name: "Codex Plus 1",
                  enabled: true,
                  credential_identity_category_id: "plus",
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

    await preserveCommittedRouteDocument(consoleApi);
    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    const provider = providerCard(/^codex$/i);
    const moreButton = within(provider).getByRole("button", {
      name: /^测试$|^Test$/i,
    });

    await user.click(moreButton);

    const providerProbeDialog = screen.getByRole("dialog", { name: /^codex$/i });
    expect(within(providerProbeDialog).queryByRole("tab", { name: /自动测试|手动测试/ })).not.toBeInTheDocument();
    await user.click(within(providerProbeDialog).getByRole("button", { name: "添加" }));
    await user.type(within(providerProbeDialog).getByRole("textbox", { name: "计划名称" }), "全池连接测试");
    await user.click(within(providerProbeDialog).getByRole("checkbox", { name: "启用自动测试" }));
    const interval = within(providerProbeDialog).getByRole("spinbutton", { name: /执行间隔/ });
    await user.clear(interval); await user.type(interval, "15");
    expect(within(providerProbeDialog).queryByRole("button", { name: "运行测试" })).not.toBeInTheDocument();
    await user.click(within(providerProbeDialog).getByRole("button", { name: "保存计划" }));
    const draft = await waitForCommittedRouteDraft(consoleApi) as { providers: Array<{
      test_plans: import("../../api/contracts").CredentialTestPlan[]; credentials: Array<Record<string, unknown>>;
    }> };
    const saved = draft.providers[0].test_plans[0];
    expect(saved).toMatchObject({ name: "全池连接测试", scopes: [{ kind: "pool" }],
      policy: { automaticEnabled: true, intervalMinutes: 15, modelSelection: "all" } });
    expect(draft.providers[0]).not.toHaveProperty("test_policy");
    await useCodexProbeResult(consoleApi);
    const run = within(providerProbeDialog).getByRole("button", { name: "手动测试" });
    await waitFor(() => expect(run).toBeEnabled());
    await user.click(run);
    await waitFor(() => expect(consoleApi.probeProvider).toHaveBeenCalledWith(
      "management-secret", "grant-1", "codex",
      { planId: saved.id, credentialIds: ["codex-free-1", "codex-plus-1"] },
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    ));
    expect(within(providerProbeDialog).getByRole("button", { name: "展开测试结果 · 全池连接测试" })).toBeInTheDocument();
    expect(draft.providers[0]?.credentials).toEqual([
      expect.objectContaining({
        id: "codex-free-1",
      }),
      expect.objectContaining({
        id: "codex-plus-1",
      }),
    ]);
    expect(draft.providers[0]?.credentials.every((account) => account.test_policy === undefined && account.scheduled_probe_enabled === undefined)).toBe(true);
  });
});
