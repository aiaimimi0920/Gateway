import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { NeuroTooltipProvider } from "../../components/ActionTooltip";
import { buildRouteAccountCatalog } from "./routeAccountCatalog";
import { AccountGroupDialog } from "./AccountGroupDialog";
import { type AccountGroupCreateValue } from "./accountGroupDraft";

function renderDialog(onSubmit = vi.fn((_value: AccountGroupCreateValue) => true), locked = false, accounts = buildRouteAccountCatalog({ providers: [], model_routes: [], aliases: {} }).accounts) {
  function Harness() {
    const [open, setOpen] = useState(false);
    return <UiLocaleProvider><NeuroTooltipProvider>
      <button onClick={() => setOpen(true)}>添加分组</button>
      <AccountGroupDialog open={open} locked={locked} accounts={accounts} existingGroupIds={["existing"]}
        onOpenChange={setOpen} onSubmit={onSubmit} />
    </NeuroTooltipProvider></UiLocaleProvider>;
  }
  render(<Harness />);
  return onSubmit;
}

describe("AccountGroupDialog", () => {
  beforeEach(() => window.localStorage.clear());

  it("reuses the edit layout and stages members locally until creation", async () => {
    const user = userEvent.setup();
    const accounts = buildRouteAccountCatalog({
      providers: [{ id: "provider-a", credentials: [{ id: "account-a", account_name: "Account A" }] }],
      model_routes: [], aliases: {},
    }).accounts;
    const onSubmit = renderDialog(undefined, false, accounts);
    await user.click(screen.getByRole("button", { name: "添加分组" }));
    expect(screen.getByRole("dialog")).toHaveClass("nt-group-edit-dialog");
    expect(screen.getByRole("region", { name: "成员管理" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "移除分组" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "加入 Account A" }));
    expect(onSubmit).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(onSubmit).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "添加分组" }));
    expect(screen.getByRole("button", { name: "加入 Account A" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "加入 Account A" }));
    await user.selectOptions(screen.getByLabelText("候选范围"), "members");
    expect(screen.getByRole("button", { name: "移除 Account A" })).toBeInTheDocument();
    await user.type(screen.getByLabelText("分组 ID"), "with-members");
    await user.click(screen.getByRole("button", { name: "创建分组" }));
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({
      groupId: "with-members", providerCredentialIds: ["account-a"],
    }));
  });

  it("focuses the ID field, traps focus, cancels, and restores the triggering button", async () => {
    const user = userEvent.setup();
    const onSubmit = renderDialog();
    const trigger = screen.getByRole("button", { name: "添加分组" });
    await user.click(trigger);
    await waitFor(() => expect(screen.getByLabelText("分组 ID")).toHaveFocus());
    await user.type(screen.getByLabelText("分组 ID"), "cancelled");
    await user.keyboard("{Shift>}{Tab}{/Shift}");
    expect(screen.getByRole("dialog")).toContainElement(document.activeElement as HTMLElement);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(trigger).toHaveFocus());
    expect(onSubmit).not.toHaveBeenCalled();
    await user.click(trigger);
    expect(screen.getByLabelText("分组 ID")).toHaveValue("");
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("validates missing and duplicate IDs, and invalid multipliers before submitting once", async () => {
    const user = userEvent.setup();
    const onSubmit = renderDialog();
    await user.click(screen.getByRole("button", { name: "添加分组" }));
    await user.click(screen.getByRole("button", { name: "创建分组" }));
    expect(screen.getByLabelText("分组 ID")).toHaveAccessibleDescription("分组 ID 必须填写。");
    fireEvent.change(screen.getByLabelText("分组 ID"), { target: { value: " existing " } });
    expect(screen.getByRole("alert")).toHaveTextContent("分组 ID 已存在");
    fireEvent.change(screen.getByLabelText("分组 ID"), { target: { value: " new-group " } });
    for (const multiplier of ["-1", "1abc", "Infinity", "1e999"]) {
      fireEvent.change(screen.getByLabelText("计费倍率"), { target: { value: multiplier } });
      await user.click(screen.getByRole("button", { name: "创建分组" }));
      expect(screen.getByLabelText("计费倍率")).toHaveAttribute("aria-invalid", "true");
      expect(onSubmit).not.toHaveBeenCalled();
    }
    fireEvent.change(screen.getByLabelText("计费倍率"), { target: { value: "1e-2" } });
    await user.type(screen.getByLabelText("分组名称"), " Team ");
    await user.type(screen.getByLabelText("描述"), " Description ");
    await user.click(screen.getByRole("checkbox", { name: "Team 启用状态" }));
    await user.click(screen.getByRole("button", { name: "创建分组" }));
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith({
      groupId: "new-group", name: "Team", description: "Description",
      billingMultiplier: "1e-2", enabled: false, notes: "",
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("retains the form if the route owner rejects creation", async () => {
    const user = userEvent.setup();
    renderDialog(vi.fn(() => false));
    await user.click(screen.getByRole("button", { name: "添加分组" }));
    await user.type(screen.getByLabelText("分组 ID"), "valid");
    await user.click(screen.getByRole("button", { name: "创建分组" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByLabelText("分组 ID")).toHaveValue("valid");
  });

  it("blocks mutations when locked even if the form receives a submit event", async () => {
    const user = userEvent.setup();
    const onSubmit = renderDialog(undefined, true);
    await user.click(screen.getByRole("button", { name: "添加分组" }));
    expect(screen.getByLabelText("分组 ID")).toBeDisabled();
    expect(screen.getByRole("button", { name: "创建分组" })).toBeDisabled();
    fireEvent.submit(screen.getByRole("button", { name: "创建分组" }).closest("form")!);
    expect(onSubmit).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "关闭" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
