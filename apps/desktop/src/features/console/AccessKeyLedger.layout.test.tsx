import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { fixture, Harness, key } from "./AccessKeysSection.fixtures";
import { renderWithProviders } from "./BrowserConsoleApp.render-fixture";
import { formatRecentKeyUse } from "./accessWorkspaceFormatting";

it("separates key columns and keeps actions beside their values", async () => {
  const { api, catalog } = fixture([key("codex", { ownerId: "hidden-owner", lastUsedAt: "2026-10-24T00:14:49" })]);
  catalog.cashQuotaSupported = true;
  renderWithProviders(<Harness api={api} />);
  const name = await screen.findByText("codex");
  const row = within(name.closest("tr")!);
  expect(screen.getAllByRole("columnheader").map((node) => node.textContent)).toEqual([
    "名称", "API Key", "权益组", "剩余额度", "有效期", "最近使用", "状态", "编辑",
  ]);
  const cells = row.getAllByRole("cell");
  expect(cells[0]).toHaveTextContent("codex");
  expect(screen.queryByText("hidden-owner")).not.toBeInTheDocument();
  expect(within(cells[1]).getByRole("button", { name: "复制 Key" })).toBeVisible();
  expect(within(cells[1]).getByRole("button", { name: "轮转" })).toBeVisible();
  expect(within(cells[3]).getByRole("button", { name: "账单" })).toBeVisible();
  expect(cells[5]).toHaveTextContent("10/24 00:14");
  expect(within(cells[6]).getByRole("button", { name: "停用" })).toBeVisible();
  expect(within(cells[6]).getByRole("button", { name: "删除" })).toBeVisible();
  expect(within(cells[7]).getByRole("button", { name: "编辑" })).toBeVisible();
});

it("hides local attribution inputs while retaining local creation defaults", async () => {
  const { api } = fixture();
  const user = userEvent.setup();
  renderWithProviders(<Harness api={api} />);
  await screen.findByText("暂无 API Key");
  await user.click(screen.getByRole("button", { name: "新建 Key" }));
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
