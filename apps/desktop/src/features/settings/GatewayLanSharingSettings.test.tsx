import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { deferred, renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { GatewayLanSharingSettings } from "./GatewayLanSharingSettings";
import { settingsCatalog, matchesSetting } from "./settingsCatalog";

const api = vi.hoisted(() => ({ load: vi.fn(), save: vi.fn() }));
vi.mock("./gatewayLanSharingApi", () => ({ gatewayLanSharingApi: () => api }));
beforeEach(() => { vi.resetAllMocks(); localStorage.clear(); api.load.mockResolvedValue({ enabled: true }); });

it("shows backend default on and only updates after a confirmed save", async () => {
  const pending = deferred<{ enabled: boolean }>(); api.save.mockReturnValue(pending.promise);
  renderWithProviders(<GatewayLanSharingSettings />);
  const on = screen.getByRole("radio", { name: "开启" });
  const off = screen.getByRole("radio", { name: "关闭" });
  await waitFor(() => expect(on).toBeChecked());
  await userEvent.setup().click(off);
  expect(api.save).toHaveBeenCalledWith("management-secret", false, expect.any(AbortSignal));
  expect(on).toBeChecked(); expect(off).toBeDisabled();
  await act(async () => pending.resolve({ enabled: false }));
  expect(off).toBeChecked(); expect(off).toBeEnabled();
  expect(localStorage.getItem("gateway-lan-sharing")).toBeNull();
});

it("keeps the confirmed selection on failure and hides private diagnostics", async () => {
  api.save.mockRejectedValue(new Error("private-server-path"));
  renderWithProviders(<GatewayLanSharingSettings />);
  await waitFor(() => expect(screen.getByRole("radio", { name: "开启" })).toBeChecked());
  await userEvent.setup().click(screen.getByRole("radio", { name: "关闭" }));
  expect(await screen.findByRole("alert")).not.toHaveTextContent("private-server-path");
  expect(screen.getByRole("radio", { name: "开启" })).toBeChecked();
});

it("does not invent enabled state when the backend API is unavailable", async () => {
  api.load.mockRejectedValue(new Error("404"));
  renderWithProviders(<GatewayLanSharingSettings />);
  expect(await screen.findByRole("alert")).toHaveTextContent("无法读取局域网共享设置");
  for (const radio of screen.getAllByRole("radio")) { expect(radio).not.toBeChecked(); expect(radio).toBeDisabled(); }
});

it("locks navigation during save, aborts on unmount, and ignores late results", async () => {
  const pending = deferred<{ enabled: boolean }>(); api.save.mockReturnValue(pending.promise);
  const lock = vi.fn(); const { unmount } = renderWithProviders(<GatewayLanSharingSettings onInteractionChange={lock} />);
  await waitFor(() => expect(screen.getByRole("radio", { name: "关闭" })).toBeEnabled());
  await userEvent.setup().click(screen.getByRole("radio", { name: "关闭" }));
  expect(lock).toHaveBeenLastCalledWith(true);
  const signal = api.save.mock.calls[0][2] as AbortSignal;
  unmount(); expect(signal.aborted).toBe(true); expect(lock).toHaveBeenLastCalledWith(false);
  await act(async () => pending.resolve({ enabled: false }));
});

it("removes the four retired settings from the search catalog without removing model display preferences", () => {
  for (const id of ["bare-model-id", "busy-models", "origins", "browser-access"]) expect(settingsCatalog.some(item => item.id === id)).toBe(false);
  for (const query of ["不带供应商的模型", "允许调用的网页", "繁忙的模型靠后", "在浏览器中访问"]) expect(settingsCatalog.some(item => matchesSetting(item, query))).toBe(false);
  expect(settingsCatalog.some(item => item.id === "provider-name")).toBe(true);
});
