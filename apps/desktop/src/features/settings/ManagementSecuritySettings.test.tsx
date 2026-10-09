import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { deferred, renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { ManagementSecuritySettings } from "./ManagementSecuritySettings";

const api = vi.hoisted(() => ({ list: vi.fn(), add: vi.fn(), revoke: vi.fn(), edit: vi.fn(), reveal: vi.fn() }));
vi.mock("./managementKeysApi", () => ({ managementKeysApi: () => api }));
const initial = { id: "initial", name: "Initial", createdAt: "", current: true };
const secondary = { id: "second", name: "Second", createdAt: "", current: false };
const bootstrapStatus = { needsBootstrap: false, managementConfigured: true, environmentOverride: true };
async function setup(keys = [initial]) {
  api.list.mockResolvedValue({ keys });
  const logout = vi.fn().mockResolvedValue(undefined);
  const view = renderWithProviders(<ManagementSecuritySettings />, { bootstrapStatus, logout });
  await screen.findByText("Initial");
  return { ...view, logout, user: userEvent.setup() };
}
async function addDraft(user: ReturnType<typeof userEvent.setup>, token = "new-secret") {
  await user.click(screen.getByRole("button", { name: "添加" }));
  await user.type(screen.getByLabelText("密钥名称"), "Second");
  await user.type(screen.getByLabelText("管理密钥", { exact: true }), token);
}
describe("ManagementSecuritySettings key list", () => {
  beforeEach(() => {
    vi.resetAllMocks(); localStorage.clear(); sessionStorage.clear();
    for (const method of [api.add, api.revoke, api.edit]) method.mockResolvedValue({ success: true });
    api.reveal.mockResolvedValue({ token: "revealed-secret" });
  });

  it("renders only rows with eye, delete, edit and a separate Add button", async () => {
    await setup([initial, secondary]);
    expect(screen.queryByRole("form")).not.toBeInTheDocument();
    const rows = screen.getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    expect(within(rows[0]).getAllByRole("button").map(button => button.getAttribute("aria-label")))
      .toEqual(["显示 Initial 的密钥", "删除 Initial", "编辑 Initial"]);
    expect(screen.getByLabelText("Initial 的管理密钥")).toHaveAttribute("type", "password");
  });

  it("creates one draft row on Add, prevents duplicate submissions and clears secrets after save", async () => {
    const pending = deferred<unknown>(); api.add.mockReturnValue(pending.promise);
    const { user } = await setup();
    await addDraft(user);
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "添加" })).toBeDisabled();
    const submit = screen.getByRole("button", { name: "保存" });
    await user.click(submit); fireEvent.click(submit);
    expect(api.add).toHaveBeenCalledExactlyOnceWith("management-secret", "Second", "new-secret");
    expect(submit).toBeDisabled();
    api.list.mockResolvedValue({ keys: [initial, secondary] });
    await act(async () => pending.resolve({ success: true }));
    await waitFor(() => expect(screen.queryByRole("form")).not.toBeInTheDocument());
    expect(screen.queryByText("管理密钥已保存。")).not.toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(JSON.stringify(localStorage)).not.toContain("new-secret");
    expect(document.body.innerHTML).not.toContain("new-secret");
  });

  it("supports input visibility, Escape and cancelling without creating a key", async () => {
    const { user } = await setup(); await addDraft(user);
    await user.click(screen.getByRole("button", { name: "显示输入的密钥" }));
    expect(screen.getByLabelText("管理密钥", { exact: true })).toHaveAttribute("type", "text");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("form")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "添加" })).toHaveFocus();
    expect(api.add).not.toHaveBeenCalled();
  });

  it("guards the last key and requires confirmation before deletion", async () => {
    const { user, rerenderWithProviders } = await setup();
    expect(screen.getByRole("button", { name: "删除 Initial" })).toBeDisabled();
    api.list.mockResolvedValue({ keys: [initial, secondary] });
    rerenderWithProviders({ managementToken: "another-session", bootstrapStatus });
    await waitFor(() => expect(screen.getByRole("button", { name: "删除 Initial" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "删除 Second" }));
    const dialog = screen.getByRole("alertdialog", { name: "确认删除管理密钥" });
    expect(dialog).toHaveTextContent("确定删除管理密钥“Second”吗？");
    expect(within(dialog).getByRole("button", { name: "取消" })).toHaveFocus();
    expect(api.revoke).not.toHaveBeenCalled();
    api.list.mockResolvedValue({ keys: [initial] });
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    expect(api.revoke).toHaveBeenCalledExactlyOnceWith("another-session", "second");
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(screen.queryByText("Second")).not.toBeInTheDocument();
    expect(screen.queryByText("管理密钥已删除。")).not.toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("cancels the deletion dialog with Escape and restores the row trigger", async () => {
    const { user } = await setup([initial, secondary]);
    const trigger = screen.getByRole("button", { name: "删除 Second" });
    await user.click(trigger);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(api.revoke).not.toHaveBeenCalled();
    expect(trigger).toHaveFocus();
    await user.click(trigger);
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(api.revoke).not.toHaveBeenCalled();
  });

  it("keeps pending deletion modal and shows failure inside it without success labels", async () => {
    const pending = deferred<unknown>(); api.revoke.mockReturnValue(pending.promise);
    const { user } = await setup([initial, secondary]);
    await user.click(screen.getByRole("button", { name: "删除 Second" }));
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    await user.keyboard("{Escape}");
    const dialog = screen.getByRole("alertdialog");
    expect(within(dialog).getByRole("button", { name: "取消" })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "确认删除" })).toBeDisabled();
    await act(async () => pending.reject(new Error("secret-in-server-error")));
    expect(within(dialog).getByRole("alert")).toHaveTextContent("未能确认");
    expect(dialog).not.toHaveTextContent("secret-in-server-error");
    expect(api.revoke).toHaveBeenCalledOnce();
    expect(screen.queryByText("管理密钥已删除。")).not.toBeInTheDocument();
  });

  it("signs out after confirmed deletion of the current key", async () => {
    const { user, logout } = await setup([initial, secondary]);
    await user.click(screen.getByRole("button", { name: "删除 Initial" }));
    expect(screen.getByText(/删除当前密钥后将退出登录/)).toBeVisible();
    await user.click(screen.getByRole("button", { name: "确认删除" }));
    expect(logout).toHaveBeenCalledOnce();
  });

  it("edits a name in place without replacing the stored key", async () => {
    const { user, logout } = await setup();
    await user.click(screen.getByRole("button", { name: "编辑 Initial" }));
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    expect(screen.getByLabelText("密钥名称")).toHaveValue("Initial");
    await user.clear(screen.getByLabelText("密钥名称")); await user.type(screen.getByLabelText("密钥名称"), "Renamed");
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(api.edit).toHaveBeenCalledExactlyOnceWith("management-secret", "initial", "Renamed", undefined);
    expect(logout).not.toHaveBeenCalled();
  });

  it("signs out after replacing the current key", async () => {
    const { user, logout } = await setup();
    await user.click(screen.getByRole("button", { name: "编辑 Initial" }));
    await user.type(screen.getByLabelText("管理密钥", { exact: true }), "replacement");
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(api.edit).toHaveBeenCalledExactlyOnceWith("management-secret", "initial", "Initial", "replacement");
    expect(logout).toHaveBeenCalledOnce();
  });

  it("reveals on demand and removes plaintext when hidden or the window loses focus", async () => {
    const { user } = await setup();
    expect(api.reveal).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "显示 Initial 的密钥" }));
    expect(screen.getByLabelText("Initial 的管理密钥")).toHaveValue("revealed-secret");
    await user.click(screen.getByRole("button", { name: "隐藏 Initial 的密钥" }));
    expect(document.body.innerHTML).not.toContain("revealed-secret");
    await user.click(screen.getByRole("button", { name: "显示 Initial 的密钥" }));
    fireEvent.blur(window);
    expect(document.body.innerHTML).not.toContain("revealed-secret");
    expect(JSON.stringify(localStorage)).not.toContain("revealed-secret");
  });

  it("does not reveal an old request after a session switch", async () => {
    const pending = deferred<{ token: string }>(); api.reveal.mockReturnValue(pending.promise);
    const { user, rerenderWithProviders } = await setup();
    await user.click(screen.getByRole("button", { name: "显示 Initial 的密钥" }));
    rerenderWithProviders({ managementToken: "changed-session", bootstrapStatus });
    await act(async () => pending.resolve({ token: "stale-secret" }));
    expect(document.body.innerHTML).not.toContain("stale-secret");
  });

  it("explains legacy nonrecoverable keys without pretending to reveal them", async () => {
    api.reveal.mockResolvedValue({ token: null });
    const { user } = await setup();
    await user.click(screen.getByRole("button", { name: "显示 Initial 的密钥" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("通过编辑重新设置");
    expect(screen.getByLabelText("Initial 的管理密钥")).toHaveAttribute("type", "password");
  });

  it.each(["中文密钥", "with spaces"])("rejects invalid key %s", async token => {
    const { user } = await setup(); await addDraft(user, token);
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
    expect(api.add).not.toHaveBeenCalled();
  });

  it("does not echo secrets from errors", async () => {
    api.add.mockRejectedValue(new Error("sensitive-secret"));
    const { user } = await setup(); await addDraft(user);
    await user.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("未能确认");
    expect(document.body.textContent).not.toContain("sensitive-secret");
  });
});
