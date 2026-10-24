import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { UiThemeProvider } from "../../theme/UiThemeProvider";
import { renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { ConsoleSettingsWorkspace } from "./ConsoleSettingsWorkspace";

beforeEach(() => { localStorage.clear(); });
afterEach(() => { delete window.__GATEWAY_WINDOW_CHROME__; });

it("uses compact selects and only shared browser settings categories", async () => {
  renderWithProviders(<UiThemeProvider><ConsoleSettingsWorkspace /></UiThemeProvider>);
  const user = userEvent.setup();
  expect(screen.getByRole("combobox", { name: "主题" })).toHaveValue("dark");
  await user.selectOptions(screen.getByRole("combobox", { name: "主题" }), "light");
  expect(localStorage.getItem("gateway-ui-theme")).toBe("light");
  expect(screen.getByRole("combobox", { name: "界面语言" })).toBeVisible();
  for (const name of ["界面偏好", "管理安全", "诊断与日志"]) {
    expect(screen.getByRole("tab", { name })).toBeVisible();
  }
  expect(screen.queryByRole("tab", { name: "后端连接" })).not.toBeInTheDocument();
  expect(screen.queryByRole("tab", { name: "桌面行为" })).not.toBeInTheDocument();
  expect(screen.queryByText("选择后自动保存")).not.toBeInTheDocument();
  expect(screen.queryByText("主题与语言")).not.toBeInTheDocument();
});

it("mounts native settings in the current content region without opening the separate settings window", async () => {
  const send = vi.fn();
  const showSettings = vi.fn();
  window.__GATEWAY_WINDOW_CHROME__ = { send, showSettings };
  const bounds = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ x: 210, y: 160, left: 210, top: 160, right: 710, bottom: 660, width: 500, height: 500, toJSON: () => ({}) });
  const { unmount } = renderWithProviders(<UiThemeProvider><ConsoleSettingsWorkspace /></UiThemeProvider>);
  const user = userEvent.setup();
  for (const [name, category] of [["桌面行为", "desktop"], ["后端连接", "backend"]]) {
    await user.click(screen.getByRole("tab", { name }));
    await waitFor(() => expect(showSettings).toHaveBeenLastCalledWith(expect.objectContaining({ category, x: 210, y: 160, width: 500, height: 500, theme: "dark", locale: "zh-CN" })));
    expect(screen.queryByRole("button", { name: "打开本机设置" })).not.toBeInTheDocument();
  }
  expect(send).not.toHaveBeenCalled();
  act(() => window.dispatchEvent(new CustomEvent("gateway:settings-busy", { detail: true })));
  expect(screen.getByRole("tab", { name: "界面偏好" })).toBeDisabled();
  act(() => window.dispatchEvent(new CustomEvent("gateway:settings-busy", { detail: false })));
  act(() => window.dispatchEvent(new Event("gateway:settings-ready")));
  expect(screen.queryByText("加载中…")).not.toBeInTheDocument();
  await user.click(screen.getByRole("tab", { name: "界面偏好" }));
  expect(send).toHaveBeenLastCalledWith("settings-hide");
  expect(send.mock.calls.every(([action]) => action === "settings-hide")).toBe(true);
  unmount(); bounds.mockRestore();
});
