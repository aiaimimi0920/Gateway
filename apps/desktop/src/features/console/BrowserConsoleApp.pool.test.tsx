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
  flipProviderCard,
} from "./BrowserConsoleApp.render-fixture";

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("edits provider-level pool policy when a provider has no identity subcategories", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getCredentialPoolAutomation).mockResolvedValue({
      automation: {
        enabled: true,
        intervalSeconds: 60,
        drivers: [{ id: "managed-refill", mode: "http", providerIds: ["managed-provider"] }],
        revisionId: "r1-deadbeefcafe",
        providers: [{
          providerId: "managed-provider",
          providerLabel: "Managed OpenAI",
          targetSize: 30,
          credentialCount: 1,
          activeCredentialCount: 1,
          autoRefillEnabled: false,
          autoPruneEnabled: false,
          permanentDeleteEnabled: false,
          driverId: "managed-refill",
          driverMode: "http",
          driverConfigured: true,
          state: "idle",
          lastRunAt: null,
          nextRunAt: null,
          lastAction: null,
          createdCount: 0,
          prunedCount: 0,
          message: null,
          revisionId: "r1-deadbeefcafe",
        }],
      },
    });

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
    await openWorkspace(user, /凭据池/i);

    const providerRow = await flipProviderCard(user, /^Managed OpenAI$/i);
    expect(within(providerRow).getByText(/^可用池$/)).toBeInTheDocument();
    expect(within(providerRow).getByText(/^补号$/)).toBeInTheDocument();
    expect(within(providerRow).getByText(/^删除（\d+）$/)).toBeInTheDocument();

    await user.click(within(providerRow).getByRole("button", { name: /编辑 Managed OpenAI 最大可用池/ }));
    const targetInput = within(providerRow).getByRole("spinbutton", { name: /Managed OpenAI 最大可用池/ });
    const autoRefillSwitch = within(providerRow).getByRole("switch", {
      name: /Managed OpenAI 自动补号|Managed OpenAI auto refill/i,
    });
    const autoPruneSwitch = within(providerRow).getByRole("switch", {
      name: /Managed OpenAI 自动删除失效号|Managed OpenAI auto delete invalid credentials/i,
    });

    expect(targetInput).toHaveValue(100);
    expect(autoRefillSwitch).toHaveAttribute("aria-checked", "false");
    expect(autoPruneSwitch).toHaveAttribute("aria-checked", "false");

    await user.clear(targetInput);
    await user.type(targetInput, "80");
    await user.click(within(providerRow).getByRole("button", { name: /保存 Managed OpenAI 最大可用池/ }));
    await user.click(autoRefillSwitch);
    await user.click(autoPruneSwitch);

    expect(within(providerRow).getByText("80")).toBeInTheDocument();
    expect(autoRefillSwitch).toHaveAttribute("aria-checked", "true");
    expect(autoPruneSwitch).toHaveAttribute("aria-checked", "true");

    const draft = JSON.stringify(await waitForCommittedRouteDraft(consoleApi));
    expect(draft).toContain('"pool_target_size":80');
    expect(draft).toContain('"auto_refill_enabled":true');
    expect(draft).toContain('"auto_prune_enabled":true');
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
            permanentDeleteEnabled: false,
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
            notificationApi: "/v1/internal/gateway/credential-pool-refill/providers/managed-provider/tasks/claim",
            inquiryApi: "/v1/internal/gateway/credential-pool-refill/providers/managed-provider",
            credentialStoragePath: "C:\\gateway\\credentials\\managed-provider",
            storagePasswordConfigured: false,
            archiveStoragePath: "C:\\gateway\\credentials\\_archive\\managed-provider",
            archivedCredentialCount: 0,
            permanentDeleteEnabled: false,
            revisionId: "r1-deadbeefcafe",
          },
        ],
        recentTasks: [],
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    await flipProviderCard(user, /^Managed OpenAI$/i);
    const refillSwitch = screen.getByRole("switch", {
      name: /Managed OpenAI 自动补号|Managed OpenAI auto refill/i,
    });
    const pruneSwitch = screen.getByRole("switch", {
      name: /Managed OpenAI 自动删除失效号|Managed OpenAI auto delete invalid credentials/i,
    });
    expect(
      screen.queryByText(
        "/v1/internal/gateway/credential-pool-refill/providers/managed-provider/tasks/claim",
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Managed OpenAI 手动补号/ })).toBeEnabled();
    expect(screen.getByRole("button", { name: /Managed OpenAI 手动删除失效号/ })).toBeDisabled();

    await user.click(refillSwitch);
    await user.click(pruneSwitch);

    expect(refillSwitch).toHaveAttribute("aria-checked", "true");
    expect(pruneSwitch).toHaveAttribute("aria-checked", "false");
    expect(
      screen.getByRole("status", { name: /尚未配置受信任的自动剔号驱动器/i }),
    ).toBeInTheDocument();

    const draft = JSON.stringify(await waitForCommittedRouteDraft(consoleApi));
    expect(draft).toContain('"auto_refill_enabled":true');
    expect(draft).not.toContain('"auto_prune_enabled":true');
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
            notificationApi: "/v1/internal/gateway/credential-pool-refill/providers/managed-provider/tasks/claim",
            inquiryApi: "/v1/internal/gateway/credential-pool-refill/providers/managed-provider",
            credentialStoragePath: "C:\\gateway\\credentials\\managed-provider",
            storagePasswordConfigured: false,
            archiveStoragePath: "C:\\gateway\\credentials\\_archive\\managed-provider",
            archivedCredentialCount: 0,
            permanentDeleteEnabled: false,
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
    await openWorkspace(user, /凭据池/i);
    await flipProviderCard(user, /^Managed OpenAI$/i);
    const requestButton = screen.getByRole("button", { name: /Managed OpenAI 手动补号|Manually refill Managed OpenAI/i });
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

    await user.click(screen.getByRole("button", { name: /Managed OpenAI 手动补号|Manually refill Managed OpenAI/i }));
    await waitFor(() => expect(consoleApi.requestCredentialRefill).toHaveBeenCalledTimes(2));
    expect(
      screen.getByRole("status", { name: /已有未完成的补号任务|already has an outstanding refill task/i }),
    ).toBeInTheDocument();
  });

  it("runs configured invalid credential cleanup through the management API", async () => {
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
      permanentDeleteEnabled: false,
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
    vi.mocked(consoleApi.pruneCredentialPool).mockResolvedValue({
      provider: {
        ...automationProvider,
        state: "succeeded",
        lastRunAt: "2026-08-14T08:00:00Z",
        lastAction: "prune",
        prunedCount: 1,
        message: "Invalid credential archived.",
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    await flipProviderCard(user, /^Managed OpenAI$/i);
    await user.click(screen.getByRole("button", { name: /Managed OpenAI 手动删除失效号|Manually delete invalid Managed OpenAI credentials/i }));
    await user.click(screen.getByRole("button", { name: /归档失效号|Archive invalid/i }));

    await waitFor(() =>
      expect(consoleApi.pruneCredentialPool).toHaveBeenCalledWith(
        "management-secret",
        "managed-provider",
      ),
    );
    expect(screen.getByRole("status", { name: /Invalid credential archived/i })).toBeInTheDocument();
  });
});
