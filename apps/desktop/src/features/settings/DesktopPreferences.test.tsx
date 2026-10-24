import { invoke } from "@tauri-apps/api/core";
import { act, renderHook, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { DesktopBehaviorSettings } from "./DesktopBehaviorSettings";
import { useDesktopPreferences } from "./useDesktopPreferences";
import { render } from "@testing-library/react";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const defaults = { closeBehavior: "exit", retentionDays: 0, logLimitMb: 0, autostart: false, autostartSupported: true } as const;
beforeEach(() => { vi.resetAllMocks(); localStorage.clear(); });

it("persists actual native preferences and does not send unrelated fields", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(defaults).mockResolvedValueOnce({ ...defaults, closeBehavior: "tray" });
  const { result } = renderHook(useDesktopPreferences);
  await waitFor(() => expect(result.current.value).toEqual(defaults));
  await act(() => result.current.update({ closeBehavior: "tray" }));
  expect(invoke).toHaveBeenLastCalledWith("save_gateway_desktop_preferences", {
    settings: { closeBehavior: "tray", retentionDays: 0, logLimitMb: 0 }, autostart: false,
  });
  expect(result.current.value?.closeBehavior).toBe("tray");
});
it("retains the last confirmed values if native save fails", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(defaults).mockRejectedValueOnce(new Error("private-system-detail"));
  function TestSettings() { return <DesktopBehaviorSettings preferences={useDesktopPreferences()} />; }
  render(<UiLocaleProvider><TestSettings /></UiLocaleProvider>);
  const user = userEvent.setup();
  const select = screen.getByRole("combobox", { name: "关闭窗口时" });
  await waitFor(() => expect(select).toBeEnabled());
  await user.selectOptions(select, "tray");
  expect(await screen.findByRole("alert")).not.toHaveTextContent("private-system-detail");
  expect(select).toHaveValue("exit");
  expect(screen.getByRole("combobox", { name: "开机启动" })).toHaveValue("off");
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
