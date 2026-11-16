import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { fixture, Harness, key } from "./AccessKeysSection.fixtures";
import { renderWithProviders } from "./BrowserConsoleApp.render-fixture";
import { formatRecentKeyUse } from "./accessWorkspaceFormatting";

it("groups metadata into six columns and moves secondary actions into a menu", async () => {
  const user = userEvent.setup();
  const { api, catalog } = fixture([key("codex", { ownerId: "hidden-owner", lastUsedAt: "2026-10-24T00:14:49" })]);
  catalog.cashQuotaSupported = true;
  renderWithProviders(<Harness api={api} />);
  const name = await screen.findByText("codex");
  const row = within(name.closest("tr")!);
  expect(screen.getAllByRole("columnheader").map((node) => node.textContent)).toEqual([
    "密钥", "权益组", "剩余额度", "使用情况", "状态", "操作",
  ]);
  const cells = row.getAllByRole("cell");
  expect(cells[0]).toHaveTextContent("codex");
  expect(screen.queryByText("hidden-owner")).not.toBeInTheDocument();
  expect(within(cells[0]).getByRole("button", { name: "复制 Key" })).toBeVisible();
  expect(cells[3]).toHaveTextContent("10/24 00:14");
  expect(cells[3]).toHaveTextContent("有效期：永久");
  expect(within(cells[5]).getByRole("button", { name: "编辑" })).toBeVisible();
  expect(row.queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
  await user.click(row.getByRole("button", { name: "更多操作" }));
  for (const name of ["轮转", "账单", "停用", "删除"]) expect(screen.getByRole("menuitem", { name })).toBeVisible();
  await user.keyboard("{Escape}");
  expect(row.getByRole("button", { name: "更多操作" })).toHaveFocus();
});

it("hides local attribution inputs while retaining local creation defaults", async () => {
  const { api } = fixture();
  const user = userEvent.setup();
  renderWithProviders(<Harness api={api} />);
  await screen.findByText("暂无 API Key");
  await user.click(screen.getByRole("button", { name: "新建密钥" }));
  const dialog = within(screen.getByRole("dialog"));
  for (const name of ["归属类型", "归属 ID", "项目 ID", "租户 ID", "密钥前缀"]) {
    expect(dialog.queryByRole("textbox", { name })).not.toBeInTheDocument();
  }
  expect(dialog.queryByText("高级设置")).not.toBeInTheDocument();
});

it("formats activity in local month/day hour:minute and handles invalid dates", () => {
  expect(formatRecentKeyUse("2026-01-02T03:04:59")).toBe("01/02 03:04");
  expect(formatRecentKeyUse("invalid")).toBe("—");
});
