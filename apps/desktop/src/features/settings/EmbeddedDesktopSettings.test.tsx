import { invoke } from "@tauri-apps/api/core";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { UiThemeProvider } from "../../theme/UiThemeProvider";
import type { EmbeddedSettingsContext } from "../../platform/desktopSettings";
import { EmbeddedDesktopSettings } from "./EmbeddedDesktopSettings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const defaults = { closeBehavior: "exit", retentionDays: 0, logLimitMb: 0, autostart: false, autostartSupported: true } as const;
function setup(category: EmbeddedSettingsContext["category"], locale: "zh-CN" | "en-US" = "zh-CN") {
  return render(<UiThemeProvider><UiLocaleProvider><EmbeddedDesktopSettings context={{ category, theme: "dark", locale, current: "https://gateway.example", revision: 1 }} /></UiLocaleProvider></UiThemeProvider>);
}
beforeEach(() => {
  vi.resetAllMocks(); localStorage.clear();
  vi.mocked(invoke).mockImplementation(async command => command === "load_gateway_desktop_preferences" ? defaults
    : command === "load_gateway_connection" ? { mode: "existing", serverUrl: "https://gateway.example" } : undefined);
});

it("renders desktop controls directly, with no second shell, tabs or connection attempt", async () => {
  setup("desktop");
  await waitFor(() => expect(screen.getByRole("combobox", { name: "开机启动" })).toBeEnabled());
  expect(screen.getByRole("combobox", { name: "关闭窗口时" })).toHaveValue("exit");
  expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
  expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "connect_gateway")).toBe(false);
});

it("writes preferences through the trusted child and locks the parent's categories during a save", async () => {
  setup("desktop");
  const select = screen.getByRole("combobox", { name: "开机启动" });
  await waitFor(() => expect(select).toBeEnabled());
  let resolve!: (value: unknown) => void;
  vi.mocked(invoke).mockImplementation(async command => command === "save_gateway_desktop_preferences" ? new Promise(done => { resolve = done; }) : undefined);
  await userEvent.setup().selectOptions(select, "on");
  expect(invoke).toHaveBeenCalledWith("set_gateway_settings_busy", { busy: true, revision: 1 });
  expect(select).toBeDisabled();
  await act(async () => resolve({ ...defaults, autostart: true }));
  expect(select).toHaveValue("on");
  expect(invoke).toHaveBeenLastCalledWith("set_gateway_settings_busy", { busy: false, revision: 1 });
});

it("keeps only native desktop behavior in the trusted child, without backend log controls", async () => {
  setup("desktop");
  await waitFor(() => expect(screen.getByRole("combobox", { name: "开机启动" })).toBeEnabled());
  expect(screen.queryByRole("combobox", { name: "日志保留时间" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /^打开$/ })).not.toBeInTheDocument();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "open_gateway_log_directory")).toBe(false);
});

it("opens backend settings without applying anything, and retains the page and active address on failure", async () => {
  setup("backend");
  const test = screen.getByRole("button", { name: "测试连接" });
  await waitFor(() => expect(test).toBeEnabled());
  expect(screen.getByRole("textbox", { name: "后端地址" })).toHaveValue("https://gateway.example");
  expect(screen.getByRole("option", { name: "本机Gateway" })).toBeInTheDocument();
  expect(screen.getByRole("option", { name: "远程Gateway" })).toBeInTheDocument();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "connect_gateway")).toBe(false);
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === "test_gateway_connection" || command === "connect_gateway") throw Error("private-native-error");
  });
  const user = userEvent.setup();
  await user.click(test);
  expect(await screen.findByRole("alert")).not.toHaveTextContent("private-native-error");
  await user.click(screen.getByRole("button", { name: "应用并连接" }));
  expect(screen.getByText("https://gateway.example", { exact: true })).toBeVisible();
  expect(test).toBeEnabled();
});

it("inherits the visible console's locale and theme", async () => {
  setup("desktop", "en-US");
  expect(await screen.findByRole("combobox", { name: "On close" })).toBeVisible();
  expect(document.documentElement.dataset.ntTheme).toBe("dark");
});

it("updates a retained view without reloading its document and ignores stale revisions", async () => {
  setup("backend");
  await waitFor(() => expect(screen.getByRole("button", { name: "测试连接" })).toBeEnabled());
  await userEvent.setup().selectOptions(screen.getByRole("combobox", { name: "连接方式" }), "local");
  expect(screen.getByRole("textbox", { name: "后端地址" })).toHaveValue("http://127.0.0.1:4200");
  act(() => window.dispatchEvent(new CustomEvent("gateway:settings-context", { detail: { category: "desktop", theme: "light", locale: "en-US", current: "https://gateway.example", revision: 2 } })));
  expect(await screen.findByRole("combobox", { name: "On close" })).toBeVisible();
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("gateway_settings_ready", { revision: 2 }));
  expect(document.documentElement.dataset.ntTheme).toBe("light");
  act(() => window.dispatchEvent(new CustomEvent("gateway:settings-context", { detail: { category: "backend", theme: "dark", locale: "zh-CN", current: "https://gateway.example", revision: 1 } })));
  expect(screen.getByRole("combobox", { name: "On close" })).toBeVisible();
  expect(screen.queryByRole("combobox", { name: "连接方式" })).not.toBeInTheDocument();
});
