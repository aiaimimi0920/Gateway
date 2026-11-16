import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import type { GrokAuthSession } from "../../api/console/grok-auth";
import { GrokOAuthPanel } from "./GrokOAuthPanel";
import type { GrokOAuthOperations } from "./grokOAuthOperations";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/grok-auth", () => ({ createGrokAuthApi: () => api }));
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const session = (status: GrokAuthSession["status"]): GrokAuthSession => ({ id: "login", providerId: "build", expectedRevision: "r1", status,
  authorizationUrl: "https://auth.x.ai/oauth2/authorize?state=synthetic", identityVerified: false,
  credentialId: status === "succeeded" ? "account" : null, targetCredentialId: null,
  savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const props = { client, managementToken: "operator", secretGrant: "grant", providerId: "build", expectedRevision: "r1",
  operations: new Map() as GrokOAuthOperations,
  disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
const poll = async () => { await act(async () => { await vi.advanceTimersByTimeAsync(1500); }); };
function fill() {
  fireEvent.change(screen.getByLabelText("OAuth Client ID"), { target: { value: "permitted-client" } });
  fireEvent.change(screen.getByLabelText("Scopes（空格分隔）"), { target: { value: "openid offline_access" } });
  fireEvent.change(screen.getByLabelText("Redirect URI"), { target: { value: "http://127.0.0.1:34567/callback" } });
}
beforeEach(() => { props.operations.clear(); vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session("waiting_user")); api.get.mockResolvedValue(session("ready")); api.act.mockResolvedValue(session("cancelled")); });
afterEach(() => vi.useRealTimers());

it("requires explicit client parameters, opens only the exact authorization URL and uses a manual callback", async () => {
  render(<GrokOAuthPanel {...props} />);
  expect(screen.getByLabelText("OAuth Client ID")).toHaveValue("");
  expect(screen.getByRole("button", { name: "通过 Grok Build 登录" })).toBeDisabled();
  fill(); await click("通过 Grok Build 登录");
  expect(api.create).toHaveBeenCalledWith(expect.objectContaining({ providerId: "build", expectedRevision: "r1", clientId: "permitted-client", scopes: ["openid", "offline_access"] }), "grant");
  expect(screen.getByRole("link")).toHaveAttribute("href", session("waiting_user").authorizationUrl);
  fireEvent.change(screen.getByLabelText("完整回调地址"), { target: { value: "private-callback" } });
  api.act.mockResolvedValueOnce(session("ready")); await click("完成授权");
  expect(api.act).toHaveBeenCalledWith("login", "complete", expect.objectContaining({ expectedRevision: "r1" }), "grant", "private-callback");
  expect(props.onSaved).not.toHaveBeenCalled();
});

it("keeps consent selection independent from required principal", async () => {
  render(<GrokOAuthPanel {...props} />); fill();
  fireEvent.change(screen.getByLabelText("Consent 预选 ID（仅提示）"), { target: { value: "team-hint" } });
  fireEvent.change(screen.getByLabelText("强制匹配类型"), { target: { value: "User" } });
  fireEvent.change(screen.getByLabelText("强制匹配 ID（独立约束）"), { target: { value: "required-user" } });
  await click("通过 Grok Build 登录");
  expect(api.create).toHaveBeenCalledWith(expect.objectContaining({ principalSelection: { kind: "Team", id: "team-hint" }, requiredPrincipal: { kind: "User", id: "required-user" } }), "grant");
});

it("does not save on ready, and refreshes exactly once only after confirmed success", async () => {
  const view = render(<GrokOAuthPanel {...props} />); fill(); await click("通过 Grok Build 登录"); await poll();
  expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(session("importing")); await click("保存到凭证池");
  expect(api.act).toHaveBeenCalledWith("login", "import", expect.objectContaining({ expectedRevision: "r1" }), "grant", undefined);
  expect(props.onSaved).not.toHaveBeenCalled();
  view.rerender(<GrokOAuthPanel {...props} expectedRevision="r2" disabled />);
  api.get.mockResolvedValue(session("succeeded")); await poll(); await poll();
  expect(props.onSaved).toHaveBeenCalledOnce();
  view.unmount(); expect(api.act).toHaveBeenCalledTimes(1);
});

it("recovers a lost save response by polling without ever re-submitting", async () => {
  render(<GrokOAuthPanel {...props} />); fill(); await click("通过 Grok Build 登录"); await poll();
  api.act.mockRejectedValueOnce(new Error("lost")); await click("保存到凭证池");
  expect(screen.queryByRole("button", { name: "保存到凭证池" })).not.toBeInTheDocument();
  expect(props.onSaved).not.toHaveBeenCalled();
  api.get.mockResolvedValue(session("succeeded")); await poll();
  expect(props.onSaved).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

it("leaves unconfirmed saves explicit and never reports or retries success", async () => {
  const view = render(<GrokOAuthPanel {...props} />); fill(); await click("通过 Grok Build 登录"); await poll();
  api.act.mockResolvedValueOnce(session("save_unconfirmed")); await click("保存到凭证池"); await poll();
  expect(screen.getByRole("status")).toHaveTextContent("保存结果未确认");
  expect(props.onSaved).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "通过 Grok Build 登录" })).toBeDisabled();
  view.unmount(); expect(api.act).toHaveBeenCalledTimes(1);
});

it("renews secret access for complete and cancel as well as create/import", async () => {
  const view = render(<GrokOAuthPanel {...props} />); fill(); await click("通过 Grok Build 登录");
  view.rerender(<GrokOAuthPanel {...props} secretGrant={null} />);
  fireEvent.change(screen.getByLabelText("完整回调地址"), { target: { value: "original-callback" } });
  await click("完成授权"); expect(api.act).not.toHaveBeenCalled(); expect(props.onRequestSecretAccess).toHaveBeenCalledOnce();
  api.act.mockResolvedValueOnce(session("ready"));
  view.rerender(<GrokOAuthPanel {...props} secretGrant="new-grant" />);
  await act(async () => {});
  expect(api.act).toHaveBeenCalledWith("login", "complete", expect.objectContaining({ expectedRevision: "r1" }), "new-grant", "original-callback");
  api.act.mockClear(); view.rerender(<GrokOAuthPanel {...props} secretGrant={null} />);
  await click("取消登录"); expect(api.act).not.toHaveBeenCalled();
  view.rerender(<GrokOAuthPanel {...props} secretGrant="cancel-grant" />); await act(async () => {});
  expect(api.act).toHaveBeenCalledWith("login", "cancel", expect.objectContaining({ expectedRevision: "r1" }), "cancel-grant", undefined);
});

it("resumes an explicit import after a server-confirmed grant rejection only", async () => {
  const view = render(<GrokOAuthPanel {...props} />); fill(); await click("通过 Grok Build 登录"); await poll();
  api.act.mockRejectedValueOnce(new GatewayApiError("Grant expired", 403, "console_secret_access_required"));
  await click("保存到凭证池"); expect(props.onRequestSecretAccess).toHaveBeenCalledOnce();
  api.act.mockResolvedValueOnce(session("succeeded")); view.rerender(<GrokOAuthPanel {...props} secretGrant="renewed" />); await act(async () => {});
  expect(props.onSaved).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledTimes(2);
});

it("blocks complete and import after revision changes, but cancels with the session's original revision", async () => {
  const view = render(<GrokOAuthPanel {...props} />); fill(); await click("通过 Grok Build 登录"); await poll();
  view.rerender(<GrokOAuthPanel {...props} expectedRevision="r2" />);
  expect(screen.getByRole("button", { name: "保存到凭证池" })).toBeDisabled();
  await click("取消登录");
  expect(api.act).toHaveBeenCalledWith("login", "cancel", expect.objectContaining({ expectedRevision: "r1" }), "grant", undefined);
});

it("retains an ambiguous save across closing and reopening the credential dialog", async () => {
  const shared = { ...props, operations: new Map() };
  const view = render(<GrokOAuthPanel {...shared} />); fill(); await click("通过 Grok Build 登录"); await poll();
  api.act.mockRejectedValueOnce(new Error("lost")); await click("保存到凭证池"); view.unmount();
  render(<GrokOAuthPanel {...shared} />);
  expect(screen.getByRole("button", { name: "通过 Grok Build 登录" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "保存到凭证池" })).not.toBeInTheDocument();
  api.get.mockResolvedValue(session("succeeded")); await poll();
  expect(props.onSaved).toHaveBeenCalledOnce(); expect(api.create).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledTimes(1);
});

it("keeps a ready receipt usable after an explicit pre-admission rejection without automatically retrying", async () => {
  render(<GrokOAuthPanel {...props} />); fill(); await click("通过 Grok Build 登录"); await poll();
  api.act.mockRejectedValueOnce(new GatewayApiError("Rejected", 400, "grok_login_save_rejected")); await click("保存到凭证池");
  expect(screen.getByRole("button", { name: "保存到凭证池" })).toBeEnabled();
  expect(screen.getByRole("alert")).toHaveTextContent("保存前校验被拒绝");
  await poll(); expect(api.act).toHaveBeenCalledOnce(); expect(props.onSaved).not.toHaveBeenCalled();
});
