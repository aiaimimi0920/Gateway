import { act, screen, waitFor, within } from "@testing-library/react";
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
} from "./BrowserConsoleApp.render-fixture";

describe("BrowserConsoleApp", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("renders entitlement groups as expandable cards plus a detail editor", async () => {
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
    await preserveCommittedRouteDocument(consoleApi);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /权益组/i);

    expect(screen.getByRole("region", { name: /权益组卡牌/i })).toBeInTheDocument();
    const groupCardToggle = screen.getByRole("button", { name: /^VIP 分组$/i, expanded: false });
    expect(groupCardToggle).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("region", { name: /VIP 分组 组内账号/i })).not.toBeInTheDocument();

    await user.click(groupCardToggle);

    expect(groupCardToggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("region", { name: /VIP 分组 组内账号/i })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /分组详情/i })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: /分组 ID/i })).toHaveValue("group-vip");
    expect(screen.getByRole("region", { name: /成员管理/i })).toBeInTheDocument();
  });

  it("reassigns an account routing pool from the account-card dropdown", async () => {
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
              base_url: "https://api.example.test/v1",
              credentials: [
                { id: "acc-prod-1", account_name: "生产账号 A", enabled: true },
              ],
            },
          ],
          model_routes: [],
          aliases: {},
          account_groups: [
            {
              id: "group-a",
              name: "青铜级别服务",
              billing_multiplier: 1,
              enabled: true,
              provider_credential_ids: ["acc-prod-1"],
            },
            {
              id: "group-b",
              name: "白银级别服务",
              billing_multiplier: 1,
              enabled: true,
              provider_credential_ids: [],
            },
          ],
        },
      },
    });
    await preserveCommittedRouteDocument(consoleApi);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);
    await waitForConsoleReady();
    await openWorkspace(user, /凭据池/i);
    await user.click(screen.getByRole("button", { name: /显示 Managed OpenAI 账号库/i }));
    const accountLibrary = screen.getByRole("region", { name: /^Managed OpenAI 账号库$/i });
    const commitCallCount = vi.mocked(consoleApi.commitRouteConfig).mock.calls.length;
    await user.selectOptions(
      within(accountLibrary).getByRole("combobox", { name: /调整 生产账号 A 分组池/i }),
      "group-b",
    );

    const draft = await waitForCommittedRouteDraft(consoleApi, commitCallCount);
    const groups = draft.account_groups as Array<{
      id: string;
      provider_credential_ids: string[];
    }>;
    expect(
      groups.find((group) => group.id === "group-a")?.provider_credential_ids,
    ).toEqual([]);
    expect(
      groups.find((group) => group.id === "group-b")?.provider_credential_ids,
    ).toEqual(["acc-prod-1"]);
  });

  it("adds a group and manages members from compact candidate rows while keeping the route draft in sync", async () => {
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
    await preserveCommittedRouteDocument(consoleApi);

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /权益组/i);

    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    const groupCards = screen.getByRole("region", { name: /权益组卡牌/i });
    await user.click(within(groupCards).getByRole("button", { name: /^新分组$/i, expanded: false }));
    await user.type(screen.getByLabelText(/分组 ID/i), "group-team-b");
    await user.type(screen.getByLabelText(/分组名称/i), "Team B");
    await user.clear(screen.getByLabelText(/计费倍率/i));
    await user.type(screen.getByLabelText(/计费倍率/i), "0.8");
    await user.type(screen.getByRole("searchbox", { name: /筛选候选账号/i }), "生产账号 B");
    await user.click(screen.getByRole("button", { name: /加入.*生产账号 B/i }));

    // Earlier field edits can commit before the member-edit debounce finishes.
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
      { timeout: 3000 },
    );
  }, 10_000);

  it("marks incomplete account-group drafts and blocks saving until the group ID is filled", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /权益组/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    const groupCards = screen.getByRole("region", { name: /权益组卡牌/i });
    await user.click(within(groupCards).getByRole("button", { name: /^新分组$/i, expanded: false }));
    await user.type(screen.getByLabelText(/分组名称/i), "Incomplete group");

    expect(screen.getByLabelText(/分组 ID/i)).toHaveAttribute("aria-invalid", "true");
    expect(screen.getAllByText(/分组 ID 必须填写/i).length).toBeGreaterThan(0);

    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 1300)); });
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();
    expect(screen.getByText("草稿待处理")).toBeInTheDocument();
    await user.type(screen.getByLabelText(/分组 ID/i), "completed-group");
    const draft = await waitForCommittedRouteDraft(consoleApi);
    expect(draft.account_groups).toEqual(expect.arrayContaining([
      expect.objectContaining({ id: "completed-group", name: "Incomplete group" }),
    ]));
  }, 10_000);
});
