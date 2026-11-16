import { act, renderHook, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { UiThemeProvider } from "../../theme/UiThemeProvider";
import { renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { ConsoleSettingsWorkspace } from "./ConsoleSettingsWorkspace";
import { SettingsHeaderSlot, SettingsNavigationProvider } from "./SettingsNavigation";
import { SettingsSearch } from "./SettingsSearch";
import { initializeDisplayPreferences, useDisplayPreferences } from "./displayPreferences";
import { UnavailableSetting } from "./UnavailableSettings";
import { settingsCatalog } from "./settingsCatalog";

beforeEach(() => { localStorage.clear(); document.documentElement.style.cssText = ""; });
function setup() {
  return renderWithProviders(<UiThemeProvider><SettingsNavigationProvider>
    <SettingsSearch /><SettingsHeaderSlot /><ConsoleSettingsWorkspace />
  </SettingsNavigationProvider></UiThemeProvider>);
}
it("persists text size and font style locally and resets style for the system font", async () => {
  const user = userEvent.setup(); setup();
  await user.click(screen.getByRole("radio", { name: "150%" }));
  expect(document.documentElement.style.fontSize).toBe("24px");
  await user.selectOptions(screen.getByRole("combobox", { name: "界面字体" }), "segoe");
  await user.selectOptions(screen.getByRole("combobox", { name: "界面字体 样式" }), "700");
  expect(document.documentElement.style.getPropertyValue("--nt-ui-weight")).toBe("700");
  await user.selectOptions(screen.getByRole("combobox", { name: "界面字体" }), "system");
  expect(screen.getByRole("combobox", { name: "界面字体 样式" })).toBeDisabled();
  expect(document.documentElement.style.getPropertyValue("--nt-ui-weight")).toBe("400");
  expect(JSON.parse(localStorage.getItem("gateway-ui-display")!)).toMatchObject({ size: 150, uiFont: "system", uiWeight: "400" });
});
it("locates a static setting across categories by keyboard, handles no results and Escape", async () => {
  const user = userEvent.setup(); setup();
  const search = screen.getByRole("combobox", { name: "查找设置" });
  await user.type(search, "自动更新");
  expect(screen.getByRole("option", { name: "自动更新" })).toBeVisible();
  await user.keyboard("{Enter}");
  await waitFor(() => expect(document.querySelector('[data-setting-id="auto-update"]')).toHaveFocus());
  expect(screen.getByRole("tab", { name: "关于" })).toHaveAttribute("aria-selected", "true");
  await user.type(search, "demo-secret-never-index");
  expect(screen.getByRole("status")).toHaveTextContent("没有找到相关设置");
  await user.keyboard("{Escape}");
  expect(search).toHaveValue("");
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
});
it("enables display naming but keeps native sleep controls unavailable in a browser", () => {
  setup();
  for (const [id, labels] of [
    ["provider-name", ["关闭", "自定义名称不带供应商", "开启"]],
    ["awake", ["关闭", "开启", "屏幕也常亮"]],
  ] as const) {
    const row = within(document.querySelector<HTMLElement>(`[data-setting-id="${id}"]`)!);
    expect(row.queryByRole("combobox")).not.toBeInTheDocument();
    expect(row.getAllByRole("radio")).toHaveLength(3);
    for (const label of labels) {
      if (id === "awake") expect(row.getByRole("radio", { name: label })).toBeDisabled();
      else expect(row.getByRole("radio", { name: label })).toBeEnabled();
    }
  }
});
it("removes the privacy tab, all of its settings and their search entries", async () => {
  const user = userEvent.setup(); setup();
  expect(screen.queryByRole("tab", { name: "隐私" })).not.toBeInTheDocument();
  for (const id of ["privacy-model", "privacy-status", "redaction-records", "redact-keys", "redact-pii", "redact-words", "redact-case", "redact-whole", "redact-rules", "redact-policy", "redact-exceptions", "redact-test", "mask-accounts", "user-statistics", "share-models"]) {
    expect(settingsCatalog.some(item => item.id === id)).toBe(false);
  }
  const search = screen.getByRole("combobox", { name: "查找设置" });
  for (const query of ["隐私", "脱敏", "账号打码", "计入用户数", "分享供应商和模型"]) {
    await user.clear(search); await user.type(search, query);
    expect(screen.getByRole("status")).toHaveTextContent("没有找到相关设置");
  }
});
it("removes unused telemetry controls and their search entries", async () => {
  const user = userEvent.setup(); setup();
  const search = screen.getByRole("combobox", { name: "查找设置" });
  for (const title of ["采集服务连接", "导出范围", "链路内容"]) {
    expect(screen.queryByText(title)).not.toBeInTheDocument();
    await user.clear(search); await user.type(search, title);
    expect(screen.getByRole("status")).toHaveTextContent("没有找到相关设置");
  }
  expect(settingsCatalog.filter(item => item.category === "diagnostics").map(item => item.title[0])).toEqual([
    "日志保存位置", "日志保留天数", "日志占用上限", "导出诊断信息", "导出运行日志",
  ]);
});
it("uses linked standard tabs with keyboard navigation instead of a second manual focus loop", async () => {
  const user = userEvent.setup(); setup();
  await user.click(screen.getByRole("tab", { name: "常规" }));
  await user.keyboard("{End}");
  await waitFor(() => expect(screen.getByRole("tab", { name: "关于" })).toHaveFocus());
  const about = screen.getByRole("tab", { name: "关于" });
  const panel = screen.getByRole("tabpanel", { name: "关于" });
  expect(about).toHaveAttribute("aria-controls", panel.id);
  expect(panel).toHaveAttribute("aria-labelledby", about.id);
  expect(about).toHaveClass("n-tab");
});
it("renders text-only secondary settings tabs without category icons", () => {
  setup();
  const tabs = within(screen.getByRole("tablist", { name: "设置分类" })).getAllByRole("tab");
  expect(tabs.map(tab => tab.textContent)).toEqual(["常规", "网络与共享", "可观测性", "同步与备份", "关于"]);
  for (const tab of tabs) expect(tab.querySelector("svg")).toBeNull();
});
it("uses disabled dropdowns for numerous or long alternatives without inventing a selected setting", () => {
  renderWithProviders(<UnavailableSetting item={{ id: "test-many-options", category: "about", title: ["测试设置", "Test setting"], choices: [["一", "One"], ["二", "Two"], ["三", "Three"], ["四", "Four"], ["五", "Five"]] }} />);
  for (const name of ["测试设置"]) {
    const select = screen.getByRole("combobox", { name });
    expect(select).toHaveClass("n-select"); expect(select).toBeDisabled(); expect(select).toHaveValue("");
  }
  expect(screen.queryByRole("radio")).not.toBeInTheDocument();
});
it("validates stored display preferences and ignores injected font strings", () => {
  localStorage.setItem("gateway-ui-display", JSON.stringify({ size: 9999, uiFont: 'url(https://example.com)', codeFont: "__proto__", uiWeight: "700" }));
  initializeDisplayPreferences();
  expect(document.documentElement.style.fontSize).toBe("16px");
  expect(document.documentElement.style.getPropertyValue("--nt-ui-font")).not.toContain("url(");
  expect(document.documentElement.style.getPropertyValue("--nt-ui-weight")).toBe("400");
});
it("handles unavailable storage without an unstable snapshot or false save", () => {
  const read = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw Error("blocked"); });
  const write = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw Error("blocked"); });
  const { result } = renderHook(useDisplayPreferences);
  act(() => expect(result.current.update({ size: 150 })).toBe(false));
  expect(result.current.value.size).toBe(100);
  read.mockRestore(); write.mockRestore();
});
