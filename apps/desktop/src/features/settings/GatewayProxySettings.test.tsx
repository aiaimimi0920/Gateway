import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { deferred, renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { GatewayProxySettings } from "./GatewayProxySettings";

const api = vi.hoisted(() => ({ load: vi.fn(), save: vi.fn(), test: vi.fn() }));
vi.mock("./gatewayProxyApi", () => ({ gatewayProxyApi: () => api }));
vi.mock("../../session/useManagementSession", () => ({ useManagementSession: () => ({ phase: "authenticated", managementToken: "test-management" }) }));
const initial = { mode: "auto", url: "", effectiveMode: "auto", restartRequired: false };
beforeEach(() => { vi.resetAllMocks(); localStorage.clear(); api.load.mockResolvedValue(initial); });

it("tests the unsaved draft, then saves with explicit restart feedback", async () => {
  api.test.mockResolvedValue({ ok: true, latencyMs: 12 });
  api.save.mockResolvedValue({ ...initial, mode: "custom", url: "http://127.0.0.1:7890/", restartRequired: true });
  renderWithProviders(<GatewayProxySettings />);
  const user = userEvent.setup();
  await waitFor(() => expect(screen.getByRole("radio", { name: "自定义" })).toBeEnabled());
  await user.click(screen.getByRole("radio", { name: "自定义" }));
  await user.type(screen.getByRole("textbox", { name: "代理地址" }), "http://127.0.0.1:7890");
  await user.click(screen.getByRole("button", { name: "测试" }));
  expect(await screen.findByText("HTTPS 测试通过 · 12 ms")).toBeInTheDocument();
  expect(api.test).toHaveBeenCalledWith("test-management", { mode: "custom", url: "http://127.0.0.1:7890" }, expect.any(AbortSignal));
  expect(api.save).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "应用" }));
  expect(await screen.findByText("代理设置已保存，重启当前网关后生效。")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "应用" })).toBeDisabled();
});

it("does not pretend unavailable backend settings were loaded", async () => {
  api.load.mockRejectedValue(new Error("private-server-details"));
  renderWithProviders(<GatewayProxySettings />);
  expect(await screen.findByRole("alert")).not.toHaveTextContent("private-server-details");
  expect(screen.getByRole("button", { name: "测试" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "应用" })).toBeDisabled();
});

it("keeps a failed draft retryable and cancels a pending request on unmount", async () => {
  api.save.mockRejectedValue(new Error("private-details"));
  const { unmount } = renderWithProviders(<GatewayProxySettings />);
  await waitFor(() => expect(screen.getByRole("radio", { name: "关闭" })).toBeEnabled());
  const user = userEvent.setup();
  await user.click(screen.getByRole("radio", { name: "关闭" }));
  await user.click(screen.getByRole("button", { name: "应用" }));
  expect(await screen.findByRole("alert")).not.toHaveTextContent("private-details");
  expect(screen.getByRole("button", { name: "应用" })).toBeEnabled();
  const pending = deferred<{ ok: boolean; latencyMs: number }>();
  api.test.mockReturnValue(pending.promise);
  await user.click(screen.getByRole("button", { name: "测试" }));
  const signal = api.test.mock.calls[0][2] as AbortSignal;
  unmount();
  expect(signal.aborted).toBe(true);
  await act(async () => pending.resolve({ ok: true, latencyMs: 1 }));
});
