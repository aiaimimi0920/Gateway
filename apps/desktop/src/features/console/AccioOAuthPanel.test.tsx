import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import type { AccioAuthSession } from "../../api/console/accio-auth";
import { AccioOAuthPanel } from "./AccioOAuthPanel";
import type { AccioOAuthProps } from "./useAccioOAuth";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/accio-auth", () => ({ createAccioAuthApi: () => api }));
const client: GatewayApiClient = { request: async () => { throw new Error("Unexpected transport"); } };
const session = (status: AccioAuthSession["status"]): AccioAuthSession => ({ id: "login", providerId: "accio", expectedRevision: "r1", status,
  authorizationUrl: "https://www.accio-ai.com/login?state=synthetic", credentialId: status === "succeeded" ? "account" : null,
  targetCredentialId: null, savedRevision: status === "succeeded" ? "r2" : null, errorCode: null });
const props: AccioOAuthProps = { client, managementToken: "operator", secretGrant: "grant", providerId: "accio", expectedRevision: "r1",
  operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
const poll = async () => { await act(async () => { await vi.advanceTimersByTimeAsync(1500); }); };
function fill() {
  fireEvent.change(screen.getByLabelText("Client ID"), { target: { value: "permitted-client" } });
  fireEvent.change(screen.getByLabelText("授权站点"), { target: { value: "china" } });
  fireEvent.change(screen.getByLabelText("回调端口"), { target: { value: "34567" } });
  fireEvent.change(screen.getByLabelText("utdid"), { target: { value: "explicit-device!" } });
}
beforeEach(() => { props.operations.clear(); vi.useFakeTimers(); vi.resetAllMocks(); api.create.mockResolvedValue(session("waiting_user")); api.get.mockResolvedValue(session("ready")); api.act.mockResolvedValue(session("cancelled")); });
afterEach(() => vi.useRealTimers());

it("has no implicit client/site/port/device and sends the explicit Accio contract", async () => {
  render(<AccioOAuthPanel {...props} />);
  for (const label of ["Client ID", "授权站点", "回调端口", "utdid"]) expect(screen.getByLabelText(label)).toHaveValue("");
  expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toBeDisabled();
  fill(); await click("通过 Accio 登录");
  expect(api.create).toHaveBeenCalledExactlyOnceWith({ providerId: "accio", expectedRevision: "r1", credentialId: undefined,
    clientId: "permitted-client", loginSite: "china", callbackPort: 34567, utdid: "explicit-device!" }, "grant");
  expect(screen.getByRole("link")).toHaveAttribute("href", session("waiting_user").authorizationUrl);
  expect(screen.getByLabelText("完整回调地址")).toHaveAttribute("type", "password");
  expect(screen.getByLabelText("完整回调地址")).toHaveAttribute("maxlength", "16384");
  fireEvent.change(screen.getByLabelText("完整回调地址"), { target: { value: "private-callback" } });
  await click("完成授权");
  expect(api.act).toHaveBeenCalledWith("login", "complete", expect.objectContaining({ expectedRevision: "r1" }), "grant", "private-callback");
  expect(props.onSaved).not.toHaveBeenCalled();
});

it.each(["", " device", "device ", "设备", "a\tb", "x".repeat(1025)])("rejects invalid new device without trimming: %j", (utdid) => {
  render(<AccioOAuthPanel {...props} />); fill(); fireEvent.change(screen.getByLabelText("utdid"), { target: { value: utdid } });
  expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toBeDisabled(); expect(api.create).not.toHaveBeenCalled();
});

it.each(["0", "65536", "1e3", "-1", "1.5"])("rejects invalid callback port %s", (port) => {
  render(<AccioOAuthPanel {...props} />); fill(); fireEvent.change(screen.getByLabelText("回调端口"), { target: { value: port } });
  expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toBeDisabled();
});

it("omits empty utdid only for explicit stable-ID reauthorization", async () => {
  render(<AccioOAuthPanel {...props} credentialId="stable" />); fill();
  fireEvent.change(screen.getByLabelText("utdid"), { target: { value: "" } }); await click("通过 Accio 登录");
  expect(api.create.mock.calls[0][0]).toMatchObject({ credentialId: "stable" });
  expect(api.create.mock.calls[0][0]).not.toHaveProperty("utdid");
});

it("clears a submitted callback while its captured action waits for a renewed grant", async () => {
  const view = render(<AccioOAuthPanel {...props} />); fill(); await click("通过 Accio 登录");
  view.rerender(<AccioOAuthPanel {...props} secretGrant={null} />);
  fireEvent.change(screen.getByLabelText("完整回调地址"), { target: { value: "original-callback" } }); await click("完成授权");
  expect(screen.getByLabelText("完整回调地址")).toHaveValue(""); expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(session("ready")); view.rerender(<AccioOAuthPanel {...props} secretGrant="renewed" />); await act(async () => {});
  expect(api.act).toHaveBeenCalledWith("login", "complete", expect.anything(), "renewed", "original-callback");
});

it("does not save on ready, and confirms exactly once after a revision change", async () => {
  const view = render(<AccioOAuthPanel {...props} />); fill(); await click("通过 Accio 登录"); await poll();
  expect(api.act).not.toHaveBeenCalled(); api.act.mockResolvedValueOnce(session("importing")); await click("保存到凭证池");
  expect(props.onSaved).not.toHaveBeenCalled(); view.rerender(<AccioOAuthPanel {...props} expectedRevision="r2" disabled />);
  api.get.mockResolvedValue(session("succeeded")); await poll(); await poll();
  expect(props.onSaved).toHaveBeenCalledOnce(); view.unmount(); expect(api.act).toHaveBeenCalledTimes(1);
});

it("retains an ambiguous save after close/reopen without reimporting", async () => {
  const view = render(<AccioOAuthPanel {...props} />); fill(); await click("通过 Accio 登录"); await poll();
  api.act.mockRejectedValueOnce(new Error("lost")); await click("保存到凭证池"); view.unmount();
  render(<AccioOAuthPanel {...props} />);
  expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "保存到凭证池" })).not.toBeInTheDocument();
  api.get.mockResolvedValue(session("succeeded")); await poll();
  expect(props.onSaved).toHaveBeenCalledOnce(); expect(api.create).toHaveBeenCalledOnce(); expect(api.act).toHaveBeenCalledTimes(1);
});

