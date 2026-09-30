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


describe("AccountsLedgerWorkspace heading", () => {
  it("leaves the page title and its actions to the shell header", () => {
    renderWorkspace(workspaceProps());

    // The shell board header owns "凭据池" plus the 添加服务商 / 添加账号 actions now,
    // so the workspace itself must not repeat them in a third command bar.
    expect(screen.queryByRole("heading", { name: "凭据池" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "添加服务商" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "添加账号" })).not.toBeInTheDocument();
    expect(screen.queryByText(/1 个服务商/)).not.toBeInTheDocument();
    expect(screen.queryByText(/1 个账号.*0 个账号组/)).not.toBeInTheDocument();
  });
});

describe("AccountsLedgerWorkspace provider cards", () => {
  it("renders one card per provider with its category tint and pool capacity", () => {
    renderWorkspace(
      workspaceProps({
          pilotSections: [
            pilotSection(),
            pilotSection({
              providerId: "suno",
              providerIds: ["suno"],
              providerLabel: "Suno",
              providerPreset: "suno",
              hostLabel: "studio-api-prod.suno.com",
              directAccounts: [pilotAccount({ providerId: "suno", accountId: "suno-1" })],
            }),
            pilotSection({
              providerId: "custom-openai",
              providerIds: ["custom-openai"],
              providerLabel: "Custom OpenAI",
              providerPreset: null,
              adapter: "openai_compatible",
              protocolProfile: "openai_compatible_generic",
              directAccounts: [
                pilotAccount({ providerId: "custom-openai", accountId: "custom-1" }),
              ],
            }),
          ],
      }),
    );

    expect(card("managed-provider").dataset.providerCategory).toBe("mainstream");
    expect(card("suno").dataset.providerCategory).toBe("media");
    expect(card("custom-openai").dataset.providerCategory).toBe("third-party-compatible");
    const managedFront = card("managed-provider").querySelector(".nt-provider-card__front");
    const sunoFront = card("suno").querySelector(".nt-provider-card__front");
    expect(managedFront).not.toBeNull();
    expect(sunoFront).not.toBeNull();
    expect(managedFront?.querySelector('[data-provider-icon="openai"]')).not.toBeNull();
    expect(sunoFront?.querySelector('[data-provider-icon="suno"]')).not.toBeNull();
    expect(within(managedFront as HTMLElement).getByText("1/30")).toBeInTheDocument();
    // No history must not render a fabricated success rate.
    expect(managedFront?.querySelector('[data-provider-availability-scope="provider"]')).toBeNull();
  });

  it("keeps lifecycle controls off the front and restores focus across a card flip", async () => {
    const user = userEvent.setup();
    renderWorkspace(workspaceProps());

    const providerCard = card("managed-provider");
    const cardInner = providerCard.querySelector(".nt-provider-card__inner");
    const cardFront = providerCard.querySelector(".nt-provider-card__front");
    const cardBack = providerCard.querySelector(".nt-provider-card__back");
    const flipButton = within(providerCard).getByRole("button", {
      name: /翻面查看 Managed OpenAI 详情/,
    });

    expect(cardInner).not.toBeNull();
    expect(cardFront).toHaveAttribute("aria-hidden", "false");
    expect(cardFront).not.toHaveAttribute("inert");
    expect(cardBack).toHaveAttribute("aria-hidden", "true");
    expect(cardBack).toHaveAttribute("inert");
    expect(providerCard).toHaveAttribute("data-provider-card-side", "front");
    expect(
      screen.queryByRole("switch", { name: /Managed OpenAI 自动补号/ }),
    ).not.toBeInTheDocument();
    expect(flipButton).toHaveAttribute("aria-pressed", "false");
    expect(flipButton.querySelector(".lucide-gallery-horizontal-end")).not.toBeNull();
    expect(
      screen.queryByRole("region", { name: /Managed OpenAI 账号生命周期/ }),
    ).not.toBeInTheDocument();

    flipButton.focus();
    await user.keyboard("{Enter}");

    expect(providerCard).toHaveAttribute("data-provider-card-side", "back");
    expect(cardFront).toHaveAttribute("aria-hidden", "true");
    expect(cardFront).toHaveAttribute("inert");
    expect(cardBack).toHaveAttribute("aria-hidden", "false");
    expect(cardBack).not.toHaveAttribute("inert");
    expect(screen.getByRole("switch", { name: /Managed OpenAI 自动补号/ })).toBeInTheDocument();
    expect(
      screen.getByRole("spinbutton", { name: /Managed OpenAI 目标号池容量/ }),
    ).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /Managed OpenAI 账号生命周期/ })).toBeInTheDocument();
    expect(screen.getByText("可用号池")).toBeInTheDocument();
    expect(screen.getByText("冷却池")).toBeInTheDocument();
    expect(screen.getByText("失效号")).toBeInTheDocument();
    expect(
      screen.getByText(
        "/v1/internal/gateway/credential-pool-refill/providers/managed-provider/tasks/claim",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText("api.openai.com")).not.toBeInTheDocument();
    expect(screen.queryByText("openai")).not.toBeInTheDocument();
    expect(screen.queryByText("最近 2 分钟前")).not.toBeInTheDocument();

    const returnButton = within(card("managed-provider")).getByRole("button", {
      name: /翻回 Managed OpenAI 卡牌正面/,
    });
    expect(returnButton).toHaveAttribute("aria-pressed", "true");
    expect(returnButton.querySelector(".lucide-gallery-horizontal-end")).not.toBeNull();
    expect(returnButton).toHaveFocus();
    expect(screen.getByRole("status")).toHaveTextContent("Managed OpenAI 卡牌背面已显示");

    await user.keyboard(" ");

    expect(providerCard).toHaveAttribute("data-provider-card-side", "front");
    expect(
      screen.queryByRole("switch", { name: /Managed OpenAI 自动补号/ }),
    ).not.toBeInTheDocument();
    expect(
      within(card("managed-provider")).getByRole("button", {
        name: /翻面查看 Managed OpenAI 详情/,
      }),
    ).toHaveFocus();
    expect(screen.getByRole("status")).toHaveTextContent("Managed OpenAI 卡牌正面已显示");
  });

  it("flips each card independently", async () => {
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
      within(card("managed-provider")).getByRole("button", { name: /翻面查看 Managed OpenAI 详情/ }),
    );

    expect(card("managed-provider").className).toContain("nt-provider-card--flipped");
    expect(card("poe").className).not.toContain("nt-provider-card--flipped");
    expect(within(card("poe")).getByRole("button", { name: /翻面查看 Poe 详情/ })).toBeInTheDocument();
  });

  it("renders one-line provider totals and provider-level availability cells", () => {
    renderWorkspace(
      workspaceProps({
        pilotSections: [
          pilotSection({
            poolTargetSize: 5,
            directAccounts: [
              pilotAccount({
                accountId: "available",
                concurrencyUsed: 2,
                concurrencyTotal: 4,
                upstreamCost: 1.25,
                userCost: 2.5,
                successWindows: [{ label: "10:00", success: 3, requests: 4 }],
              }),
              pilotAccount({
                accountId: "recovering",
                statusLabel: "限流等待恢复",
                concurrencyUsed: 1,
                concurrencyTotal: 2,
                upstreamCost: 0.75,
                userCost: 1.5,
                successWindows: [{ label: "10:00", success: 1, requests: 2 }],
              }),
              pilotAccount({ accountId: "invalid", statusLabel: "失效", dispatchEnabled: false }),
            ],
          }),
        ],
      }),
    );

    const providerCard = card("managed-provider");
    expect(within(providerCard).getByRole("img", { name: /可用 1，待恢复 1，失效 1，待观测 0，剩余 2/ })).toBeInTheDocument();
    expect(within(providerCard).getByText("3/6")).toBeInTheDocument();
    expect(within(providerCard).getByText("≈$2.00")).toBeInTheDocument();
    expect(within(providerCard).getByText("$4.00")).toBeInTheDocument();
    expect(providerCard.querySelector('[data-provider-metric="requests"]')).toHaveTextContent("36");
    expect(providerCard.querySelector('[data-provider-metric="success-rate"]')).toHaveTextContent(
      "66.7%",
    );
    expect(providerCard.querySelectorAll('[data-provider-availability-cell="success"]')).toHaveLength(
      8,
    );
    expect(providerCard.querySelectorAll('[data-provider-availability-cell="failure"]')).toHaveLength(
      4,
    );
    expect(
      within(providerCard).getByRole("img", {
        name: /最近窗口的调用成功率，所有模型聚合，4\/6 次成功，66\.7%/,
      }),
    ).toHaveAttribute("title", expect.stringContaining("所有模型聚合，按小时"));
    expect(
      providerCard.querySelector('[data-provider-availability-window="10:00"]'),
    ).toHaveAttribute("title", "10:00：4/6 次成功（67%）");
  });

  it("edits lifecycle policy and confirms delete or purge actions inside the app", async () => {
    const user = userEvent.setup();
    const onUpdateProviderPoolTargetSize = vi.fn();
    const onToggleProviderAutoRefill = vi.fn();
    const onToggleProviderAutoPrune = vi.fn();
    const onToggleProviderPermanentDelete = vi.fn();
    const onRequestProviderRefill = vi.fn();
    const onPruneProviderCredentials = vi.fn();
    const onPurgeProviderArchive = vi.fn();
    renderWorkspace(
      workspaceProps({
        onUpdateProviderPoolTargetSize,
        onToggleProviderAutoRefill,
        onToggleProviderAutoPrune,
        onToggleProviderPermanentDelete,
        onRequestProviderRefill,
        onPruneProviderCredentials,
        onPurgeProviderArchive,
      }),
    );

    await user.click(
      within(card("managed-provider")).getByRole("button", {
        name: /翻面查看 Managed OpenAI 详情/,
      }),
    );

    const targetInput = screen.getByRole("spinbutton", {
      name: /Managed OpenAI 目标号池容量/,
    });
    await user.clear(targetInput);
    await user.type(targetInput, "42");
    await user.tab();
    expect(onUpdateProviderPoolTargetSize).toHaveBeenCalledWith("managed-provider", 42);

    await user.click(screen.getByRole("switch", { name: /Managed OpenAI 自动补号/ }));
    expect(onToggleProviderAutoRefill).toHaveBeenCalledWith("managed-provider", true);
    await user.click(screen.getByRole("button", { name: /Managed OpenAI 手动补号/ }));
    expect(onRequestProviderRefill).toHaveBeenCalledWith("managed-provider");

    await user.click(screen.getByRole("switch", { name: /Managed OpenAI 自动删除失效号/ }));
    expect(onToggleProviderAutoPrune).toHaveBeenCalledWith("managed-provider", true);
    await user.click(screen.getByRole("button", { name: /Managed OpenAI 手动删除失效号/ }));
    expect(screen.getByRole("dialog", { name: "确认归档失效号" })).toBeInTheDocument();
    expect(onPruneProviderCredentials).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "归档失效号" }));
    expect(onPruneProviderCredentials).toHaveBeenCalledWith("managed-provider");

    await user.click(screen.getByRole("switch", { name: /Managed OpenAI 彻底删除模式/ }));
    expect(screen.getByRole("dialog", { name: "开启彻底删除模式" })).toBeInTheDocument();
    expect(onToggleProviderPermanentDelete).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认开启" }));
    expect(onToggleProviderPermanentDelete).toHaveBeenCalledWith("managed-provider", true);

    await user.click(screen.getByRole("button", { name: /Managed OpenAI 手动清空账号归档/ }));
    expect(screen.getByRole("dialog", { name: "确认清空账号归档" })).toBeInTheDocument();
    expect(onPurgeProviderArchive).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "彻底删除归档" }));
    expect(onPurgeProviderArchive).toHaveBeenCalledWith("managed-provider");
  });

  it("copies provider-scoped values and edits the storage password in the current draft", async () => {
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, "writeText");
    const onUpdateProviderStoragePassword = vi.fn(() => true);
    renderWorkspace(workspaceProps({ onUpdateProviderStoragePassword }));

    const providerCard = card("managed-provider");
    await user.click(
      within(providerCard).getByRole("button", {
        name: /翻面查看 Managed OpenAI 详情/,
      }),
    );

    const archive = providerCard.querySelector(".nt-provider-lifecycle__archive");
    const endpoints = providerCard.querySelector(".nt-provider-lifecycle__endpoints");
    expect(archive).not.toBeNull();
    expect(endpoints).not.toBeNull();
    expect(
      (archive as HTMLElement).compareDocumentPosition(endpoints as HTMLElement) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).not.toBe(0);

    await user.click(
      within(providerCard).getByRole("button", {
        name: /复制 Managed OpenAI 补号通知 API/,
      }),
    );
    expect(writeText).toHaveBeenLastCalledWith(
      "/v1/internal/gateway/credential-pool-refill/providers/managed-provider/tasks/claim",
    );

    await user.click(
      within(providerCard).getByRole("button", {
        name: /编辑 Managed OpenAI 存储密码/,
      }),
    );
    const passwordInput = within(providerCard).getByLabelText("Managed OpenAI 存储密码");
    await user.type(passwordInput, "provider-storage-secret");
    await user.click(
      within(providerCard).getByRole("button", {
        name: /保存 Managed OpenAI 存储密码/,
      }),
    );
    expect(onUpdateProviderStoragePassword).toHaveBeenCalledWith(
      "managed-provider",
      "provider-storage-secret",
    );

    await user.click(
      within(providerCard).getByRole("button", {
        name: /复制 Managed OpenAI 存储密码/,
      }),
    );
    expect(writeText).toHaveBeenLastCalledWith("provider-storage-secret");
  });

  it("runs provider actions and confirms deletion inside the app", async () => {
    const user = userEvent.setup();
    const onToggleDispatch = vi.fn();
    const onEdit = vi.fn();
    const onRemove = vi.fn();
    renderWorkspace(workspaceProps({ onToggleDispatch, onEdit, onRemove }));

    await user.click(screen.getByRole("switch", { name: /Managed OpenAI 调度开关/ }));
    expect(onToggleDispatch).toHaveBeenCalledWith("managed-provider", "acct-1", false);

    await user.click(screen.getByRole("button", { name: "编辑" }));
    expect(onEdit).toHaveBeenCalledWith("managed-provider", "acct-1");

    await user.click(screen.getByRole("button", { name: "删除" }));
    expect(screen.getByRole("dialog", { name: "确认删除账号" })).toBeInTheDocument();
    expect(onRemove).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(onRemove).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "删除" }));
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    expect(onRemove).toHaveBeenCalledWith("managed-provider", "acct-1", "Account 1");
  });

  it("offers provider tests for multi-account providers without account-only actions", async () => {
    const user = userEvent.setup();
    const section = pilotSection({
      directAccounts: [
        pilotAccount({ accountId: "acct-1" }),
        pilotAccount({ accountId: "acct-2", displayName: "Account 2" }),
      ],
    });
    const onOpenProviderProbe = vi.fn();
    const onOpenProviderSchedule = vi.fn();
    renderWorkspace(
      workspaceProps({
        pilotSections: [section],
        onOpenProviderProbe,
        onOpenProviderSchedule,
      }),
    );

    expect(screen.getByRole("button", { name: "编辑" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "删除" })).toBeDisabled();
    const moreButton = screen.getByRole("button", { name: /Managed OpenAI 更多操作/ });
    await user.click(moreButton);
    await user.click(screen.getByRole("menuitem", { name: "服务商测试" }));
    expect(onOpenProviderProbe).toHaveBeenCalledWith(section);
    await user.click(moreButton);
    await user.click(screen.getByRole("menuitem", { name: "自动定时测试" }));
    expect(onOpenProviderSchedule).toHaveBeenCalledWith(section);
    expect(screen.queryByRole("menuitem", { name: "查看账号明细" })).not.toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "复制账号" })).not.toBeInTheDocument();
  });
});
