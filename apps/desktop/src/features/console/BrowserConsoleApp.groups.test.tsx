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

  it("opens an entitlement group editor from its card and autosaves the change", async () => {
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

    const groupCardToggle = within(screen.getByRole("navigation", { name: "权益分组" })).getByRole("button", { name: /VIP 分组/ });
    await user.click(groupCardToggle);
    expect(groupCardToggle).toHaveAttribute("aria-current", "true");
    const edit = screen.getByRole("button", { name: "编辑分组" });
    await user.click(edit);

    expect(groupCardToggle).toHaveAttribute("aria-current", "true");
    expect(screen.getByRole("dialog", { name: "编辑权益组" })).toBeInTheDocument();
    expect(document.querySelector(".nt-group-admin__detail")).toBeNull();
    expect(screen.getByRole("textbox", { name: /分组 ID/i })).toHaveValue("group-vip");
    expect(screen.getByRole("region", { name: /成员管理/i })).toBeInTheDocument();
    const name = screen.getByRole("textbox", { name: /分组名称/i });
    expect(name).toHaveFocus();
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();
    await user.clear(name);
    await user.type(name, "Updated VIP");
    const draft = await waitForCommittedRouteDraft(consoleApi);
    expect(draft.account_groups).toEqual([
      expect.objectContaining({
        id: "group-vip",
        name: "Updated VIP",
        billing_multiplier: 1.5,
        provider_credential_ids: ["acc-prod-1"],
      }),
      {
        id: "default",
        name: "default",
        billing_multiplier: 1,
        enabled: true,
        provider_credential_ids: ["acc-prod-2"],
      },
    ]);
    expect(screen.getByRole("dialog", { name: "编辑权益组" })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByLabelText("分组名称")).toBeEnabled());
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.getByRole("button", { name: "编辑分组" })).toHaveFocus());
    expect(screen.getByRole("heading", { name: "Updated VIP" })).toBeVisible();
  }, 10_000);

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
    await user.click(screen.getByRole("button", { name: "生产账号 A" }));
    await user.click(screen.getByRole("button", { name: "账号配置" }));
    const accountLibrary = document.querySelector(".nt-credential-account-settings") as HTMLElement;
    const commitCallCount = vi.mocked(consoleApi.commitRouteConfig).mock.calls.length;
    await user.click(within(accountLibrary).getByRole("combobox", { name: /调整 生产账号 A 分组池/i }));
    await user.click(screen.getByRole("option", { name: "白银级别服务" }));

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
    expect(screen.getByRole("dialog", { name: "添加分组" })).toBeInTheDocument();
    await user.type(screen.getByLabelText(/分组 ID/i), "group-team-b");
    await user.type(screen.getByLabelText(/分组名称/i), "Team B");
    await user.clear(screen.getByLabelText(/计费倍率/i));
    await user.type(screen.getByLabelText(/计费倍率/i), "0.8");
    expect(screen.getByRole("dialog", { name: "添加分组" })).toHaveClass("nt-group-edit-dialog");
    await user.type(screen.getByRole("searchbox", { name: /筛选候选账号/i }), "生产账号 B");
    await user.click(screen.getByRole("button", { name: /加入.*生产账号 B/i }));
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: /^创建分组$/i }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(within(screen.getByRole("navigation", { name: "权益分组" })).getByRole("button", { name: /Team B/ }));
    await waitFor(() => expect(screen.getByRole("button", { name: "编辑分组" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "编辑分组" }));
    await user.type(screen.getByRole("searchbox", { name: /筛选候选账号/i }), "生产账号 B");
    expect(screen.getByRole("button", { name: /移除.*生产账号 B/i })).toBeInTheDocument();

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

  it("keeps incomplete group forms out of the route document until confirmed", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();

    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);

    await waitForConsoleReady();
    await openWorkspace(user, /权益组/i);
    await user.click(screen.getByRole("button", { name: /添加分组/i }));
    await user.type(screen.getByLabelText(/分组名称/i), "Incomplete group");
    await user.click(screen.getByRole("button", { name: /^创建分组$/i }));

    expect(screen.getByLabelText(/分组 ID/i)).toHaveAttribute("aria-invalid", "true");
    expect(screen.getAllByText(/分组 ID 必须填写/i).length).toBeGreaterThan(0);

    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 1300)); });
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();
    expect(screen.queryByText("草稿待处理")).not.toBeInTheDocument();
    expect(document.querySelector('[data-entitlement-group-card="completed-group"]')).toBeNull();
    await user.type(screen.getByLabelText(/分组 ID/i), "completed-group");
    await user.click(screen.getByRole("button", { name: /^创建分组$/i }));
    const draft = await waitForCommittedRouteDraft(consoleApi);
    expect(draft.account_groups).toEqual(expect.arrayContaining([
      expect.objectContaining({ id: "completed-group", name: "Incomplete group" }),
    ]));
  }, 10_000);

  it("cancels creation without changing the derived default group or saving", async () => {
    const consoleApi = createConsoleApi();
    const user = userEvent.setup();
    renderWithProviders(<BrowserConsoleApp consoleApi={consoleApi} />);
    await waitForConsoleReady();
    await openWorkspace(user, /权益组/i);
    expect(screen.getByRole("heading", { name: "default" })).toBeVisible();
    const trigger = screen.getByRole("button", { name: /^添加分组$/i });
    await user.click(trigger);
    await user.type(screen.getByLabelText(/分组 ID/i), "cancelled-group");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
    expect(screen.getByRole("heading", { name: "default" })).toBeVisible();
    expect(within(screen.getByRole("navigation", { name: "权益分组" })).queryByRole("button", { name: /cancelled-group/ })).not.toBeInTheDocument();
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 1300)); });
    expect(consoleApi.commitRouteConfig).not.toHaveBeenCalled();
  });
});
