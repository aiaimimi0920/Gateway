import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { AccessKeyHeaderProvider, AccessKeyHeaderSlot, compareAccessKeys } from "./AccessKeyHeader";
import { fixture, Harness, key } from "./AccessKeysSection.fixtures";
import { renderWithProviders } from "./BrowserConsoleApp.render-fixture";

it("places key controls in shell slots, copies the host URL and restores create focus", async () => {
  const { api } = fixture([key("alpha"), key("beta")]);
  const user = userEvent.setup();
  renderWithProviders(<AccessKeyHeaderProvider>
    <header aria-label="Key page header"><AccessKeyHeaderSlot slot="leading" /><AccessKeyHeaderSlot slot="actions" /></header>
    <Harness api={api} />
  </AccessKeyHeaderProvider>);
  await screen.findByText("alpha");
  const header = within(screen.getByRole("banner", { name: "Key page header" }));
  expect(screen.queryByText("生效密钥")).not.toBeInTheDocument();
  expect(screen.queryByText("Base URL")).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "密钥状态" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "刷新密钥" })).not.toBeInTheDocument();
  expect(header.getByRole("combobox", { name: "密钥排序" })).toBeVisible();
  await user.type(header.getByRole("searchbox", { name: "搜索密钥" }), "beta");
  expect(screen.queryByText("alpha")).not.toBeInTheDocument();
  expect(screen.getByText("beta")).toBeVisible();
  await user.click(header.getByRole("button", { name: "清除密钥搜索" }));
  expect(screen.getByText("alpha")).toBeVisible();
  const copy = header.getByRole("button", { name: "复制" });
  await user.click(copy);
  expect(await navigator.clipboard.readText()).toBe("https://gateway.test/v1");
  expect(copy.querySelector("span:not(.nt-visually-hidden)")).toHaveTextContent(/^复制$/);
  expect(copy.querySelector(".lucide-check")).not.toBeNull();
  expect(copy.querySelector(".nt-visually-hidden")).toHaveTextContent("已复制");
  const create = header.getByRole("button", { name: "新建 Key" });
  await user.click(create);
  expect(screen.getByRole("dialog")).toBeVisible();
  await user.keyboard("{Escape}");
  expect(create).toHaveFocus();
});

it("sorts by creation time or public name without changing the source", () => {
  const keys = [key("beta", { createdAt: "2026-01-01T00:00:00Z" }), key("alpha", { createdAt: "2026-02-01T00:00:00Z" })];
  for (const [sort, first] of [["newest", "alpha"], ["oldest", "beta"], ["name", "alpha"], ["name-desc", "beta"]] as const) {
    expect([...keys].sort((a, b) => compareAccessKeys(a, b, sort))[0].displayName).toBe(first);
  }
  expect(keys[0].displayName).toBe("beta");
});
