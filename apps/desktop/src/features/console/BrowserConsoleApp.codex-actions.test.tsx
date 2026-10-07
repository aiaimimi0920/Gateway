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
import {
  renderWithProviders,
  waitForConsoleReady,
  openWorkspace,
  waitForCommittedRouteDraft,
  providerCard,
} from "./BrowserConsoleApp.render-fixture";

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

    expect(within(freeCard as HTMLElement).getByText("暂停")).toBeInTheDocument();

    const draft = JSON.stringify(await waitForCommittedRouteDraft(consoleApi));
    expect(draft).toContain('"id":"codex-free-1"');
    expect(draft).toContain('"enabled":false');
  });

  it("opens the codex more menu and runs a real account single-point test", async () => {
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
    const moreButton = within(accountLibrary).getByRole("button", {
      name: /更多操作 codex-free-1|More actions codex-free-1/i,
    });
    await user.click(moreButton);

    expect(screen.getByRole("menuitem", { name: /账号测试|Test account/i })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /复制账号|Duplicate account/i })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: /查看统计|View stats/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: /定时测试|Scheduled tests/i })).not.toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menuitem", { name: /账号测试|Test account/i })).not.toBeInTheDocument();
    expect(moreButton).toHaveFocus();

    await user.click(moreButton);
    await user.click(within(accountLibrary).getByText("Codex Free 1"));
    expect(screen.queryByRole("menuitem", { name: /账号测试|Test account/i })).not.toBeInTheDocument();

    await user.click(moreButton);
    await user.click(screen.getByRole("menuitem", { name: /账号测试|Test account/i }));

    const probeDialog = screen.getByRole("dialog", { name: /账号单点测试|Account single-point test/i });
    expect(within(probeDialog).getByText("Codex Free 1")).toBeInTheDocument();
    expect(within(probeDialog).queryByLabelText(/选择测试模型|Select test model/i)).not.toBeInTheDocument();
    await user.click(within(probeDialog).getByRole("button", { name: /开始测试|Start test/i }));

    await waitFor(() =>
      expect(consoleApi.probeCredential).toHaveBeenCalledWith(
        "management-secret",
        "grant-1",
        "codex-free-1",
      ),
    );
    expect(await within(probeDialog).findByText(/Credential connectivity probe passed\./i)).toBeInTheDocument();
    expect(
      within(probeDialog).getByText("GET https://managed.example/v1/models"),
    ).toBeInTheDocument();
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

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    const provider = providerCard(/^codex$/i);
    const moreButton = within(provider).getByRole("button", {
      name: /codex 更多操作|codex more actions/i,
    });

    await user.click(moreButton);
    await user.click(screen.getByRole("menuitem", { name: /服务商测试|Test provider/i }));
    const providerProbeDialog = screen.getByRole("dialog", {
      name: /服务商测试|Provider test/i,
    });
    await user.click(
      within(providerProbeDialog).getByRole("button", {
        name: /测试全部账号|Test all accounts/i,
      }),
    );
    await waitFor(() =>
      expect(consoleApi.probeProvider).toHaveBeenCalledWith(
        "management-secret",
        "grant-1",
        "codex",
      ),
    );
    const passedCard = within(providerProbeDialog)
      .getByText("通过", { selector: "span" })
      .closest("article");
    expect(passedCard).not.toBeNull();
    expect(within(passedCard as HTMLElement).getByText("1")).toBeInTheDocument();
    expect(
      within(providerProbeDialog).getByText("GET https://managed.example/v1/models"),
    ).toBeInTheDocument();
    await user.click(within(providerProbeDialog).getByText("关闭", { selector: "button" }));

    await user.click(moreButton);
    await user.click(
      screen.getByRole("menuitem", {
        name: /自动定时测试|Automatic scheduled tests/i,
      }),
    );
    const scheduleDialog = screen.getByRole("dialog", {
      name: /自动定时测试|Automatic scheduled tests/i,
    });
    await user.click(
      within(scheduleDialog).getByRole("checkbox", {
        name: /启用全部账号的自动单点测试|Enable automatic tests for all accounts/i,
      }),
    );
    const interval = within(scheduleDialog).getByRole("spinbutton", {
      name: /执行间隔|Interval/i,
    });
    await user.clear(interval);
    await user.type(interval, "15");
    await user.click(
      within(scheduleDialog).getByRole("button", {
        name: /更新自动测试计划|Update automatic test schedule/i,
      }),
    );

    const draft = (await waitForCommittedRouteDraft(consoleApi)) as {
      providers: Array<{ credentials: Array<Record<string, unknown>> }>;
    };
    expect(draft.providers[0]?.credentials).toEqual([
      expect.objectContaining({
        id: "codex-free-1",
        scheduled_probe_enabled: true,
        scheduled_probe_interval_minutes: 15,
      }),
      expect.objectContaining({
        id: "codex-plus-1",
        scheduled_probe_enabled: true,
        scheduled_probe_interval_minutes: 15,
      }),
    ]);
  });
});
