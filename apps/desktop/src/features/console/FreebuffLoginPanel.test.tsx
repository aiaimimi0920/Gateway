import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import type { FreebuffAuthSession } from "../../api/console/freebuff-auth";
import { FreebuffLoginPanel } from "./FreebuffLoginPanel";
import type { FreebuffLoginProps } from "./useFreebuffLogin";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/freebuff-auth", () => ({ createFreebuffAuthApi: () => api }));
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const session = (status: FreebuffAuthSession["status"] = "waiting_user"): FreebuffAuthSession => ({ id: "login", providerId: "freebuff", expectedRevision: "r1", status,
  loginUrl: "https://freebuff.com/login?code=synthetic", pollIntervalSecs: 5, expiresInSecs: 590,
  credentialId: status === "succeeded" ? "account" : null, targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const props = (): FreebuffLoginProps => ({ client, managementToken: "owner", secretGrant: "grant", providerId: "freebuff", expectedRevision: "r1",
  operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() });
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
const check = async (time = 1500) => { await act(async () => { await vi.advanceTimersByTimeAsync(time); }); };
beforeEach(() => {
  vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session()); api.get.mockResolvedValue(session()); api.act.mockResolvedValue(session("cancelled"));
});
afterEach(() => vi.useRealTimers());

it("uses explicit browser approval and poll, then requires a separate save without frontend secrets", async () => {
  const owner = props(); render(<FreebuffLoginPanel {...owner} />);
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument(); await click("通过 FreeBuff 登录");
  expect(api.create).toHaveBeenCalledExactlyOnceWith({ providerId: "freebuff", expectedRevision: "r1", credentialId: undefined }, "grant");
  expect(screen.getByRole("link", { name: "打开授权页面" })).toHaveAttribute("href", session().loginUrl);
  expect(screen.getByLabelText("授权链接（可手动复制）")).toHaveAttribute("readonly");
  expect(screen.queryByLabelText("完整回调地址")).not.toBeInTheDocument();
  await check(); await check(); expect(api.get).toHaveBeenCalledTimes(2); expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(session("polling")); await click("检查授权");
  expect(api.act).toHaveBeenCalledExactlyOnceWith("login", "poll", expect.objectContaining({ expectedRevision: "r1" }), "grant", undefined);
  expect(screen.getByRole("button", { name: "检查授权" })).toBeDisabled();
  api.get.mockResolvedValue(session("ready")); await check(); expect(owner.onSaved).not.toHaveBeenCalled(); expect(api.act).toHaveBeenCalledTimes(1);
  api.act.mockResolvedValueOnce(session("succeeded")); await click("保存到凭证池");
  expect(owner.onSaved).toHaveBeenCalledOnce(); expect(owner.operations.size).toBe(0); expect(api.act).toHaveBeenCalledTimes(2);
});

it("cancels an unsaved session on close but never rolls back an accepted import", async () => {
  const owner = props(); const view = render(<FreebuffLoginPanel {...owner} />); await click("通过 FreeBuff 登录"); view.unmount();
  expect(api.act).toHaveBeenCalledExactlyOnceWith("login", "cancel", expect.objectContaining({ expectedRevision: "r1" }), "grant");
  api.act.mockClear(); const saved = render(<FreebuffLoginPanel {...owner} />); await click("通过 FreeBuff 登录");
  api.get.mockResolvedValue(session("ready")); await check(); api.act.mockResolvedValueOnce(session("importing")); await click("保存到凭证池");
  expect(screen.queryByRole("button", { name: "取消登录" })).not.toBeInTheDocument(); saved.unmount();
  expect(api.act).toHaveBeenCalledTimes(1); expect(api.act.mock.calls[0][1]).toBe("import");
});

it("restores ambiguous accepted saves after close/reopen and checks only their receipt", async () => {
  const owner = props(); const view = render(<FreebuffLoginPanel {...owner} />); await click("通过 FreeBuff 登录");
  api.get.mockResolvedValue(session("ready")); await check(); api.act.mockRejectedValueOnce(new Error("synthetic lost response")); await click("保存到凭证池");
  expect(screen.queryByText("登录材料已取得，尚未保存。")).not.toBeInTheDocument();
  expect(screen.getAllByText(/保存响应.*(未确认|丢失)/)).toHaveLength(1); view.unmount();
  render(<FreebuffLoginPanel {...owner} />);
  expect(screen.getByRole("button", { name: "通过 FreeBuff 登录" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "保存到凭证池" })).not.toBeInTheDocument();
  api.get.mockResolvedValue(session("succeeded")); await check();
  expect(owner.onSaved).toHaveBeenCalledOnce(); expect(api.create).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledTimes(1);
});

it("keeps save_unconfirmed locked without cancellation, resubmission or false success", async () => {
  const owner = props(); const activity = vi.fn(); const view = render(<FreebuffLoginPanel {...owner} onActivityChange={activity} />); await click("通过 FreeBuff 登录");
  api.get.mockResolvedValue(session("ready")); await check(); api.act.mockResolvedValueOnce(session("save_unconfirmed")); await click("保存到凭证池");
  expect(activity).toHaveBeenLastCalledWith(true); expect(screen.getByRole("button", { name: "通过 FreeBuff 登录" })).toBeDisabled();
  expect(screen.getByText(/保存结果未确认/)).toBeInTheDocument(); view.unmount();
  expect(api.act).toHaveBeenCalledTimes(1); expect(owner.onSaved).not.toHaveBeenCalled();
});

it.each(["freebuff_login_save_rejected", "unknown"])("only an explicit pre-admission rejection permits save retry: %s", async (code) => {
  const owner = props(); render(<FreebuffLoginPanel {...owner} />); await click("通过 FreeBuff 登录");
  api.get.mockResolvedValue(session("ready")); await check(); api.act.mockRejectedValueOnce(new GatewayApiError("Rejected", 400, code)); await click("保存到凭证池");
  expect(Boolean(screen.queryByRole("button", { name: "保存到凭证池" }))).toBe(code === "freebuff_login_save_rejected");
  await check(); expect(api.act).toHaveBeenCalledTimes(1); expect(owner.onSaved).not.toHaveBeenCalled();
});

it.each([{ disabled: true }, { expectedRevision: "" }, { managementToken: null }])("prevents starting without a saved writable management context: %j", (patch) => {
  render(<FreebuffLoginPanel {...props()} {...patch} />); expect(screen.getByRole("button", { name: "通过 FreeBuff 登录" })).toBeDisabled(); expect(api.create).not.toHaveBeenCalled();
});

it("makes cancellation explicit and shows the terminal result", async () => {
  render(<FreebuffLoginPanel {...props()} />); await click("通过 FreeBuff 登录"); await click("取消登录");
  expect(screen.getByText("登录已取消。")).toBeInTheDocument(); expect(screen.getByRole("button", { name: "通过 FreeBuff 登录" })).toBeEnabled();
});
