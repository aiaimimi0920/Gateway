import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GrokOAuthPanel } from "./GrokOAuthPanel";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/grok-auth", () => ({ createGrokAuthApi: () => api }));
const waiting = { id: "loopback-login", providerId: "build", expectedRevision: "r1", status: "waiting_user",
  authorizationUrl: "https://auth.x.ai/oauth2/authorize?state=synthetic", automaticCallback: true,
  identityVerified: false, deviceAuthorization: null, credentialId: null,
  targetCredentialId: null, savedRevision: null, errorCode: null };
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const props = { client, managementToken: "operator", secretGrant: "grant", providerId: "build", expectedRevision: "r1",
  operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); props.operations.clear(); api.create.mockResolvedValue(waiting); api.get.mockResolvedValue(waiting); api.act.mockResolvedValue({ ...waiting, status: "cancelled" }); });
afterEach(() => vi.useRealTimers());

it("explicitly requests loopback reception and only reads status until an explicit save", async () => {
  render(<GrokOAuthPanel {...props} />);
  expect(screen.getByRole("option", { name: "代码授权（本机自动回调）" })).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("授权方式"), { target: { value: "loopback" } });
  fireEvent.change(screen.getByLabelText("OAuth Client ID"), { target: { value: "permitted-client" } });
  fireEvent.change(screen.getByLabelText("Scopes（空格分隔）"), { target: { value: "offline_access" } });
  fireEvent.change(screen.getByLabelText("Redirect URI"), { target: { value: "http://127.0.0.1:34567/callback" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 Grok Build 登录" })); });
  expect(api.create).toHaveBeenCalledWith(expect.objectContaining({ authorizationFlow: "loopback", redirectUri: "http://127.0.0.1:34567/callback" }), "grant");
  expect(screen.queryByLabelText("完整回调地址")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "完成授权" })).not.toBeInTheDocument();
  api.get.mockResolvedValue({ ...waiting, status: "ready", authorizationUrl: null });
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(api.act).not.toHaveBeenCalled();
  expect(props.onSaved).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "保存到凭证池" })).toBeEnabled();
});
