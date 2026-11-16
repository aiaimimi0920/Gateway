import { act, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { GatewayBackupSettings } from "./GatewayBackupSettings";

const mocks = vi.hoisted(() => ({ request: vi.fn(), save: vi.fn(), read: vi.fn() }));
vi.mock("../../session/useManagementSession", () => ({ useManagementSession: () => ({ managementToken: "fixture-token", secretGrant: { grant: "fixture-grant" }, phase: "authenticated" }) }));
vi.mock("./gatewayBackupApi", async original => ({ ...await original<typeof import("./gatewayBackupApi")>(), gatewayBackupApi: () => mocks.request, saveBackupFile: mocks.save, readBackupFile: mocks.read }));
beforeEach(() => {
  vi.clearAllMocks();
  mocks.request.mockImplementation(async (body: { action: string }) => {
    if (body.action === "status") return { destination: null, pending: false, revision: "revision-1" };
    if (body.action === "export") return { content: "encrypted-fixture", fileName: "gateway-backup.ngbk" };
    if (body.action === "preview") return { preview: { createdAt: "2026-11-15", providers: 2, accounts: 3, files: 10, replacesAccessKeysAndBalances: true }, revision: "revision-1" };
    if (body.action === "restore") return { pending: true };
    if (body.action === "cancelRestore") return { pending: false };
    return { saved: true };
  });
  mocks.read.mockResolvedValue("encrypted-fixture");
});
async function setup() {
  const view = renderWithProviders(<GatewayBackupSettings />);
  await waitFor(() => expect(screen.getByRole("button", { name: "导出…" })).toBeEnabled());
  return view;
}

it("requires a matching password before exporting and downloads only the confirmed ciphertext", async () => {
  const user = userEvent.setup(); await setup();
  await user.click(screen.getByRole("button", { name: "导出…" }));
  await user.type(screen.getByLabelText("备份密码", { exact: true }), "backup-password");
  expect(screen.getByRole("button", { name: "导出加密备份" })).toBeDisabled();
  await user.type(screen.getByLabelText("确认密码", { exact: true }), "backup-password");
  await user.click(screen.getByRole("button", { name: "导出加密备份" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledWith("encrypted-fixture"));
  expect(mocks.request).toHaveBeenCalledWith({ action: "export", password: "backup-password" }, "fixture-token", "fixture-grant", expect.any(AbortSignal));
  expect(screen.queryByLabelText("备份密码", { exact: true })).not.toBeInTheDocument();
});

it("previews an imported file before explicit replacement and can cancel a staged restore", async () => {
  const user = userEvent.setup(); await setup();
  await user.upload(screen.getByLabelText("选择加密备份"), new File(["cipher"], "backup.ngbk"));
  await user.type(await screen.findByLabelText("备份密码", { exact: true }), "backup-password");
  await user.click(screen.getByRole("button", { name: "校验备份" }));
  await screen.findByText(/2 个服务商 · 3 个账号/);
  expect(mocks.request.mock.calls.some(([body]) => body.action === "restore")).toBe(false);
  await user.click(screen.getByRole("button", { name: "确认覆盖并准备恢复" }));
  await screen.findByRole("button", { name: "取消待恢复" });
  expect(screen.getByRole("button", { name: "导出…" })).toBeDisabled();
  expect(mocks.request).toHaveBeenCalledWith(expect.objectContaining({ action: "restore", confirmed: true, expectedRevision: "revision-1" }), "fixture-token", "fixture-grant", expect.any(AbortSignal));
  await user.click(screen.getByRole("button", { name: "取消待恢复" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "导出…" })).toBeEnabled());
});

it("saves the selected sync provider and its credentials only through the protected backend", async () => {
  const user = userEvent.setup(); await setup();
  await user.click(screen.getByRole("radio", { name: "GitHub" }));
  for (const [label, value] of [["仓库所有者", "fixture"], ["仓库名称", "backup"], ["备份文件路径", "gateway.ngbk"], ["访问令牌", "synthetic-token"]]) {
    await user.type(screen.getByLabelText(label, { exact: true }), value);
  }
  await user.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText("同步设置已保存。");
  expect(mocks.request).toHaveBeenCalledWith(expect.objectContaining({ action: "saveDestination", destination: expect.objectContaining({ kind: "github", token: "synthetic-token", branch: "main" }) }), "fixture-token", "fixture-grant", expect.any(AbortSignal));
});

it("aborts on unmount and never downloads a stale export response", async () => {
  const user = userEvent.setup(); const view = await setup();
  let resolve: (value: unknown) => void = () => {};
  mocks.request.mockImplementation(() => new Promise(done => { resolve = done; }));
  await user.click(screen.getByRole("button", { name: "导出…" }));
  await user.type(screen.getByLabelText("备份密码", { exact: true }), "backup-password");
  await user.type(screen.getByLabelText("确认密码", { exact: true }), "backup-password");
  await user.click(screen.getByRole("button", { name: "导出加密备份" }));
  const signal = mocks.request.mock.lastCall?.[3] as AbortSignal;
  view.unmount(); expect(signal.aborted).toBe(true);
  await act(async () => resolve({ content: "stale" }));
  expect(mocks.save).not.toHaveBeenCalled();
});
