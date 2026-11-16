import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import {
  pilotAccount,
  pilotSection,
  renderWorkspace,
  workspaceProps,
} from "./AccountsLedgerWorkspace.fixtures";

describe("AccountsLedgerWorkspace account library", () => {
  it("opens account-scoped statistics and returns to the selected provider list", async () => {
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

    const provider = within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /Managed OpenAI/ });
    await user.click(provider);
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(3);
    await user.click(screen.getByRole("button", { name: "Account 1" }));
    const statistics = screen.getByRole("region", { name: "Account 1 统计" });
    expect(statistics).toHaveTextContent("2/4");
    expect(statistics).toHaveTextContent("75%");
    expect(statistics).toHaveTextContent("≈$1.25");
    expect(statistics).toHaveTextContent("$2.50");
    expect(screen.queryByRole("region", { name: "Managed OpenAI 统计" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "← 返回账号列表" }));
    expect(provider).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("heading", { name: "Managed OpenAI" })).toBeInTheDocument();
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(3);
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

    // Source libraries no longer partition the directory; routing membership remains account-scoped.
    expect(within(screen.getByRole("table")).getByText("free@example.com")).toBeInTheDocument();
    expect(within(screen.getByRole("table")).getByText("plus@example.com")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "plus@example.com" }));
    await user.click(screen.getByRole("button", { name: "账号配置" }));
    expect(screen.getByRole("combobox", { name: "调整 plus@example.com 分组池" })).toHaveTextContent("青铜级别服务");
    expect(screen.queryByRole("combobox", { name: "调整 free@example.com 分组池" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "← 返回账号列表" }));
    await user.click(screen.getByRole("button", { name: "free@example.com" }));
    await user.click(screen.getByRole("button", { name: "账号配置" }));
    expect(screen.getByRole("combobox", { name: "调整 free@example.com 分组池" })).toHaveTextContent("青铜级别服务");
  });

  it("pages the account directory without dropping accounts and resets the page on search", async () => {
    const user = userEvent.setup();
    const accounts = Array.from({ length: 21 }, (_, index) => pilotAccount({
      accountId: `acct-${index + 1}`, displayName: `account-${index + 1}@example.com`, libraryName: "free",
    }));
    renderWorkspace(workspaceProps({ pilotSections: [pilotSection({ directAccounts: accounts })] }));
    await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /Managed OpenAI/ }));
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(21);
    expect(screen.getByRole("button", { name: "account-20@example.com" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "account-21@example.com" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "account-21@example.com" })).toBeInTheDocument();
    await user.type(screen.getByRole("searchbox", { name: "搜索账号" }), "account-1@");
    expect(screen.getByRole("button", { name: "account-1@example.com" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "上一页" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "下一页" })).toBeDisabled();
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

    await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /Managed OpenAI/ }));
    const providerQuota = screen.getByRole("region", { name: "Managed OpenAI 统计" }).querySelector('[data-quota-scope="provider"]');
    expect(providerQuota).toHaveTextContent("75%");
    expect(providerQuota).toHaveTextContent("≈$3.50");

    await user.click(screen.getByRole("button", { name: "quota@example.com" }));
    const quotaCard = screen.getByRole("region", { name: "quota@example.com 统计" });
    expect(quotaCard?.querySelector('[data-quota-scope="account"]')).toHaveTextContent("75%");
    expect(quotaCard?.querySelector('[data-quota-scope="account"]')).toHaveTextContent("≈$3.50");
    expect(quotaCard?.querySelectorAll("[data-quota-window]")).toHaveLength(1);
    expect(quotaCard?.querySelector('[aria-label="容量"]')).toBeNull();
    expect(quotaCard?.querySelector(".nt-provider-account-card__provider-icon")).toBeNull();
    expect(quotaCard?.querySelector("code")).toBeNull();
    await user.click(screen.getByRole("button", { name: "← 返回账号列表" }));
    await user.click(screen.getByRole("button", { name: "empty@example.com" }));
    expect(screen.getByRole("region", { name: "empty@example.com 统计" }).querySelector('[data-quota-scope="account"]')).toBeNull();
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
            directAccounts: [pilotAccount({ providerId: "poe", accountId: "poe-1", displayName: "Poe account" })],
          }),
        ],
      }),
    );

    const directory = within(screen.getByRole("navigation", { name: "服务商目录" }));
    await user.click(directory.getByRole("button", { name: /Managed OpenAI/ }));
    expect(screen.getByRole("button", { name: "Account 1" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Poe account" })).not.toBeInTheDocument();
    await user.click(directory.getByRole("button", { name: /Poe/ }));
    expect(screen.getByRole("heading", { name: "Poe" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Account 1" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Poe account" })).toBeInTheDocument();
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

    await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /Managed OpenAI/ }));
    expect(within(screen.getByRole("table")).getAllByRole("row")).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "Account 1" })).toHaveLength(1);
  });

  it("routes account card actions through the existing account callbacks", async () => {
    const user = userEvent.setup();
    const onToggleDispatch = vi.fn();
    const onEdit = vi.fn();
    const onRemove = vi.fn();
    const onOpenStats = vi.fn();
    const onOpenProbe = vi.fn();
    const onOpenModelMapping = vi.fn();
    const onSetAccountGroup = vi.fn();
    renderWorkspace(
      workspaceProps({
        onToggleDispatch,
        onEdit,
        onRemove,
        onOpenStats,
        onOpenProbe,
        onOpenModelMapping,
        onSetAccountGroup,
        groupOptions: [
          { value: "all", label: "全部分组" },
          { value: "bronze", label: "青铜级别服务" },
        ],
      }),
    );

    await user.click(screen.getByRole("button", { name: "Account 1" }));
    await user.click(screen.getByRole("button", { name: "账号配置" }));
    const library = document.querySelector(".nt-credential-account-settings") as HTMLElement;

    await user.click(within(library).getByRole("combobox", { name: "调整 Account 1 分组池" }));
    await user.click(screen.getByRole("option", { name: "青铜级别服务" }));
    expect(onSetAccountGroup).toHaveBeenCalledWith("acct-1", "bronze");

    await user.click(within(library).getByRole("button", { name: "测试账号 Account 1" }));
    expect(onOpenProbe).toHaveBeenCalledWith("managed-provider", expect.objectContaining({ accountId: "acct-1" }));
    await user.click(within(library).getByRole("button", { name: "模型映射 Account 1" }));
    expect(onOpenModelMapping).toHaveBeenCalledWith(expect.objectContaining({ providerId: "managed-provider" }));

    await user.click(within(library).getByRole("switch", { name: "调度 acct-1" }));
    expect(onToggleDispatch).toHaveBeenCalledWith("managed-provider", "acct-1", false);
    await user.click(within(library).getByRole("button", { name: "编辑账号 Account 1" }));
    expect(onEdit).toHaveBeenCalledWith("managed-provider", "acct-1");
    await user.click(within(library).getByRole("button", { name: "删除账号 Account 1" }));
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    expect(onRemove).toHaveBeenCalledWith("managed-provider", "acct-1", "Account 1");
  });

  it("offers account creation for an empty provider directory", async () => {
    const user = userEvent.setup();
    const onAddExplicit = vi.fn();
    renderWorkspace(
      workspaceProps({
        onAddExplicit,
        pilotSections: [pilotSection({ hasExplicitAccounts: false, directAccounts: [] })],
      }),
    );

    await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /Managed OpenAI/ }));
    expect(screen.getByText("尚未添加账号")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "添加账号" }));
    expect(onAddExplicit).toHaveBeenCalledWith("managed-provider");
  });

  it("removes the legacy bottom account detail panel while keeping the account library", async () => {
    const user = userEvent.setup();
    renderWorkspace(workspaceProps());

    expect(
      screen.queryByRole("article", { name: /Managed OpenAI 账号明细/ }),
    ).not.toBeInTheDocument();
    await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /Managed OpenAI/ }));

    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Account 1" })).toBeInTheDocument();
    expect(
      screen.queryByRole("article", { name: /Managed OpenAI 账号明细/ }),
    ).not.toBeInTheDocument();
  });

});
