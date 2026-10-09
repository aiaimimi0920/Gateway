import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ConsoleAccessKeyBalance } from "../../api/contracts";
import {
  deferred,
  renderWithProviders,
} from "./BrowserConsoleApp.render-fixture";
import {
  fixture,
  Harness,
  key,
  openCreate,
} from "./AccessKeysSection.fixtures";
import { editedAccessKeyInput, editAccessKeyDraft } from "./accessKeyEditing";
import { keyQuotaInput, validKeyQuota } from "./accessKeyQuota";

function balance(id: string): ConsoleAccessKeyBalance {
  return {
    accessKeyId: id,
    balanceMode: "message_prepaid",
    status: "active",
    totalMessages: 10,
    remainingMessages: 6,
    totalTokens: null,
    remainingTokens: null,
    unlimitedUntil: null,
    periodStartsAt: null,
    periodEndsAt: null,
    updatedAt: "2026-10-01T00:00:00Z",
  };
}

describe("API Key editing and copying", () => {
  it("copies the actual key repeatedly after reopening without storing plaintext", async () => {
    const { api } = fixture([key("not-the-token")]);
    const user = userEvent.setup();
    const view = renderWithProviders(<Harness api={api} />);
    await screen.findByText("not-the-token");
    expect(
      screen.queryByRole("button", { name: "复制 ID" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "复制 Key" }));
    expect(await navigator.clipboard.readText()).toBe("sk-gw-fixture-secret");
    view.unmount();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("not-the-token");
    await user.click(screen.getByRole("button", { name: "复制 Key" }));
    expect(api.copyAccessKey).toHaveBeenCalledTimes(2);
    expect(api.copyAccessKey).toHaveBeenLastCalledWith(
      "management-fixture",
      "not-the-token",
    );
    expect(document.body.textContent).not.toContain("sk-gw-fixture-secret");
    expect(JSON.stringify(window.localStorage)).not.toContain(
      "sk-gw-fixture-secret",
    );
    expect(JSON.stringify(window.sessionStorage)).not.toContain(
      "sk-gw-fixture-secret",
    );
  });

  it("shows a copy error and does not copy a late response after leaving", async () => {
    const { api } = fixture([key("legacy")]);
    const user = userEvent.setup();
    vi.mocked(api.copyAccessKey!).mockRejectedValueOnce(
      new Error("旧密钥尚未保存可复制原文"),
    );
    const view = renderWithProviders(<Harness api={api} />);
    await screen.findByText("legacy");
    await user.click(screen.getByRole("button", { name: "复制 Key" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "旧密钥尚未保存可复制原文",
    );
    const pending = deferred<{ token: string }>();
    vi.mocked(api.copyAccessKey!).mockReturnValueOnce(pending.promise);
    await navigator.clipboard.writeText("unchanged");
    await user.click(screen.getByRole("button", { name: "复制 Key" }));
    view.unmount();
    await act(async () => pending.resolve({ token: "must-not-copy" }));
    expect(await navigator.clipboard.readText()).toBe("unchanged");
  });

  it("creates a limited key in the same request and rejects incomplete allowance", async () => {
    const { api } = fixture();
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    const dialog = await openCreate(user);
    await user.type(
      within(dialog).getByRole("textbox", { name: "名称" }),
      "limited",
    );
    await user.selectOptions(
      within(dialog).getByRole("combobox", { name: "额度限制" }),
      "token_prepaid",
    );
    const create = within(dialog).getByRole("button", { name: "创建 API Key" });
    expect(create).toBeDisabled();
    await user.type(
      within(dialog).getByRole("spinbutton", { name: "总 Token 额度" }),
      "12000",
    );
    await user.click(create);
    await screen.findByRole("dialog", { name: "API Key 已生成" });
    expect(api.createAccessKey).toHaveBeenCalledWith(
      "management-fixture",
      expect.objectContaining({
        quota: { mode: "token_prepaid", limit: 12000 },
        metadata: { accountGroupIds: ["group-a"] },
      }),
    );
  });

  it("edits name, expiry, multiple groups and total allowance in the shared secondary dialog", async () => {
    const original = key("editable", {
      metadata: { accountGroupIds: ["group-a"], models: ["model-a"] },
    });
    const { api, catalog } = fixture([original]);
    catalog.balances = [balance(original.id)];
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("editable");
    const trigger = screen.getByRole("button", { name: "编辑" });
    await user.click(trigger);
    const dialog = screen.getByRole("dialog", { name: "编辑 API Key" });
    expect(within(dialog).getByRole("textbox", { name: "名称" })).toHaveFocus();
    expect(within(dialog).getByText("已用 4 · 剩余 6")).toBeInTheDocument();
    expect(
      within(dialog).getByRole("textbox", { name: "项目 ID" }),
    ).toBeDisabled();
    await user.clear(within(dialog).getByRole("textbox", { name: "名称" }));
    await user.type(
      within(dialog).getByRole("textbox", { name: "名称" }),
      "renamed",
    );
    await user.click(within(dialog).getByRole("checkbox", { name: /高级组/ }));
    await user.selectOptions(
      within(dialog).getByRole("combobox", { name: "有效期" }),
      "7",
    );
    await user.clear(
      within(dialog).getByRole("spinbutton", { name: "总请求次数" }),
    );
    await user.type(
      within(dialog).getByRole("spinbutton", { name: "总请求次数" }),
      "20",
    );
    await user.click(within(dialog).getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(trigger).toHaveFocus();
    expect(api.updateAccessKey).toHaveBeenLastCalledWith(
      "management-fixture",
      "editable",
      expect.objectContaining({
        displayName: "renamed",
        quota: { mode: "message_prepaid", limit: 20 },
        metadata: {
          models: ["model-a"],
          accountGroupIds: ["group-a", "group-b"],
        },
      }),
    );
  });

  it("omits untouched quota and preserves time-pass restrictions", () => {
    const original = key("edit", {
      metadata: { accountGroupIds: ["group-a"] },
    });
    const { catalog } = fixture([original]);
    catalog.balances = [balance(original.id)];
    const draft = editAccessKeyDraft(original, catalog);
    draft.displayName = "renamed";
    expect(
      editedAccessKeyInput(original, draft, catalog).quota,
    ).toBeUndefined();
    catalog.balances[0].balanceMode = "time_pass";
    expect(editAccessKeyDraft(original, catalog).quotaMode).toBe("unchanged");
    expect(
      keyQuotaInput(editAccessKeyDraft(original, catalog)),
    ).toBeUndefined();
  });

  it("validates nonnegative safe integer allowances without rounding", () => {
    for (const quotaLimit of [
      "",
      "-1",
      "1.5",
      "1e3",
      "9007199254740992",
      "NaN",
    ])
      expect(validKeyQuota({ quotaMode: "token_prepaid", quotaLimit })).toBe(
        false,
      );
    expect(
      keyQuotaInput({ quotaMode: "message_prepaid", quotaLimit: "0" }),
    ).toEqual({ mode: "message_prepaid", limit: 0 });
    expect(
      keyQuotaInput({
        quotaMode: "token_prepaid",
        quotaLimit: "9007199254740991",
      })?.limit,
    ).toBe(Number.MAX_SAFE_INTEGER);
  });
});
