import { fireEvent, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "./BrowserConsoleApp.render-fixture";
import { fixture, Harness, key } from "./AccessKeysSection.fixtures";

describe("compact key editor", () => {
  it("removes redundant text without changing the name and retains one quota input across modes", async () => {
    const { api, catalog } = fixture([key("codex", { metadata: { accountGroupIds: ["group-a"] } })]);
    catalog.cashQuotaSupported = true;
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("codex");
    await user.click(screen.getByRole("button", { name: "编辑" }));
    const dialog = screen.getByRole("dialog", { name: "编辑 API Key" });
    const view = within(dialog);
    expect(view.getByRole("textbox", { name: "名称" })).toHaveValue("codex");
    expect(view.queryByText("codex")).not.toBeInTheDocument();
    expect(view.queryByText(/已选.*可多选|决定此密钥|不设密钥额度上限|请填写有效的总额度上限|填写累计总上限/)).not.toBeInTheDocument();
    const input = view.getByRole("spinbutton", { name: "额度上限" });
    expect(input).toBeDisabled();
    await user.click(view.getByRole("button", { name: "权益组" }));
    await user.keyboard("{Escape}");
    expect(dialog).toBeInTheDocument();
    expect(view.getByRole("button", { name: "权益组" })).toHaveAttribute("aria-expanded", "false");
    const quota = view.getByRole("combobox", { name: "额度限制" });
    const save = view.getByRole("button", { name: "保存" });
    for (const [mode, label] of [["message_prepaid", "总请求次数"], ["token_prepaid", "总 Token 额度"], ["cash_prepaid", "总花费（USD）"]]) {
      await user.selectOptions(quota, mode);
      expect(view.getByRole("spinbutton", { name: label })).toBe(input);
      expect(input).toBeEnabled();
      expect(save).toBeDisabled();
      expect(input).toHaveAttribute("aria-invalid", "true");
      fireEvent.submit(dialog.querySelector("form")!);
      expect(api.updateAccessKey).not.toHaveBeenCalled();
      await user.type(input, "12");
      expect(save).toBeEnabled();
      expect(input).toHaveAttribute("aria-invalid", "false");
    }
    await user.selectOptions(quota, "unlimited");
    expect(view.getByRole("spinbutton", { name: "额度上限" })).toBe(input);
    expect(input).toBeDisabled();
    expect(input).toHaveValue(null);
    expect(save).toBeEnabled();
    expect(view.queryByRole("status")).not.toBeInTheDocument();
  });

  it("keeps failure feedback and group validation without expanding hint rows", async () => {
    const { api } = fixture([key("codex", { metadata: { accountGroupIds: ["group-a"] } })]);
    vi.mocked(api.updateAccessKey!).mockRejectedValue(new Error("save rejected"));
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("codex");
    await user.click(screen.getByRole("button", { name: "编辑" }));
    const view = within(screen.getByRole("dialog"));
    const group = view.getByRole("button", { name: "权益组" });
    const save = view.getByRole("button", { name: "保存" });
    await user.click(group);
    await user.click(view.getByRole("menuitemcheckbox", { name: /基础组/ }));
    expect(group).toHaveTextContent("选择权益组");
    expect(save).toBeDisabled();
    await user.click(view.getByRole("menuitemcheckbox", { name: /高级组/ }));
    await user.click(save);
    expect(await view.findByRole("alert")).toHaveTextContent("save rejected");
    expect(group).toHaveTextContent("高级组");
    expect(view.queryByText(/请选择至少|请填写有效/)).not.toBeInTheDocument();
  });
});
