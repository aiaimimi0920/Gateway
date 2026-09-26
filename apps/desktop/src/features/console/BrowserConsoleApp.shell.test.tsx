import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import {
  renderWithProviders, waitForConsoleReady, consoleNavigation,
  workspaceButton, openWorkspace,
} from "./BrowserConsoleApp.render-fixture";

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
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

  it("keeps only the account, group, and model workspaces in the management sidebar", async () => {
    const consoleApi = createConsoleApi();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(workspaceButton(/凭据池/i)).toBeInTheDocument();
    expect(workspaceButton(/权益组/i)).toBeInTheDocument();
    expect(workspaceButton(/模型池/i)).toBeInTheDocument();
    for (const removed of [
      /Provider 资源/i,
      /总览/i,
      /路由编辑/i,
      /敏感信息/i,
      /修订历史/i,
      /高级 JSON/i,
    ]) {
      expect(within(consoleNavigation()).queryByRole("button", { name: removed })).not.toBeInTheDocument();
    }
    expect(screen.queryByText(/原始文档/i)).not.toBeInTheDocument();
  });

  it("puts a settings button under the rail divider and opens the appearance workspace", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    const brand = document.querySelector(".nt-rail .nt-brand__name");
    expect(brand).toHaveTextContent(/^Gateway$/);
    expect(document.querySelector(".nt-rail .nt-brand__copy")).toBeNull();

    const utility = screen.getByRole("navigation", { name: /辅助导航/i });
    expect(utility.closest(".nt-rail__utility")).not.toBeNull();
    const settingsButton = within(utility).getByRole("button", { name: /设置/i });

    await user.click(settingsButton);
    expect(await screen.findByRole("heading", { name: /^主题$/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /界面语言/i })).toBeInTheDocument();
    expect(document.querySelector(".nt-hud-strip")).toBeNull();
  });

  it("collapses and restores the console sidebar without hiding navigation semantics", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    const shell = document.querySelector(".nt-shell--console") as HTMLElement;
    const collapse = screen.getByRole("button", { name: /收起侧栏|Collapse sidebar/i });
    const activeWorkspace = workspaceButton(/凭据池|Credential pool/i);
    expect(collapse).toHaveAttribute("aria-expanded", "true");
    expect(activeWorkspace).toHaveAttribute("aria-current", "page");
    expect(activeWorkspace).not.toHaveAttribute("aria-pressed");
    expect(shell).not.toHaveClass("nt-shell--console-rail-collapsed");

    await user.click(collapse);

    expect(shell).toHaveClass("nt-shell--console-rail-collapsed");
    expect(workspaceButton(/凭据池|Credential pool/i)).toBeInTheDocument();
    const expand = screen.getByRole("button", { name: /展开侧栏|Expand sidebar/i });
    expect(expand).toHaveAttribute("aria-expanded", "false");

    await user.click(expand);
    expect(shell).not.toHaveClass("nt-shell--console-rail-collapsed");
  });

  it("removes the browser-console hero and the accounts ledger overview shell chrome", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    expect(screen.queryByRole("heading", { name: /Gateway 网页控制台/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/直接通过 Gateway 本体托管的浏览器控制台/i)).not.toBeInTheDocument();

    await openWorkspace(user, /凭据池/i);
    expect(screen.queryByRole("heading", { name: /账号台账/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/统一查看 Gateway 当前可路由账号与启用状态/i)).not.toBeInTheDocument();
    expect(screen.queryByText("账号总数")).not.toBeInTheDocument();
    expect(screen.queryByRole("searchbox", { name: /搜索账号/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/^当前显示$/)).not.toBeInTheDocument();
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
    await openWorkspace(user, /凭据池/i);

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

    const providerLibrary = screen.getByRole("region", { name: /^Managed OpenAI 账号库$/i });
    const firstCard = providerLibrary.querySelector('[data-account-card="acc-prod-1"]');
    const secondCard = providerLibrary.querySelector('[data-account-card="acc-prod-2"]');

    expect(firstCard).not.toBeNull();
    expect(secondCard).not.toBeNull();
    expect(within(firstCard as HTMLElement).getByText("生产账号 A")).toBeInTheDocument();
    expect(
      within(firstCard as HTMLElement).getByRole("combobox", { name: /调整 生产账号 A 分组池/i }),
    ).toHaveValue("group-vip");
    expect(within(firstCard as HTMLElement).getByRole("switch", { name: /调度 acc-prod-1/i })).toHaveAttribute("aria-checked", "true");
    expect(
      within(secondCard as HTMLElement).getByRole("combobox", { name: /调整 生产账号 B 分组池/i }),
    ).toHaveValue("");
    expect(within(secondCard as HTMLElement).getByRole("switch", { name: /调度 acc-prod-2/i })).toHaveAttribute("aria-checked", "true");
    expect(within(providerLibrary).queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: /Managed OpenAI 账号明细/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /测试账号 生产账号 A|Test account 生产账号 A/i })).not.toBeInTheDocument();
    expect(within(firstCard as HTMLElement).getByRole("button", { name: /删除账号 生产账号 A|Delete account 生产账号 A/i })).toBeInTheDocument();
  });

});