it("keeps save_unconfirmed terminal and blocks both saving and a new login", async () => {
  render(<AccioOAuthPanel {...props} />); fill(); await click("通过 Accio 登录"); await poll();
  api.act.mockResolvedValueOnce(session("save_unconfirmed")); await click("保存到凭证池"); await poll();
  expect(screen.getByRole("status")).toHaveTextContent("保存结果未确认"); expect(props.onSaved).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toBeDisabled(); expect(api.act).toHaveBeenCalledTimes(1);
});

it("allows only an explicit retry after Accio pre-admission rejection", async () => {
  render(<AccioOAuthPanel {...props} />); fill(); await click("通过 Accio 登录"); await poll();
  api.act.mockRejectedValueOnce(new GatewayApiError("Rejected", 400, "accio_login_save_rejected")); await click("保存到凭证池");
  expect(screen.getByRole("button", { name: "保存到凭证池" })).toBeEnabled(); await poll();
  expect(api.act).toHaveBeenCalledTimes(1); expect(props.onSaved).not.toHaveBeenCalled();
});

it("does not infer rejection from an arbitrary HTTP 400 response", async () => {
  render(<AccioOAuthPanel {...props} />); fill(); await click("通过 Accio 登录"); await poll();
  api.act.mockRejectedValueOnce(new GatewayApiError("Unknown", 400, "unknown")); await click("保存到凭证池");
  expect(screen.queryByRole("button", { name: "保存到凭证池" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toBeDisabled();
});
