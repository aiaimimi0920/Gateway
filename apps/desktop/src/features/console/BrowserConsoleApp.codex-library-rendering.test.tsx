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

  it("shows an empty Codex library when the provider has no credential accounts", async () => {
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
    await user.click(screen.getByRole("button", { name: /^codex$/i }));
    const accountLibrary = screen.getByRole("region", { name: /^codex 账号库$/i });
    expect(within(accountLibrary).getByText("当前账号库为空")).toBeInTheDocument();
    expect(accountLibrary.querySelectorAll("[data-account-card]")).toHaveLength(0);
    expect(within(accountLibrary).queryByRole("switch")).not.toBeInTheDocument();
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();
  });

  it("renders codex account libraries as cards without the legacy detail table", async () => {
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
              credentials: [
                { id: "codex-card-a", account_name: "Card A", credential_identity_category_id: "free" },
                { id: "codex-card-b", account_name: "Card B", credential_identity_category_id: "plus" },
                { id: "codex-card-c", account_name: "Card C", credential_identity_category_id: "plus" },
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
    const accountLibrary = screen.getByRole("region", { name: /^codex 账号库$/i });
      expect(accountLibrary.querySelectorAll("[data-account-card]")).toHaveLength(3);
    expect(within(accountLibrary).queryByRole("table")).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: /codex 账号明细/i })).not.toBeInTheDocument();
  });

  it("does not expose legacy codex identity-category pool rows below the provider card", async () => {
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
    await user.click(screen.getByRole("button", { name: /^codex$/i }));

    expect(screen.getByRole("region", { name: /^codex 账号库$/i })).toBeInTheDocument();
    expect(screen.queryByRole("spinbutton", { name: /Plus 目标号池容量/i })).not.toBeInTheDocument();
    expect(document.querySelector(".nt-provider-subtab-row")).toBeNull();
  });

  it("does not expose the legacy add-account-category action", async () => {
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
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /^codex$/i }));
    expect(screen.queryByRole("button", { name: /添加账号类别/i })).not.toBeInTheDocument();
    expect(prompt).not.toHaveBeenCalled();
  });

  it("renders only configured Codex credentials without synthetic demo accounts", async () => {
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
              credentials: [{
                id: "codex-configured-1",
                account_name: "Configured Codex account",
                enabled: true,
                credential_identity_category_id: "free",
              }],
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
    const accountLibrary = screen.getByRole("region", { name: /^codex 账号库$/i });
    expect(accountLibrary.querySelectorAll("[data-account-card]")).toHaveLength(1);
    const card = accountLibrary.querySelector('[data-account-card="codex-configured-1"]');
    expect(card).not.toBeNull();
    expect(within(card as HTMLElement).getByText("Configured Codex account")).toBeInTheDocument();
    expect(accountLibrary.querySelector('[data-account-card*="demo"]')).toBeNull();
    expect(within(card as HTMLElement).getByRole("switch", {
      name: /调度 codex-configured-1|Dispatch codex-configured-1/i,
    })).toBeEnabled();
    expect(within(accountLibrary).queryByRole("table")).not.toBeInTheDocument();
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
    await openWorkspace(user, /凭据池/i);

    expect(screen.getByRole("button", { name: /^Managed OpenAI$/i })).toBeInTheDocument();
    expect(screen.queryByText("acc-prod-1")).not.toBeInTheDocument();
    expect(screen.queryByText("acc-prod-2")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/凭证分组/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/服务商/i)).not.toBeInTheDocument();
  });
});
