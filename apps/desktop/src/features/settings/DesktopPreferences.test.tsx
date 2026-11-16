import { invoke } from "@tauri-apps/api/core";
import { act, renderHook, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { DesktopBehaviorSettings } from "./DesktopBehaviorSettings";
import { useDesktopPreferences } from "./useDesktopPreferences";
import { render } from "@testing-library/react";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const defaults = { closeBehavior: "exit", retentionDays: 0, logLimitMb: 0, autostart: false, autostartSupported: true, sleepMode: "off", sleepSupported: true } as const;
beforeEach(() => { vi.resetAllMocks(); localStorage.clear(); });

it("persists actual native preferences and does not send unrelated fields", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(defaults).mockResolvedValueOnce({ ...defaults, closeBehavior: "tray" });
  const { result } = renderHook(useDesktopPreferences);
  await waitFor(() => expect(result.current.value).toEqual(defaults));
  await act(() => result.current.update({ closeBehavior: "tray" }));
  expect(invoke).toHaveBeenLastCalledWith("save_gateway_desktop_preferences", {
    settings: { closeBehavior: "tray", sleepMode: "off", retentionDays: 0, logLimitMb: 0 }, autostart: false,
  });
  expect(result.current.value?.closeBehavior).toBe("tray");
});
it("retains the last confirmed values if native save fails", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(defaults).mockRejectedValueOnce(new Error("private-system-detail"));
  function TestSettings() { return <DesktopBehaviorSettings preferences={useDesktopPreferences()} />; }
  render(<UiLocaleProvider><TestSettings /></UiLocaleProvider>);
  const user = userEvent.setup();
  const select = screen.getByRole("radio", { name: "最小化到托盘" });
  await waitFor(() => expect(select).toBeEnabled());
  await user.click(select);
  expect(await screen.findByRole("alert")).not.toHaveTextContent("private-system-detail");
  expect(screen.getByRole("radio", { name: "退出程序" })).toBeChecked();
  for (const radio of screen.getAllByRole("radio", { name: "关闭" })) expect(radio).toBeChecked();
});
it("saves the native display-awake mode and confirms it from the backend", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(defaults).mockResolvedValueOnce({ ...defaults, sleepMode: "display" });
  const { result } = renderHook(useDesktopPreferences);
  await waitFor(() => expect(result.current.value).toEqual(defaults));
  await act(() => result.current.update({ sleepMode: "display" }));
  expect(invoke).toHaveBeenLastCalledWith("save_gateway_desktop_preferences", {
    settings: { closeBehavior: "exit", sleepMode: "display", retentionDays: 0, logLimitMb: 0 }, autostart: false,
  });
  expect(result.current.value?.sleepMode).toBe("display");
});
it("prevents duplicate native writes while a save is pending", async () => {
  let resolve!: (value: unknown) => void;
  vi.mocked(invoke).mockResolvedValueOnce(defaults).mockImplementationOnce(() => new Promise(done => { resolve = done; }));
  const { result } = renderHook(useDesktopPreferences);
  await waitFor(() => expect(result.current.value).toEqual(defaults));
  let first!: Promise<void>;
  act(() => { first = result.current.update({ autostart: true }); });
  await act(() => result.current.update({ closeBehavior: "tray" }));
  expect(invoke).toHaveBeenCalledTimes(2);
  await act(async () => { resolve({ ...defaults, autostart: true }); await first; });
  expect(result.current.value?.autostart).toBe(true);
});
