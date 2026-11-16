import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GrokOAuthPanel } from "./GrokOAuthPanel";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/grok-auth", () => ({ createGrokAuthApi: () => api }));
const waiting = { id: "device-login", providerId: "build", expectedRevision: "r1", status: "waiting_user",
  authorizationUrl: "https://accounts.x.ai/device?user_code=TEST-CODE", identityVerified: false,
  deviceAuthorization: { userCode: "TEST-CODE", pollIntervalSecs: 5, pollAfterSecs: 0, expiresInSecs: 300 },
  credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null };
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const props = { client, managementToken: "operator", secretGrant: "grant", providerId: "build", expectedRevision: "r1",
  operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); props.operations.clear(); api.create.mockResolvedValue(waiting); api.get.mockResolvedValue(waiting); api.act.mockResolvedValue({ ...waiting, status: "cancelled" }); });
afterEach(() => vi.useRealTimers());
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };

it("creates device authorization without a redirect, displays only public user code, and explicitly polls before saving", async () => {
  render(<GrokOAuthPanel {...props} />);
  fireEvent.change(screen.getByLabelText("授权方式"), { target: { value: "device" } });
  fireEvent.change(screen.getByLabelText("OAuth Client ID"), { target: { value: "permitted-client" } });
  fireEvent.change(screen.getByLabelText("Scopes（空格分隔）"), { target: { value: "offline_access" } });
  expect(screen.queryByLabelText("Redirect URI")).not.toBeInTheDocument();
  await click("通过 Grok Build 登录");
  expect(api.create).toHaveBeenCalledWith({ providerId: "build", expectedRevision: "r1", credentialId: undefined,
    clientId: "permitted-client", scopes: ["offline_access"], authorizationFlow: "device" }, "grant");
  expect(screen.getByRole("link")).toHaveAttribute("href", waiting.authorizationUrl);
  expect(screen.getByLabelText("用户授权码")).toHaveValue("TEST-CODE");
  expect(screen.queryByLabelText("完整回调地址")).not.toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce({ ...waiting, status: "ready", authorizationUrl: null, deviceAuthorization: null });
  await click("检查授权");
  expect(api.act).toHaveBeenCalledWith("device-login", "poll", expect.objectContaining({ expectedRevision: "r1" }), "grant", undefined);
  expect(props.onSaved).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "保存到凭证池" })).toBeEnabled();
});

it("does not silently ignore code-flow principal selection in a device grant", () => {
  render(<GrokOAuthPanel {...props} />);
  fireEvent.change(screen.getByLabelText("Consent 预选 ID（仅提示）"), { target: { value: "team-hint" } });
  fireEvent.change(screen.getByLabelText("授权方式"), { target: { value: "device" } });
  expect(screen.getByRole("button", { name: "通过 Grok Build 登录" })).toBeDisabled();
});

it.each(["https://evil.invalid/device", "https://accounts.x.ai.evil.invalid/device", "http://accounts.x.ai/device", "https://auth.x.ai@evil.invalid/device", "https://accounts.x.ai/device#private"])("refuses unsafe device authorization links: %s", (value) => {
  render(<ChatgptAuthorizationLink kind="grok-device" value={value} busy={false} />);
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
});

it("keeps long server intervals outside the HTTP poll action, without automatic token polling", async () => {
  api.create.mockResolvedValueOnce({ ...waiting, deviceAuthorization: { ...waiting.deviceAuthorization, pollIntervalSecs: 60, pollAfterSecs: 60 } });
  render(<GrokOAuthPanel {...props} />);
  fireEvent.change(screen.getByLabelText("授权方式"), { target: { value: "device" } });
  fireEvent.change(screen.getByLabelText("OAuth Client ID"), { target: { value: "permitted-client" } });
  fireEvent.change(screen.getByLabelText("Scopes（空格分隔）"), { target: { value: "offline_access" } });
  await click("通过 Grok Build 登录");
  expect(screen.getByRole("button", { name: "检查授权" })).toBeDisabled();
  api.get.mockResolvedValueOnce(waiting);
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(screen.getByRole("button", { name: "检查授权" })).toBeEnabled();
  expect(api.act).not.toHaveBeenCalled();
});
