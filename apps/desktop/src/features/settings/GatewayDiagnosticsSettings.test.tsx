import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createBrowserHost } from "../../platform/browserHost";
import { HostProvider } from "../../platform/HostProvider";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { deferred, renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { ConsoleSettingsWorkspace } from "./ConsoleSettingsWorkspace";
import { GatewayDiagnosticsSettings } from "./GatewayDiagnosticsSettings";
import { downloadGatewayExport } from "./diagnosticDownload";

const api = vi.hoisted(() => ({ load: vi.fn(), save: vi.fn(), export: vi.fn() }));
vi.mock("./gatewayDiagnosticsApi", () => ({ gatewayDiagnosticsApi: () => ({ ...api }) }));
vi.mock("../../session/useManagementSession", () => ({ useManagementSession: () => ({ phase: "authenticated", managementToken: "management-secret" }) }));
vi.mock("./diagnosticDownload", () => ({ downloadGatewayExport: vi.fn() }));
const settings = { logDirectory: "/server-b/logs", retentionDays: 7, logLimitMb: 100 };
beforeEach(() => { vi.resetAllMocks(); localStorage.clear(); api.load.mockResolvedValue(settings); });
afterEach(() => { delete window.__GATEWAY_WINDOW_CHROME__; });

it.each([false, true])("uses the same backend settings and downloads in desktop=%s", async desktop => {
  const showSettings = vi.fn();
  if (desktop) window.__GATEWAY_WINDOW_CHROME__ = { send: vi.fn(), showSettings };
  api.export.mockResolvedValue({ fileName: "gateway-logs.log", mimeType: "text/plain;charset=utf-8", content: "server-b log", truncated: false });
  renderWithProviders(<ConsoleSettingsWorkspace />);
  const user = userEvent.setup();
  await user.click(screen.getByRole("tab", { name: "诊断与日志" }));
  const directory = screen.getByRole("textbox", { name: "日志目录" });
  await waitFor(() => expect(directory).toHaveValue("/server-b/logs"));
  expect(screen.queryByText(/日志策略在此 Gateway 下次启动时生效/)).not.toBeInTheDocument();
  expect(showSettings).not.toHaveBeenCalled();
  expect(api.load).toHaveBeenCalledWith("management-secret", expect.any(AbortSignal));
  api.save.mockResolvedValue({ ...settings, retentionDays: 30 });
  await user.selectOptions(screen.getByRole("combobox", { name: "日志保留时间" }), "30");
  expect(api.save).toHaveBeenCalledWith("management-secret", { retentionDays: 30, logLimitMb: 100 }, expect.any(AbortSignal));
  await waitFor(() => expect(screen.getByRole("combobox", { name: "日志保留时间" })).toHaveValue("30"));
  const logsRow = screen.getByText("运行日志").closest(".nt-setting-row") as HTMLElement;
  await user.click(within(logsRow).getByRole("button", { name: "导出" }));
  expect(api.export).toHaveBeenCalledWith("management-secret", "logs", expect.any(AbortSignal));
  expect(downloadGatewayExport).toHaveBeenCalledWith(expect.objectContaining({ content: "server-b log" }));
});

it("does not download a stale response after a backend switch", async () => {
  const pending = deferred<unknown>(); api.export.mockReturnValue(pending.promise);
  const base = createBrowserHost();
  const first = { ...base, apiOrigin: "https://a.example" };
  const second = { ...base, apiOrigin: "https://b.example" };
  function Switcher() {
    const [host, setHost] = useState(first);
    return <><button onClick={() => setHost(second)}>Switch backend</button><HostProvider adapter={host}><GatewayDiagnosticsSettings /></HostProvider></>;
  }
  render(<UiLocaleProvider><Switcher /></UiLocaleProvider>);
  await waitFor(() => expect(screen.getAllByRole("button", { name: "导出" })[0]).toBeEnabled());
  const user = userEvent.setup();
  await user.click(screen.getAllByRole("button", { name: "导出" })[1]);
  await user.click(screen.getByRole("button", { name: "Switch backend" }));
  await act(async () => pending.resolve({ fileName: "gateway-logs.log", content: "old-server", truncated: false }));
  expect(downloadGatewayExport).not.toHaveBeenCalled();
});

it("retains confirmed policy and hides private server errors when saving fails", async () => {
  api.save.mockRejectedValue(new Error("secret-private-path"));
  renderWithProviders(<GatewayDiagnosticsSettings />);
  const select = screen.getByRole("combobox", { name: "日志保留时间" });
  await waitFor(() => expect(select).toBeEnabled());
  await userEvent.setup().selectOptions(select, "30");
  expect(await screen.findByRole("alert")).not.toHaveTextContent("secret-private-path");
  expect(select).toHaveValue("7");
});

it("aborts pending work on unmount and blocks duplicate downloads", async () => {
  const pending = deferred<unknown>(); api.export.mockReturnValue(pending.promise);
  const { unmount } = renderWithProviders(<GatewayDiagnosticsSettings />);
  await waitFor(() => expect(screen.getAllByRole("button", { name: "导出" })[0]).toBeEnabled());
  const user = userEvent.setup();
  await user.click(screen.getAllByRole("button", { name: "导出" })[1]);
  expect(screen.getAllByRole("button", { name: "导出" })[0]).toBeDisabled();
  const signal = api.export.mock.calls[0][2] as AbortSignal;
  unmount(); expect(signal.aborted).toBe(true);
  await act(async () => pending.resolve({ fileName: "gateway-logs.log", content: "late", truncated: false }));
  expect(downloadGatewayExport).not.toHaveBeenCalled();
});
