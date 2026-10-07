import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import {
  automationProvider,
  card,
  pilotAccount,
  pilotSection,
  refillDemand,
  renderWorkspace,
  workspaceProps,
} from "./AccountsLedgerWorkspace.fixtures";

describe("AccountsLedgerWorkspace account library", () => {
  it("attaches an account library after its provider card without resizing the provider card", async () => {
    const user = userEvent.setup();
    renderWorkspace(
      workspaceProps({
        pilotSections: [
          pilotSection({
            directAccounts: [
              pilotAccount({
                accountId: "acct-1",
                concurrencyUsed: 2,
                concurrencyTotal: 4,
                upstreamCost: 1.25,
                userCost: 2.5,
                successWindows: [{ label: "10:00", success: 3, requests: 4 }],
              }),
              pilotAccount({
                accountId: "acct-2",
                displayName: "Account 2",
                statusLabel: "限流等待恢复",
              }),
            ],
          }),
        ],
      }),
    );

    const providerCard = card("managed-provider");
    const showLibrary = within(providerCard).getByRole("button", {
      name: "显示 Managed OpenAI 账号库",
    });
    const flip = within(providerCard).getByRole("button", {
      name: /翻面查看 Managed OpenAI 详情/,
    });
    expect(showLibrary).toHaveAttribute("aria-expanded", "false");
    expect(
      showLibrary.compareDocumentPosition(flip) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).not.toBe(0);

    await user.click(showLibrary);

    const library = screen.getByRole("region", { name: "Managed OpenAI 账号库" });
    expect(
      within(providerCard).getByRole("button", { name: "收起 Managed OpenAI 账号库" }),
    ).toHaveAttribute("aria-expanded", "true");
    const providerStack = providerCard.closest("[data-provider-card-stack]");
    expect(providerStack).not.toHaveClass("nt-provider-card-stack--expanded");
    expect(providerStack?.nextElementSibling).toBe(library);
    expect(library).toHaveClass("nt-provider-account-library--attached");
    expect(providerCard.contains(library)).toBe(false);
    expect(library.querySelectorAll("[data-account-card]")).toHaveLength(2);
    expect(library.querySelector('[data-account-card="acct-1"]')).toHaveTextContent("Account 1");
    expect(library.querySelector('[data-account-card="acct-1"]')).toHaveTextContent("2/4");
    expect(library.querySelector('[data-account-card="acct-1"]')).toHaveTextContent("75%");

    await user.click(
      within(providerCard).getByRole("button", { name: "收起 Managed OpenAI 账号库" }),
    );
    expect(screen.queryByRole("region", { name: "Managed OpenAI 账号库" })).not.toBeInTheDocument();
  });

  it("separates source-path account libraries from independently assigned routing pools", async () => {
    const user = userEvent.setup();
    renderWorkspace(
      workspaceProps({
        pilotSections: [
          pilotSection({
            directAccounts: [
              pilotAccount({
                accountId: "free-1",
                displayName: "free@example.com",
                libraryName: "free",
                logicalLabels: ["青铜级别服务"],
                logicalGroupIds: ["bronze-service"],
              }),
              pilotAccount({
                accountId: "plus-1",
                displayName: "plus@example.com",
                libraryName: "plus",
                logicalLabels: ["青铜级别服务"],
                logicalGroupIds: ["bronze-service"],
              }),
            ],
          }),
        ],
        groupOptions: [{ value: "bronze-service", label: "青铜级别服务" }],
      }),
    );

    await user.click(
      within(card("managed-provider")).getByRole("button", {
        name: "显示 Managed OpenAI 账号库",
      }),
    );

    const freeLibrary = screen.getByRole("tabpanel");
    expect(within(freeLibrary).getByText("free@example.com")).toBeInTheDocument();
    expect(screen.queryByText("plus@example.com")).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "plus 1" }));
    const plusLibrary = screen.getByRole("tabpanel");
    expect(within(plusLibrary).getByText("plus@example.com")).toBeInTheDocument();
    expect(screen.queryByText("free@example.com")).not.toBeInTheDocument();
    expect(within(plusLibrary).getByRole("combobox", { name: "调整 plus@example.com 分组池" })).toHaveTextContent("青铜级别服务");
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByRole("tab", { name: "free 1" })).toHaveAttribute("aria-selected", "true");
  });

  it("renders all accounts in a two-row scroll viewport without pagination", async () => {
    const user = userEvent.setup();
    const accounts = Array.from({ length: 18 }, (_, index) => pilotAccount({
      accountId: `acct-${index + 1}`, displayName: `account-${index + 1}@example.com`, libraryName: "free",
    }));
    renderWorkspace(workspaceProps({ pilotSections: [pilotSection({ directAccounts: accounts })] }));
    await user.click(within(card("managed-provider")).getByRole("button", { name: "显示 Managed OpenAI 账号库" }));
    const library = screen.getByRole("tabpanel");
    const grid = library.querySelector("[data-account-library-rows]");
    expect(grid).toHaveAttribute("data-account-library-rows", "2");
    expect(grid?.querySelectorAll("[data-account-card]")).toHaveLength(18);
    expect(within(library).getByRole("region", { name: "账号库滚动区域" })).toHaveAttribute("tabindex", "0");
    expect(screen.queryByRole("button", { name: "下一页" })).not.toBeInTheDocument();
  });

  it("shows account quota, provider aggregate quota, and hides the strip when quota is missing", async () => {
    const user = userEvent.setup();
    renderWorkspace(
      workspaceProps({
        pilotSections: [
          pilotSection({
            directAccounts: [
              pilotAccount({
                accountId: "quota-1",
                displayName: "quota@example.com",
                quotaRemainingUsd: 3.5,
                quota: {
                  providerAccountId: "managed-provider",
                  providerCredentialId: "quota-1",
                  providerType: "openai",
                  source: "cache",
                  status: "ready",
                  ready: true,
                  checkedAt: "2026-08-19T04:00:00Z",
                  nextCheckAt: "2026-08-19T04:05:00Z",
                  nextResetAt: "2026-08-20T00:00:00Z",
                  planType: "free",
                  representativeClaim: null,
                  windows: [
                    {
                      key: "daily",
                      label: "Daily",
                      usedPercent: 25,
                      remainingRatio: 0.75,
                      limitWindowSeconds: 86_400,
                      resetAt: "2026-08-20T00:00:00Z",
                      resetAfterSeconds: 3600,
                    },
                  ],
                  error: null,
                  rawData: {},
                },
              }),
              pilotAccount({ accountId: "quota-none", displayName: "empty@example.com" }),
            ],
          }),
        ],
      }),
    );

    const providerQuota = card("managed-provider").querySelector('[data-quota-scope="provider"]');
    expect(providerQuota).toHaveTextContent("75%");
    expect(providerQuota).toHaveTextContent("≈$3.50");

    await user.click(
      within(card("managed-provider")).getByRole("button", {
        name: "显示 Managed OpenAI 账号库",
      }),
    );
    const quotaCard = document.querySelector('[data-account-card="quota-1"]');
    const emptyCard = document.querySelector('[data-account-card="quota-none"]');
    expect(quotaCard?.querySelector('[data-quota-scope="account"]')).toHaveTextContent("75%");
    expect(quotaCard?.querySelector('[data-quota-scope="account"]')).toHaveTextContent("≈$3.50");
    expect(quotaCard?.querySelectorAll("[data-quota-window]")).toHaveLength(1);
    expect(emptyCard?.querySelector('[data-quota-scope="account"]')).toBeNull();
    expect(quotaCard?.querySelector('[aria-label="容量"]')).toBeNull();
    expect(quotaCard?.querySelector(".nt-provider-account-card__provider-icon")).toBeNull();
    expect(quotaCard?.querySelector("code")).toBeNull();
  });

  it("shows one provider account library at a time", async () => {
    const user = userEvent.setup();
    renderWorkspace(
      workspaceProps({
        pilotSections: [
          pilotSection(),
          pilotSection({
            providerId: "poe",
            providerIds: ["poe"],
            providerLabel: "Poe",
            providerPreset: "poe-openai",
            directAccounts: [pilotAccount({ providerId: "poe", accountId: "poe-1" })],
          }),
        ],
      }),
    );

    await user.click(
      within(card("managed-provider")).getByRole("button", {
        name: "显示 Managed OpenAI 账号库",
      }),
    );
    await user.click(
      within(card("poe")).getByRole("button", {
        name: "显示 Poe 账号库",
      }),
    );

    expect(screen.queryByRole("region", { name: "Managed OpenAI 账号库" })).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Poe 账号库" })).toBeInTheDocument();
  });

  it("renders a categorized account only once when category data overlaps", async () => {
    const user = userEvent.setup();
    const sharedAccount = pilotAccount({ accountId: "shared-account" });
    renderWorkspace(
      workspaceProps({
        pilotSections: [
          pilotSection({
            supportsIdentityCategories: true,
            directAccounts: [],
            identityCategories: [
              {
                id: "free",
                label: "Free",
                count: 1,
                poolTargetSize: 5,
                autoRefillEnabled: false,
                autoPruneEnabled: false,
                accounts: [sharedAccount],
              },
              {
                id: "plus",
                label: "Plus",
                count: 1,
                poolTargetSize: 5,
                autoRefillEnabled: false,
                autoPruneEnabled: false,
                accounts: [sharedAccount],
              },
            ],
          }),
        ],
      }),
    );

    await user.click(
      within(card("managed-provider")).getByRole("button", {
        name: "显示 Managed OpenAI 账号库",
      }),
    );

    const library = screen.getByRole("region", { name: "Managed OpenAI 账号库" });
    expect(library.querySelectorAll('[data-account-card="shared-account"]')).toHaveLength(1);
  });

  it("routes account card actions through the existing account callbacks", async () => {
    const user = userEvent.setup();
    const onToggleDispatch = vi.fn();
    const onEdit = vi.fn();
    const onRemove = vi.fn();
    const onOpenStats = vi.fn();
    const onOpenProbe = vi.fn();
    const onDuplicate = vi.fn();
    const onSetAccountGroup = vi.fn();
    renderWorkspace(
      workspaceProps({
        onToggleDispatch,
        onEdit,
        onRemove,
        onOpenStats,
        onOpenProbe,
        onDuplicate,
        onSetAccountGroup,
        groupOptions: [
          { value: "all", label: "全部分组" },
          { value: "bronze", label: "青铜级别服务" },
        ],
      }),
    );

    await user.click(
      within(card("managed-provider")).getByRole("button", {
        name: "显示 Managed OpenAI 账号库",
      }),
    );
    const library = screen.getByRole("region", { name: "Managed OpenAI 账号库" });

    await user.click(within(library).getByRole("button", { name: "查看 Account 1 统计" }));
    expect(onOpenStats).toHaveBeenCalledWith("managed-provider", expect.objectContaining({ accountId: "acct-1" }));
    await user.click(within(library).getByRole("combobox", { name: "调整 Account 1 分组池" }));
    await user.click(screen.getByRole("option", { name: "青铜级别服务" }));
    expect(onSetAccountGroup).toHaveBeenCalledWith("acct-1", "bronze");

    await user.click(within(library).getByRole("button", { name: "更多操作 acct-1" }));
    expect(within(library).queryByRole("menuitem", { name: "查看统计" })).not.toBeInTheDocument();
    expect(within(library).queryByRole("menuitem", { name: "定时测试" })).not.toBeInTheDocument();
    await user.click(within(library).getByRole("menuitem", { name: "账号测试" }));
    expect(onOpenProbe).toHaveBeenCalledWith("managed-provider", expect.objectContaining({ accountId: "acct-1" }));
    await user.click(within(library).getByRole("button", { name: "更多操作 acct-1" }));
    await user.click(within(library).getByRole("menuitem", { name: "复制账号" }));
    expect(onDuplicate).toHaveBeenCalledWith("managed-provider", expect.objectContaining({ accountId: "acct-1" }));

    await user.click(within(library).getByRole("switch", { name: "调度 acct-1" }));
    expect(onToggleDispatch).toHaveBeenCalledWith("managed-provider", "acct-1", false);
    await user.click(within(library).getByRole("button", { name: "编辑账号 Account 1" }));
    expect(onEdit).toHaveBeenCalledWith("managed-provider", "acct-1");
    await user.click(within(library).getByRole("button", { name: "删除账号 Account 1" }));
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    expect(onRemove).toHaveBeenCalledWith("managed-provider", "acct-1", "Account 1");
  });

  it("offers account creation when an expanded account library is empty", async () => {
    const user = userEvent.setup();
    const onAddExplicit = vi.fn();
    renderWorkspace(
      workspaceProps({
        onAddExplicit,
        pilotSections: [pilotSection({ hasExplicitAccounts: false, directAccounts: [] })],
      }),
    );

    await user.click(
      within(card("managed-provider")).getByRole("button", {
        name: "显示 Managed OpenAI 账号库",
      }),
    );
    const library = screen.getByRole("region", { name: "Managed OpenAI 账号库" });
    expect(within(library).getByText("当前账号库为空")).toBeInTheDocument();
    await user.click(within(library).getByRole("button", { name: "手动录入账号" }));
    expect(onAddExplicit).toHaveBeenCalledWith("managed-provider");
  });

  it("removes the legacy bottom account detail panel while keeping the account library", async () => {
    const user = userEvent.setup();
    renderWorkspace(workspaceProps());

    expect(
      screen.queryByRole("article", { name: /Managed OpenAI 账号明细/ }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^Managed OpenAI$/ }));

    expect(screen.getByRole("region", { name: "Managed OpenAI 账号库" })).toBeInTheDocument();
    expect(
      screen.queryByRole("article", { name: /Managed OpenAI 账号明细/ }),
    ).not.toBeInTheDocument();
  });

});
