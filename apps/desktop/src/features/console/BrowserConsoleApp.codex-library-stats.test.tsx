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
    await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /^codex/ }));
    const providerStats = screen.getByRole("region", { name: "codex 统计" });
    expect(within(providerStats).getByText("请求数").parentElement).toHaveTextContent("32");
    expect(consoleApi.getCostOverview).toHaveBeenCalledWith("management-secret");
    expect(consoleApi.getRequestAuditSummary).toHaveBeenCalledWith("management-secret");
    expect(consoleApi.listProviderCredentialModelStates).toHaveBeenCalledWith("management-secret");
    await user.click(screen.getByRole("button", { name: "Codex Free 1" }));
    const accountStats = screen.getByRole("region", { name: "Codex Free 1 统计" });
    // A sole credential can own the provider aggregate; multi-account isolation is covered by ledger tests.
    expect(within(accountStats).getByText("请求数").parentElement).toHaveTextContent("32");
    expect(screen.queryByRole("region", { name: "codex 统计" })).not.toBeInTheDocument();
    expect(consoleApi.getCredentialUsage).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "账号配置" }));
    expect(screen.getByRole("article", { name: "Codex Free 1 账号" })).toBeVisible();
  });

  it("edits the selected account without losing its identity or leaving the account detail scope", async () => {
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
    await user.click(screen.getByRole("button", { name: "Codex Free 1" }));
    await user.click(screen.getByRole("button", { name: "编辑账号" }));
    const dialog = screen.getByRole("dialog", { name: /编辑账号/ });
    await user.type(within(dialog).getByLabelText(/^API Key$/i), "updated-secret");
    await user.click(within(dialog).getByRole("button", { name: "保存" }));
    const saved = await waitForCommittedRouteDraft(consoleApi);
    expect(saved.providers[0]).toMatchObject({ id: "codex", credentials: [
      { id: "codex-free-1", account_name: "Codex Free 1", enabled: true, credential_identity_category_id: "free" },
    ] });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByRole("region", { name: "Codex Free 1 统计" })).toBeVisible();
  }, 10_000);
});
