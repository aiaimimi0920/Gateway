import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import {
  pilotAccount,
  pilotSection,
  refillDemand,
  renderWorkspace,
  workspaceProps,
} from "./AccountsLedgerWorkspace.fixtures";

async function chooseProvider(user: ReturnType<typeof userEvent.setup>) {
  await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /Managed OpenAI/ }));
}

async function openSettings(user: ReturnType<typeof userEvent.setup>) {
  await chooseProvider(user);
  await user.click(screen.getByRole("button", { name: "服务商设置" }));
  return screen.getByRole("dialog", { name: "服务商设置" });
}

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
  it("selects a provider before exposing its lifecycle controls", async () => {
    const user = userEvent.setup();
    renderWorkspace(workspaceProps());
    expect(screen.queryByRole("button", { name: "服务商设置" })).not.toBeInTheDocument();
    await chooseProvider(user);
    expect(screen.getByRole("heading", { name: "Managed OpenAI" })).toBeVisible();
    expect(screen.getByRole("region", { name: "Managed OpenAI 统计" })).toHaveTextContent("1 / 30");
    expect(screen.queryByRole("switch", { name: /自动补号/ })).not.toBeInTheDocument();
    const opener = screen.getByRole("button", { name: "服务商设置" });
    await user.click(opener);
    expect(screen.getByRole("dialog", { name: "服务商设置" })).toBeVisible();
    expect(screen.getByRole("switch", { name: /Managed OpenAI 自动补号/ })).toBeVisible();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(opener).toHaveFocus());
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

    await openSettings(user);

    await user.click(screen.getByRole("button", { name: /编辑 Managed OpenAI 最大可用池/ }));
    const targetInput = screen.getByRole("spinbutton", { name: /Managed OpenAI 最大可用池/ });
    await user.clear(targetInput);
    await user.type(targetInput, "42");
    await user.click(screen.getByRole("button", { name: /保存 Managed OpenAI 最大可用池/ }));
    expect(onUpdateProviderPoolTargetSize).toHaveBeenCalledWith("managed-provider", 42);

    await user.click(screen.getByRole("switch", { name: /Managed OpenAI 自动补号/ }));
    expect(onToggleProviderAutoRefill).toHaveBeenCalledWith("managed-provider", true);
    await user.click(screen.getByRole("button", { name: /Managed OpenAI 手动补号/ }));
    expect(onRequestProviderRefill).toHaveBeenCalledWith("managed-provider");

    await user.click(screen.getByRole("switch", { name: /Managed OpenAI 自动删除失效号/ }));
    expect(onToggleProviderAutoPrune).toHaveBeenCalledWith("managed-provider", true);
    await user.click(screen.getByRole("button", { name: /Managed OpenAI 手动删除失效号/ }));
    expect(screen.queryByRole("alertdialog", { name: "确认归档失效号" })).not.toBeInTheDocument();
    expect(onPruneProviderCredentials).toHaveBeenCalledWith("managed-provider");

    await user.click(screen.getByRole("switch", { name: /Managed OpenAI 彻底删除模式/ }));
    expect(screen.getByRole("alertdialog", { name: "开启彻底删除模式" })).toBeInTheDocument();
    expect(onToggleProviderPermanentDelete).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认开启" }));
    expect(onToggleProviderPermanentDelete).toHaveBeenCalledWith("managed-provider", true);

    await user.click(screen.getByRole("button", { name: /Managed OpenAI 手动清空账号归档/ }));
    expect(screen.getByRole("alertdialog", { name: "确认清空账号归档" })).toBeInTheDocument();
    expect(onPurgeProviderArchive).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "彻底删除归档" }));
    expect(onPurgeProviderArchive).toHaveBeenCalledWith("managed-provider");
  });

  it("shows unsupported cloud purge as restricted without opening a delete dialog", async () => {
    const user = userEvent.setup();
    const onPurgeProviderArchive = vi.fn();
    renderWorkspace(workspaceProps({
      onPurgeProviderArchive, refillByProvider: new Map([["managed-provider", {
        ...refillDemand(), archivePurgeSupported: false, archivePurgeUnsupportedReason: "Conditional delete is unverified",
      }]])
    }));
    await openSettings(user);
    const trigger = screen.getByRole("button", { name: /Managed OpenAI 手动清空账号归档/ });
    expect(trigger).toBeDisabled();
    expect(trigger).toHaveTextContent("受限");
    expect(trigger).toHaveAttribute("title", "Conditional delete is unverified");
    await user.click(trigger);
    expect(onPurgeProviderArchive).not.toHaveBeenCalled();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("preserves unavailable counts and read errors when cloud purge is also restricted", async () => {
    const user = userEvent.setup();
    renderWorkspace(workspaceProps({
      refillByProvider: new Map([["managed-provider", {
        ...refillDemand(), archivedCredentialCount: null, archiveStorageError: "Storage authentication failed",
        archivePurgeSupported: false, archivePurgeUnsupportedReason: "Conditional delete is unverified",
      }]])
    }));
    await openSettings(user);
    const trigger = screen.getByRole("button", { name: /Managed OpenAI 手动清空账号归档/ });
    expect(trigger).toBeDisabled();
    expect(trigger).toHaveTextContent("清空（不可用）");
    expect(trigger).not.toHaveTextContent("0");
    expect(trigger).toHaveAttribute("title", "Storage authentication failed; Conditional delete is unverified");
  });

  it("cannot open purge confirmation against an uncommitted archive path", async () => {
    const user = userEvent.setup();
    renderWorkspace(workspaceProps({ lifecycleActionsLocked: true }));
    await openSettings(user);
    const trigger = screen.getByRole("button", { name: /Managed OpenAI 手动清空账号归档/ });
    expect(trigger).toBeDisabled();
    await user.click(trigger);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("returns focus to the destructive action after Cancel or Escape", async () => {
    const user = userEvent.setup();
    const onPurgeProviderArchive = vi.fn();
    renderWorkspace(workspaceProps({ onPurgeProviderArchive }));
    await openSettings(user);
    const trigger = screen.getByRole("button", { name: /Managed OpenAI 手动清空账号归档/ });
    await user.click(trigger);
    await user.click(screen.getByRole("button", { name: "取消" }));
    await waitFor(() => expect(trigger).toHaveFocus());
    await user.keyboard("{Enter}{Escape}");
    await waitFor(() => expect(trigger).toHaveFocus());
    expect(onPurgeProviderArchive).not.toHaveBeenCalled();
  });

  it("copies provider-scoped values and edits the storage password in the current draft", async () => {
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, "writeText");
    const onUpdateProviderStoragePassword = vi.fn(() => true);
    renderWorkspace(workspaceProps({ onUpdateProviderStoragePassword }));

    const providerCard = await openSettings(user);

    expect(providerCard.querySelectorAll(".nt-provider-lifecycle__group")).toHaveLength(3);
    expect(providerCard.querySelector(".nt-provider-lifecycle__endpoint a")).toBeNull();

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

    expect(within(providerCard).queryByDisplayValue("provider-storage-secret")).not.toBeInTheDocument();
    await user.click(within(providerCard).getByRole("button", { name: /编辑 Managed OpenAI 存储密码/ }));
    expect(within(providerCard).getByLabelText("Managed OpenAI 存储密码")).toHaveValue("");

  });

  it("keeps dispatch and confirms whole-pool deletion without account editing", async () => {
    const user = userEvent.setup();
    const onToggleDispatch = vi.fn();
    const onRemoveProvider = vi.fn();
    const onRemove = vi.fn();
    renderWorkspace(workspaceProps({ onToggleDispatch, onRemoveProvider, onRemove }));
    await chooseProvider(user);
    await user.click(screen.getByRole("switch", { name: "服务商调度" }));
    expect(onToggleDispatch).toHaveBeenCalledWith("managed-provider", "acct-1", false);
    expect(screen.queryByRole("button", { name: "编辑" })).not.toBeInTheDocument();
    await user.click(screen.getByLabelText("更多操作"));
    const remove = screen.getByRole("button", { name: "删除服务商" });
    await user.click(remove);
    expect(screen.getByRole("alertdialog", { name: "确认删除凭据池" })).toBeInTheDocument();
    expect(onRemoveProvider).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(onRemoveProvider).not.toHaveBeenCalled();
    await user.click(remove);
    await user.keyboard("{Escape}");
    expect(onRemoveProvider).not.toHaveBeenCalled();
    await user.click(remove);
    await user.click(screen.getByRole("button", { name: "确认删除凭据池" }));
    expect(onRemoveProvider).toHaveBeenCalledExactlyOnceWith("managed-provider");
    expect(onRemove).not.toHaveBeenCalled();
  });

  it("provides the same four actions for multi-account pools", async () => {
    const user = userEvent.setup();
    const section = pilotSection({ directAccounts: [pilotAccount({ accountId: "acct-1" }), pilotAccount({ accountId: "acct-2" })] });
    const onOpenProviderSchedule = vi.fn();
    const onOpenModelMapping = vi.fn();
    renderWorkspace(workspaceProps({ pilotSections: [section], onOpenProviderSchedule, onOpenModelMapping }));
    await chooseProvider(user);
    expect(screen.getByRole("switch", { name: "服务商调度" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "测试" }));
    expect(onOpenProviderSchedule).toHaveBeenCalledWith(section);
    await user.click(screen.getByRole("button", { name: "模型映射" }));
    expect(onOpenModelMapping).toHaveBeenCalledWith(section);
    await user.click(screen.getByLabelText("更多操作"));
    expect(screen.getByRole("button", { name: "删除服务商" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: /Managed OpenAI 更多操作/ })).not.toBeInTheDocument();
  });
});
