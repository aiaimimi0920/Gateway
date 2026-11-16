import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { DimAuthSession } from "../../api/console/dim-auth";
import { GatewayApiError } from "../../api/errors";
import { DimOAuthPanel } from "./DimOAuthPanel";
import type { DimOAuthProps } from "./useDimOAuth";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/dim-auth", () => ({ createDimAuthApi: () => api, validDimRedirect: (s: string) => /^http:\/\/127\.0\.0\.1:43567\/oauth\/callback$/.test(s) }));
const session = (status: DimAuthSession["status"]): DimAuthSession => ({ id: "login", providerId: "dim", expectedRevision: "r1", site: "china", status,
  authorizationUrl: "https://dimagent.cn/oauth/authorize?state=synthetic", redirectUri: "http://127.0.0.1:43567/oauth/callback", identityAssurance: "user_info_endpoint",
  credentialId: status === "succeeded" ? "stable" : null, targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const props = (): DimOAuthProps => ({ client: { request: vi.fn() }, managementToken: "owner", secretGrant: "grant", providerId: "dim", expectedRevision: "r1",
  operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() });
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session("waiting_user")); api.get.mockResolvedValue(session("ready")); api.act.mockResolvedValue(session("cancelled")); });
afterEach(() => vi.useRealTimers());
function fill() {
  fireEvent.change(screen.getByLabelText("Client ID"), { target: { value: "permitted-gateway" } });
  fireEvent.change(screen.getByLabelText("已登记的回调地址"), { target: { value: session("ready").redirectUri } });
  fireEvent.change(screen.getByLabelText("Scopes"), { target: { value: "openid profile email" } });
}
it("requires explicit registration and uses the saved product site without borrowing a client", async () => {
  render(<DimOAuthPanel {...props()} site="china" />);
  expect(screen.getByLabelText("Client ID")).toHaveValue(""); expect(screen.getByLabelText("已登记的回调地址")).toHaveValue("");
  expect(screen.getByRole("button", { name: "通过 DIM 登录" })).toBeDisabled();
  fill(); await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 DIM 登录" })); });
  expect(api.create).toHaveBeenCalledWith(expect.objectContaining({ site: "china", clientId: "permitted-gateway", redirectUri: session("ready").redirectUri }), "grant");
  expect(screen.getByRole("link")).toHaveAttribute("href", session("waiting_user").authorizationUrl);
});
it("does not auto-save or present userinfo as a signed identity", async () => {
  const p = props(); render(<DimOAuthPanel {...p} site="china" />); fill();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 DIM 登录" })); });
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(api.act).not.toHaveBeenCalled(); expect(p.onSaved).not.toHaveBeenCalled();
  expect(screen.getByText(/未通过 ID token 验签/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "拉取目录并保存到凭证池" })).toBeEnabled();
  expect(screen.getByText(/不调用模型、不自动测试/)).toBeInTheDocument();
});
it("blocks an unsafe draft before creating a login", () => {
  render(<DimOAuthPanel {...props()} disabled site="global" />); fill();
  expect(screen.getByRole("button", { name: "通过 DIM 登录" })).toBeDisabled(); expect(api.create).not.toHaveBeenCalled();
});

it("unlocks the login form after the server reclaims an unsaved session", async () => {
  api.get.mockRejectedValue(new GatewayApiError("dim_login_unavailable", 400, "dim_login_rejected"));
  render(<DimOAuthPanel {...props()} site="china" />); fill();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 DIM 登录" })); });
  expect(screen.getByLabelText("Client ID")).toBeDisabled();
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(screen.getByText("授权会话已不可用，请重新授权。")).toBeInTheDocument();
  expect(screen.getByLabelText("Client ID")).toBeEnabled();
  expect(screen.getByRole("button", { name: "通过 DIM 登录" })).toBeEnabled();
  expect(screen.queryByLabelText("完整回调地址（仅手动回退）")).not.toBeInTheDocument();
  expect(api.create).toHaveBeenCalledOnce();
});

it("does not reopen login or saving when an accepted save receipt disappears", async () => {
  api.create.mockResolvedValueOnce(session("ready"));
  api.act.mockRejectedValueOnce(new Error("response lost"));
  const original = props(); render(<DimOAuthPanel {...original} site="china" />); fill();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 DIM 登录" })); });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "拉取目录并保存到凭证池" })); });
  api.get.mockRejectedValue(new GatewayApiError("dim_login_unavailable", 400, "dim_login_rejected"));
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(screen.getByText(/保存回执已不可用/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "通过 DIM 登录" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "拉取目录并保存到凭证池" })).not.toBeInTheDocument();
  expect(api.act).toHaveBeenCalledOnce(); expect(original.onSaved).not.toHaveBeenCalled();
});
