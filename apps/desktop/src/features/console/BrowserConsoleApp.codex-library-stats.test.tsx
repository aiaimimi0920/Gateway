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
import { renderWithProviders, waitForConsoleReady, openWorkspace, waitForCommittedRouteDraft } from "./BrowserConsoleApp.render-fixture";

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("shows polled provider aggregates and credential health without a separate usage request", async () => {
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
    consoleApi.getCostOverview = vi.fn<NonNullable<typeof consoleApi.getCostOverview>>().mockResolvedValue({
      overview: {
        pricingEditors: [],
        providerBuckets: [{
          providerAccountId: "codex",
          label: "codex",
          adapter: "openai_compatible",
          protocolFamily: "openai",
          requestCount: 32,
          promptTokens: 400,
          completionTokens: 600,
          totalTokens: 1000,
          estimatedMarketCostMicros: null,
          pricedModelCount: 0,
          unpricedModelCount: 1,
          lastRequestAt: "2026-08-19T11:00:00Z",
          models: [],
        }],
      },
    });
    consoleApi.getRequestAuditSummary = vi.fn<NonNullable<typeof consoleApi.getRequestAuditSummary>>().mockResolvedValue({
      summary: {
        totalRequests: 40,
        completedCount: 28,
        failedCount: 4,
        cancelledCount: 5,
        runningCount: 3,
        providerAccounts: [{
          providerAccountId: "codex",
          totalRequests: 40,
          completedCount: 28,
          failedCount: 4,
          cancelledCount: 5,
          runningCount: 3,
          lastRequestAt: "2026-08-19T11:00:00Z",
          windows: [],
          models: [],
        }],
      },
    });
    consoleApi.listProviderCredentialModelStates = vi.fn<NonNullable<typeof consoleApi.listProviderCredentialModelStates>>().mockResolvedValue({
      states: [{
        id: "health-row",
        providerAccountId: "codex",
        providerCredentialId: null,
        providerCredentialRef: "codex-free-1",
        protocolProfile: null,
        model: "gpt-5.4",
        status: "active",
        failureClass: null,
        failureScope: null,
        failureCount: 2,
        lastError: null,
        lastUpstreamStatus: 200,
        cooldownUntil: null,
        lastSuccessAt: "2026-08-19T11:00:00Z",
        lastFailureAt: null,
        updatedAt: "2026-08-19T11:00:00Z",
      }],
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /显示 codex 账号库/i }));
    const accountLibrary = screen.getByRole("region", { name: /^codex 账号库$/i });
    await user.click(
      within(accountLibrary).getByRole("button", { name: /查看 Codex Free 1 统计/i }),
    );

    const statsDialog = screen.getByRole("dialog", { name: /查看账号统计|View account stats/i });
    expect(within(statsDialog).getByText("Codex Free 1")).toBeInTheDocument();
    expect(consoleApi.getCostOverview).toHaveBeenCalledWith("management-secret");
    expect(consoleApi.getRequestAuditSummary).toHaveBeenCalledWith("management-secret");
    expect(consoleApi.listProviderCredentialModelStates).toHaveBeenCalledWith("management-secret");
    expect(consoleApi.getCredentialUsage).not.toHaveBeenCalled();
    expect(await within(statsDialog).findByText("32")).toBeInTheDocument();
    expect(within(statsDialog).getByText("28")).toBeInTheDocument();
    expect(within(statsDialog).getByText("87.5%")).toBeInTheDocument();
    expect(within(statsDialog).getByText("4")).toBeInTheDocument();
    expect(within(statsDialog).getByText("1,000")).toBeInTheDocument();
    expect(within(statsDialog).getByRole("heading", { name: /服务商账号聚合/i })).toBeInTheDocument();
    expect(within(statsDialog).getByTitle("gpt-5.4")).toHaveTextContent("1");
    expect(within(statsDialog).getByText("正常")).toBeInTheDocument();
    expect(within(statsDialog).getByText("累计失败").parentElement).toHaveTextContent("2");
    expect(within(statsDialog).getByRole("heading", { name: /定时测试/i })).toBeInTheDocument();
  });

  it("updates a real scheduled-test draft and duplicates an account from the account menu", async () => {
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

    await preserveCommittedRouteDocument(consoleApi);
    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />, {
      session: { secretAccessGranted: true },
      secretGrant: { grant: "duplicate-account-grant", expiresAt: "2099-01-01T00:00:00Z" },
    });

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /显示 codex 账号库/i }));
    let accountLibrary = screen.getByRole("region", { name: /^codex 账号库$/i });
    await user.click(
      within(accountLibrary).getByRole("button", { name: /查看 Codex Free 1 统计/i }),
    );

    const statsDialog = screen.getByRole("dialog", { name: /查看账号统计|View account stats/i });
    await user.click(
      within(statsDialog).getByRole("checkbox", { name: /启用连接测试计划/i }),
    );
    const intervalInput = within(statsDialog).getByRole("spinbutton", {
      name: /执行间隔（分钟）/i,
    });
    await user.clear(intervalInput);
    await user.type(intervalInput, "15");
    await user.click(within(statsDialog).getByRole("button", { name: /更新计划/i }));
    await user.click(within(statsDialog).getByText(/^关闭$/i));

    const scheduledDraft = JSON.stringify(await waitForCommittedRouteDraft(consoleApi));
    expect(scheduledDraft).toContain('"scheduled_probe_enabled":true');
    expect(scheduledDraft).toContain('"scheduled_probe_interval_minutes":15');

    accountLibrary = screen.getByRole("region", { name: /^codex 账号库$/i });
    const moreButton = within(accountLibrary).getByRole("button", {
      name: /更多操作 codex-free-1|More actions codex-free-1/i,
    });

    const savedCount = vi.mocked(consoleApi.commitRouteConfig).mock.calls.length;
    await user.click(moreButton);
    await user.click(screen.getByRole("menuitem", { name: /复制账号|Duplicate account/i }));

    const duplicateDialog = screen.getByRole("dialog", { name: /新增账号|Add account/i });
    expect(within(duplicateDialog).getByLabelText(/账号 ID/i)).toHaveValue("codex-free-1-copy");
    expect(within(duplicateDialog).getByLabelText(/账号名称/i)).toHaveValue("Codex Free 1 Copy");
    await user.type(within(duplicateDialog).getByLabelText(/^API Key$/i), "copy-secret");
    await user.click(within(duplicateDialog).getByRole("button", { name: /保存/i }));

    const duplicateDocument = await waitForCommittedRouteDraft(consoleApi, savedCount);
    const duplicateDraft = JSON.stringify(duplicateDocument);
    expect(duplicateDraft).toContain('"id":"codex-free-1-copy"');
    expect(duplicateDraft).toContain('"account_name":"Codex Free 1 Copy"');
    expect(duplicateDocument.providers).toEqual(expect.arrayContaining([
      expect.objectContaining({ credentials: expect.arrayContaining([
        expect.objectContaining({
          id: "codex-free-1",
          scheduled_probe_enabled: true,
          scheduled_probe_interval_minutes: 15,
        }),
      ]) }),
    ]));
  }, 10_000);
});
