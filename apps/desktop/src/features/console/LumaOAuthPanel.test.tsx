import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import type { LumaAuthSession } from "../../api/console/luma-auth";
import { GatewayApiError } from "../../api/errors";
import { LumaOAuthPanel } from "./LumaOAuthPanel";
import type { LumaOAuthProps } from "./useLumaOAuth";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const session = (status: LumaAuthSession["status"]): LumaAuthSession => ({ id: "login", providerId: "mcp", expectedRevision: "r1", status,
  authorizationUrl: status === "waiting_user" ? "https://agents.lumalabs.ai/mcp/oauth/authorize?state=synthetic" : null,
  redirectUri: "http://127.0.0.1:43567/callback", credentialId: status === "succeeded" ? "stable" : null,
  targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
function fixture() {
  let status: LumaAuthSession["status"] = "waiting_user";
  const request = vi.fn(async (path: string, schema, _options?: GatewayApiRequestOptions) => {
    if (path.endsWith("/import")) status = "importing";
    if (path.endsWith("/cancel")) status = "cancelled";
    if (path.endsWith("/complete")) status = "ready";
    return schema.parse({ session: session(status) });
  });
  const props: LumaOAuthProps = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "mcp", expectedRevision: "r1", operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
  return { props, request, setStatus: (next: LumaAuthSession["status"]) => { status = next; } };
}
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
const poll = async () => { await act(async () => { await vi.advanceTimersByTimeAsync(1500); }); };
function fill() {
  fireEvent.change(screen.getByLabelText("Client ID"), { target: { value: "registered-client" } });
  fireEvent.change(screen.getByLabelText("已登记的回调地址"), { target: { value: session("ready").redirectUri } });
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
it("sends offline consent only after the explicit checkbox is selected", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />); fill();
  fireEvent.click(screen.getByRole("checkbox", { name: "允许刷新账号授权" }));
  await click("通过 Luma 登录");
  expect(f.request.mock.calls[0][2]?.body).toMatchObject({ offlineAccess: true });
});
it("follows explicit login, callback and save without auto-import or tools calls", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />);
  expect(screen.getByRole("button", { name: "通过 Luma 登录" })).toBeDisabled(); fill(); await click("通过 Luma 登录");
  expect(f.request.mock.calls[0][2]).toMatchObject({ body: { providerId: "mcp", expectedRevision: "r1", clientId: "registered-client" }, secretGrant: "grant" });
  expect(screen.getByRole("link")).toHaveAttribute("href", session("waiting_user").authorizationUrl);
  f.setStatus("ready"); await poll();
  expect(f.request.mock.calls.filter(([path]) => path.endsWith("/import"))).toHaveLength(0);
  expect(f.props.onSaved).not.toHaveBeenCalled(); await click("保存只读 MCP 账号");
  expect(screen.getByText("正在保存")).toBeInTheDocument();
  f.setStatus("succeeded"); await poll(); expect(f.props.onSaved).toHaveBeenCalledOnce();
  expect(f.request.mock.calls.every(([path]) => path.includes("/luma-auth-sessions"))).toBe(true);
});
it("clears manual callback material immediately and never stores it in retained receipts", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />); fill(); await click("通过 Luma 登录");
  const callback = `${session("ready").redirectUri}?code=private&state=synthetic`;
  fireEvent.change(screen.getByLabelText("回调地址（手动完成）"), { target: { value: callback } }); await click("完成授权");
  expect(f.request.mock.calls.at(-1)?.[2]?.body).toEqual({ expectedRevision: "r1", callbackUrl: callback });
  expect(screen.queryByDisplayValue(callback)).not.toBeInTheDocument();
  expect(JSON.stringify([...f.props.operations])).not.toContain("private");
});
it("does not rebase a create waiting for a new secret grant", async () => {
  const f = fixture(); const props = { ...f.props, secretGrant: null };
  const view = render(<LumaOAuthPanel {...props} />); fill(); await click("通过 Luma 登录");
  expect(f.props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(f.request).not.toHaveBeenCalled();
  view.rerender(<LumaOAuthPanel {...props} secretGrant="renewed" expectedRevision="r2" />);
  await act(async () => {}); expect(f.request).not.toHaveBeenCalled();
});
it("cancels an unsaved session on close", async () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} />); fill(); await click("通过 Luma 登录");
  view.unmount(); await act(async () => {});
  expect(f.request.mock.calls.at(-1)?.[0]).toMatch(/\/login\/cancel$/);
});
it("preserves an ambiguous save across reopen without replay, then accepts a confirmed receipt", async () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} />); fill(); await click("通过 Luma 登录");
  f.setStatus("ready"); await poll(); f.request.mockRejectedValueOnce(new Error("response lost"));
  await click("保存只读 MCP 账号"); expect(f.props.operations.size).toBe(1); view.unmount();
  render(<LumaOAuthPanel {...f.props} />);
  expect(screen.queryByRole("button", { name: "通过 Luma 登录" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存只读 MCP 账号" })).not.toBeInTheDocument();
  f.setStatus("succeeded"); await poll(); expect(f.props.onSaved).toHaveBeenCalledOnce();
  expect(f.request.mock.calls.filter(([path]) => path.endsWith("/import"))).toHaveLength(1);
});
it("allows explicit restart for a reclaimed unsaved session but not a lost save receipt", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />); fill(); await click("通过 Luma 登录");
  f.request.mockRejectedValueOnce(new GatewayApiError("luma_login_unavailable", 400, "luma_login_rejected")); await poll();
  expect(screen.getByRole("button", { name: "通过 Luma 登录" })).toBeEnabled();
  await click("通过 Luma 登录"); f.setStatus("ready"); await poll();
  f.request.mockRejectedValueOnce(new Error("response lost")); await click("保存只读 MCP 账号");
  f.request.mockRejectedValueOnce(new GatewayApiError("luma_login_unavailable", 400, "luma_login_rejected")); await poll();
  expect(screen.getByText(/保存回执已不可用/)).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "通过 Luma 登录" })).not.toBeInTheDocument();
  expect(f.props.onSaved).not.toHaveBeenCalled();
});
it("continues checking an accepted save after revision change but does not create or import again", async () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} />); fill(); await click("通过 Luma 登录");
  f.setStatus("ready"); await poll(); await click("保存只读 MCP 账号");
  view.rerender(<LumaOAuthPanel {...f.props} expectedRevision="r2" disabled />);
  f.setStatus("succeeded"); await poll(); expect(f.props.onSaved).toHaveBeenCalledOnce();
  expect(f.request.mock.calls.filter(([path]) => path.endsWith("/import"))).toHaveLength(1);
});
