import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import { pilotAccount, pilotSection, renderWorkspace, workspaceProps } from "./AccountsLedgerWorkspace.fixtures";
import { providerAccessKind, providerAccessLabel } from "./providerAccessKind";

it("shows the directory and keeps provider/account statistics independent", async () => {
  const user = userEvent.setup();
  renderWorkspace(workspaceProps({ pilotSections: [pilotSection({ directAccounts: [pilotAccount({ requestCount: 17 }), pilotAccount({ accountId: "two", displayName: "Account 2", requestCount: 29 })] })] }));
  await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /Managed OpenAI/ }));
  expect(within(screen.getByRole("region", { name: "Managed OpenAI 统计" })).getByText("46")).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Account 1" }));
  expect(within(screen.getByRole("region", { name: "Account 1 统计" })).getByText("17")).toBeVisible();
  expect(screen.queryByText("46")).not.toBeInTheDocument();
  await user.selectOptions(screen.getByRole("combobox", { name: "切换账号" }), "managed-provider:two");
  expect(within(screen.getByRole("region", { name: "Account 2 统计" })).getByText("29")).toBeVisible();
  await user.click(screen.getByRole("button", { name: "← 返回账号列表" }));
  expect(screen.getByRole("button", { name: "Account 1" })).toBeVisible();
});
it("preserves provider actions and locked dispatch", async () => {
  const user = userEvent.setup(); const props = workspaceProps({ editorLocked: true });
  renderWorkspace(props);
  await user.click(within(screen.getByRole("navigation")).getByRole("button", { name: /Managed OpenAI/ }));
  expect(screen.getByRole("button", { name: "添加账号" })).toBeDisabled();
  expect(screen.getByRole("switch", { name: "服务商调度" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "服务商设置" }));
  expect(screen.getByRole("dialog", { name: "服务商设置" })).toBeVisible();
});
it("classifies custom compatibility metadata without renaming legacy labels", () => {
  expect(providerAccessKind({ adapter: "openai_compatible" })).toBe("custom");
  expect(providerAccessKind({ adapter: "openai_compatible", providerPreset: "openai" })).toBe("api");
  expect(providerAccessLabel({ adapter: "openai_compatible", providerLabel: "relay-api" })).toBe("relay-api");
});
