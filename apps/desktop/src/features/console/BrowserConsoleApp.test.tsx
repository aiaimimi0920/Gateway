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
import { renderWithProviders, waitForConsoleReady, openWorkspace, providerCard } from "./BrowserConsoleApp.render-fixture";

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("shows configured LongCat accounts and unavailable telemetry without demo metrics", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    vi.mocked(consoleApi.getRouteConfig).mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-longcat-preview", sequence: 1, message: "LongCat preview" },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: {
          providers: [
            {
              id: "longcat",
              label: "LongCat",
              vendor_name: "LongCat",
              preset: "longcat-openai",
              base_url: "https://api.longcat.chat/openai",
              supported_models: ["LongCat-2.0"],
              credentials: [
                {
                  id: "longcat-live",
                  account_name: "LongCat Current",
                  api_key: "fake-longcat-key",
                },
              ],
            },
          ],
          model_routes: [{ pattern: "LongCat-*", provider_ids: ["longcat"] }],
          aliases: { longcat: "LongCat-2.0" },
        },
        secrets: [{ path: "/providers/0/credentials/0/api_key", configured: true, preview: "ak-***" }],
        mutationSupported: true,
      },
    });

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);

    const card = providerCard(/^LongCat$/i);
    for (const metric of ["concurrency", "upstream-cost", "platform-revenue", "requests"]) {
      expect(card.querySelector(`[data-provider-metric="${metric}"]`)).toHaveTextContent("—");
    }
    expect(
      within(card).queryByRole("img", {
        name: /最近窗口的调用成功率/,
      }),
    ).not.toBeInTheDocument();

    await user.click(within(card).getByRole("button", { name: /^LongCat$/i }));
    const library = screen.getByRole("region", { name: /^LongCat 账号库$/i });
    expect(library.querySelectorAll("[data-account-card]")).toHaveLength(1);
    expect(library.querySelector('[data-account-card="longcat-live"]')).not.toBeNull();
    expect(library.querySelector('[data-account-card*="demo"]')).toBeNull();
    expect(within(library).queryByRole("navigation", { name: /账号库分页/i })).not.toBeInTheDocument();
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
    await openWorkspace(user, /凭据池/i);
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
    await waitFor(() => expect(consoleApi.commitRouteConfig).toHaveBeenCalled(), { timeout: 3000 });

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
    await openWorkspace(user, /凭据池/i);

    expect(screen.getByRole("button", { name: /^Gemini$/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Gemini Business$/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Gemini Canvas$/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Gemini Web \/u\/1\/$/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^gemini-canvas-chat$/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Gemini$/ }));
    const geminiPanel = screen.getByRole("region", { name: /^Gemini 账号库$/ });
    expect(within(geminiPanel).getByText("Gemini Web Account")).toBeInTheDocument();
    expect(within(geminiPanel).getByText("Gemini Chat Account")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Gemini Canvas$/ }));
    const canvasPanel = screen.getByRole("region", { name: /^Gemini Canvas 账号库$/ });
    expect(within(canvasPanel).getByText("Canvas Account")).toBeInTheDocument();
  });
});
