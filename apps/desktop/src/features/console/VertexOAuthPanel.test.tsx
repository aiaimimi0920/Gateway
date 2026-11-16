import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import type { VertexAuthSession } from "../../api/console/vertex-auth";
import { GatewayApiError } from "../../api/errors";
import { VertexOAuthPanel } from "./VertexOAuthPanel";
import type { VertexOAuthProps } from "./useVertexOAuth";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/vertex-auth", () => ({ createVertexAuthApi: () => api }));
const session = (status: VertexAuthSession["status"]): VertexAuthSession => ({ id: "login", providerId: "vertex", expectedRevision: "r1", status,
  authorizationUrl: "https://accounts.google.com/o/oauth2/v2/auth?state=synthetic", redirectUri: "http://127.0.0.1:43567/",
  credentialId: status === "succeeded" ? "account" : null, targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const props = (): VertexOAuthProps => ({ client: {} as GatewayApiClient, managementToken: "owner", secretGrant: "grant", providerId: "vertex", expectedRevision: "r1",
  target: { project: "project-a", location: "global" }, operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() });
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
const poll = async () => { await act(async () => { await vi.advanceTimersByTimeAsync(1500); }); };
const change = (label: string, value: string) => fireEvent.change(screen.getByLabelText(label), { target: { value } });
const start = async () => { change("Client ID", "explicit-client"); await click("通过 Vertex 登录"); };
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session("waiting_user")); api.get.mockResolvedValue(session("ready")); api.act.mockResolvedValue(session("cancelled")); });
afterEach(() => vi.useRealTimers());

it("requires an explicit client, captures optional private material and shows the actual listener", async () => {
  const original = props(); render(<VertexOAuthPanel {...original} />);
  expect(screen.getByLabelText("Client ID")).toHaveValue(""); expect(screen.getByRole("button", { name: "通过 Vertex 登录" })).toBeDisabled();
  expect(screen.getByLabelText("Client Secret（可选）")).toHaveAttribute("type", "password");
  change("Client ID", "explicit-client"); change("Client Secret（可选）", "private-secret"); change("Quota Project（可选）", "quota-a");
  change("Project", "ignored-retarget"); await click("通过 Vertex 登录");
  expect(api.create).toHaveBeenCalledExactlyOnceWith({ providerId: "vertex", expectedRevision: "r1", credentialId: undefined,
    clientId: "explicit-client", clientSecret: "private-secret", target: { project: "project-a", location: "global", quota_project: "quota-a" } }, "grant");
  expect(screen.queryByLabelText("Client Secret（可选）")).not.toBeInTheDocument();
  expect(screen.getByRole("link")).toHaveAttribute("href", session("waiting_user").authorizationUrl);
  expect(screen.getByLabelText("Redirect URI")).toHaveValue("http://127.0.0.1:43567/"); expect(original.operations.size).toBe(0);
});
it.each([" bad", "bad ", "设备", "x".repeat(1025)])("rejects invalid clients without trimming or borrowing a default: %j", async (clientId) => {
  render(<VertexOAuthPanel {...props()} />); change("Client ID", clientId);
  expect(screen.getByRole("button", { name: "通过 Vertex 登录" })).toBeDisabled(); expect(api.create).not.toHaveBeenCalled();
});
it("keeps the stable credential and its quota fixed, and omits an empty optional client secret", async () => {
  render(<VertexOAuthPanel {...props()} credentialId="stable" target={{ project: "project-a", location: "global", quota_project: "saved-quota" }} />);
  expect(screen.getByLabelText("重新授权目标账号")).toHaveValue("stable");
  expect(screen.getByLabelText("Quota Project（可选）")).toHaveAttribute("readonly"); change("Quota Project（可选）", "ignored-quota"); await start();
  expect(api.create.mock.calls[0][0]).toMatchObject({ credentialId: "stable", target: { quota_project: "saved-quota" } });
  expect(api.create.mock.calls[0][0]).not.toHaveProperty("clientSecret");
});
it("does not auto-save on automatic callback and confirms one accepted save across a revision change", async () => {
  const original = props(); const view = render(<VertexOAuthPanel {...original} />); await start(); await poll();
  expect(api.act).not.toHaveBeenCalled(); expect(original.onSaved).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(session("importing")); await click("保存到凭证池");
  view.rerender(<VertexOAuthPanel {...original} expectedRevision="r2" disabled />);
  api.get.mockResolvedValue(session("succeeded")); await poll(); await poll();
  expect(original.onSaved).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledTimes(1);
});
it("clears the manual callback while resuming only that explicit operation with a renewed grant", async () => {
  const original = props(); const view = render(<VertexOAuthPanel {...original} />); await start();
  view.rerender(<VertexOAuthPanel {...original} secretGrant={null} />);
  change("手动回调地址", "http://127.0.0.1:43567/?code=private"); await click("提交手动回调");
  expect(screen.getByLabelText("手动回调地址")).toHaveValue(""); expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(session("ready")); view.rerender(<VertexOAuthPanel {...original} secretGrant="renewed" />); await act(async () => {});
  expect(api.act).toHaveBeenCalledExactlyOnceWith("login", "complete", expect.objectContaining({ expectedRevision: "r1" }), "renewed", "http://127.0.0.1:43567/?code=private");
});
it("does not rebase a private pending create and retains no client material in operation receipts", async () => {
  const original = { ...props(), secretGrant: null }; const view = render(<VertexOAuthPanel {...original} />);
  change("Client Secret（可选）", "private-secret"); await start();
  expect(screen.getByLabelText("Client Secret（可选）")).toHaveValue(""); expect(original.onRequestSecretAccess).toHaveBeenCalledOnce();
  view.rerender(<VertexOAuthPanel {...original} expectedRevision="r2" secretGrant="renewed" />); await act(async () => {});
  expect(api.create).not.toHaveBeenCalled(); expect(original.operations.size).toBe(0);
});
it("queries an unknown accepted save after closing and reopening, without new login or import", async () => {
  const original = props(); const first = render(<VertexOAuthPanel {...original} />); await start(); await poll();
  api.act.mockRejectedValueOnce(new Error("lost")); await click("保存到凭证池"); first.unmount(); render(<VertexOAuthPanel {...original} />);
  expect(screen.queryByLabelText("Quota Project（可选）")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存到凭证池" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "通过 Vertex 登录" })).not.toBeInTheDocument();
  api.get.mockResolvedValue(session("succeeded")); await poll(); expect(original.onSaved).toHaveBeenCalledOnce();
  expect(api.create).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledTimes(1);
});
it("keeps save_unconfirmed terminal, does not cancel it on close and never enables a retry", async () => {
  const original = props(); const view = render(<VertexOAuthPanel {...original} />); await start(); await poll();
  api.act.mockResolvedValueOnce(session("save_unconfirmed")); await click("保存到凭证池"); await poll();
  expect(screen.getByText(/保存结果未确认/)).toBeInTheDocument(); expect(screen.queryByRole("button", { name: "重新开始授权" })).not.toBeInTheDocument();
  view.unmount(); expect(api.act).toHaveBeenCalledTimes(1); expect(original.onSaved).not.toHaveBeenCalled();
});
it("allows restarting a reclaimed unsaved session, but not after any unknown save", async () => {
  const original = props(); render(<VertexOAuthPanel {...original} />); await start();
  api.get.mockRejectedValue(new GatewayApiError("vertex_login_unavailable", 400, "vertex_login_rejected")); await poll();
  expect(screen.getByText(/授权会话已不可用/)).toBeInTheDocument(); expect(screen.getByRole("button", { name: "通过 Vertex 登录" })).toBeEnabled();
});
