import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { deferred, renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { ManagementSecuritySettings } from "./ManagementSecuritySettings";

const api = vi.hoisted(() => ({ list: vi.fn(), setSlot: vi.fn() }));
vi.mock("./managementKeysApi", () => ({ managementKeysApi: () => api }));
const first = { id: "initial", name: "Initial", createdAt: "", current: true, enabled: true };
const second = { id: "second", name: "Second", createdAt: "", current: false, enabled: false };
beforeEach(() => { vi.resetAllMocks(); localStorage.clear(); api.list.mockResolvedValue({ keys: [first], slotsSupported: true }); api.setSlot.mockResolvedValue({ success: true }); });
async function ready() { await waitFor(() => expect(screen.getByLabelText("管理密钥一", { exact: true })).toBeEnabled()); }

it("renders exactly two inline rows without the previous panel or dialogs", async () => {
  renderWithProviders(<ManagementSecuritySettings />); await ready();
  expect(document.querySelectorAll('.nt-management-key-slots .nt-setting-row')).toHaveLength(2);
  expect(screen.queryByRole("heading")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /添加|删除|编辑/ })).not.toBeInTheDocument();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.getByLabelText("管理密钥一", { exact: true })).toHaveAttribute("type", "password");
  expect(screen.getByRole("switch", { name: "管理密钥一 启用" })).toBeChecked();
  expect(screen.getByRole("switch", { name: "管理密钥二 启用" })).toBeDisabled();
});

it("disabling the current final key reinitializes into the backend's passwordless mode", async () => {
  const retryInitialization = vi.fn();
  renderWithProviders(<ManagementSecuritySettings />, { retryInitialization }); await ready();
  await userEvent.setup().click(screen.getByRole("switch", { name: "管理密钥一 启用" }));
  expect(api.setSlot).toHaveBeenCalledWith("management-secret", 0, false, undefined);
  await waitFor(() => expect(retryInitialization).toHaveBeenCalledOnce());
});

it("enables a second slot from its inline draft without persisting the secret in browser storage", async () => {
  renderWithProviders(<ManagementSecuritySettings />); await ready();
  const user = userEvent.setup();
  await user.type(screen.getByLabelText("管理密钥二", { exact: true }), "second-secret");
  await user.click(screen.getByRole("switch", { name: "管理密钥二 启用" }));
  expect(api.setSlot).toHaveBeenCalledExactlyOnceWith("management-secret", 1, true, "second-secret");
  await waitFor(() => expect(screen.getByLabelText("管理密钥二", { exact: true })).toHaveValue(""));
  expect(JSON.stringify(localStorage)).not.toContain("second-secret");
});

it("keeps confirmed state and the retryable draft when a save fails", async () => {
  api.setSlot.mockRejectedValue(new Error("private-server-detail"));
  renderWithProviders(<ManagementSecuritySettings />); await ready();
  const user = userEvent.setup();
  await user.type(screen.getByLabelText("管理密钥二", { exact: true }), "retry-secret{Enter}");
  expect(await screen.findByRole("alert")).not.toHaveTextContent("private-server-detail");
  expect(screen.getByLabelText("管理密钥二", { exact: true })).toHaveValue("retry-secret");
  expect(screen.getByRole("switch", { name: "管理密钥二 启用" })).not.toBeChecked();
});

it("locks navigation while editing or saving and ignores completion after unmount", async () => {
  const pending = deferred<unknown>(); api.setSlot.mockReturnValue(pending.promise);
  const onInteractionChange = vi.fn(); const login = vi.fn();
  const { unmount } = renderWithProviders(<ManagementSecuritySettings onInteractionChange={onInteractionChange} />, { login }); await ready();
  const user = userEvent.setup();
  await user.type(screen.getByLabelText("管理密钥一", { exact: true }), "replacement");
  expect(onInteractionChange).toHaveBeenLastCalledWith(true);
  fireEvent.keyDown(screen.getByLabelText("管理密钥一", { exact: true }), { key: "Enter" });
  await waitFor(() => expect(screen.getByRole("switch", { name: "管理密钥一 启用" })).toBeDisabled());
  unmount(); await act(async () => pending.resolve({ success: true }));
  expect(login).not.toHaveBeenCalled();
  expect(onInteractionChange).toHaveBeenLastCalledWith(false);
});

it("retains disabled key slots and never silently truncates a legacy ring", async () => {
  api.list.mockResolvedValue({ keys: [first, second, { ...second, id: "third" }], slotsSupported: true });
  renderWithProviders(<ManagementSecuritySettings />);
  expect(await screen.findByRole("alert")).toHaveTextContent("超过两把历史密钥");
  expect(screen.getByRole("switch", { name: "管理密钥一 启用" })).toBeDisabled();
  expect(api.setSlot).not.toHaveBeenCalled();
});

it("does not simulate enable support on an older backend", async () => {
  api.list.mockResolvedValue({ keys: [first] });
  renderWithProviders(<ManagementSecuritySettings />);
  expect(await screen.findByText("启用设置需要更新网关后端。")).toBeInTheDocument();
  expect(screen.getByRole("switch", { name: "管理密钥一 启用" })).toBeDisabled();
});
