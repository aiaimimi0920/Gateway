import { act, fireEvent, render, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { NousOAuthPanel } from "./NousOAuthPanel";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/nous-auth", () => ({ createNousAuthApi: () => api }));
const waiting = { id: "login", providerId: "pool", expectedRevision: "r1", status: "waiting_user",
  authorizationUrl: "https://portal.nousresearch.com/device", receiptReceived: false,
  deviceAuthorization: { userCode: "ABCD", pollAfterSecs: 0, expiresInSecs: 300 },
  credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null };
const props = { client: { request: vi.fn() }, managementToken: "owner", secretGrant: "grant", providerId: "pool", expectedRevision: "r1",
  operations: new Map(), disabled: false, onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
const ready = { ...waiting, status: "ready", authorizationUrl: null, deviceAuthorization: null, receiptReceived: true };
beforeEach(() => { vi.useFakeTimers(); vi.resetAllMocks(); props.operations.clear(); api.create.mockResolvedValue(waiting); api.get.mockResolvedValue(waiting); api.act.mockResolvedValue({ ...waiting, status: "cancelled" }); });
afterEach(() => vi.useRealTimers());
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
async function start() {
  fireEvent.change(screen.getByLabelText("获准的 Client ID"), { target: { value: "allowed-client" } });
  await click("通过 Nous 登录");
}
it("requires explicit client, device poll and save; status reads never exchange tokens", async () => {
  render(<NousOAuthPanel {...props} />, { wrapper: NeuroProvider });
  expect(screen.getByRole("button", { name: "通过 Nous 登录" })).toBeDisabled();
  await start();
  expect(api.create).toHaveBeenCalledWith({ providerId: "pool", expectedRevision: "r1", credentialId: undefined, clientId: "allowed-client" }, "grant");
  expect(screen.getByLabelText("用户授权码")).toHaveValue("ABCD");
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(api.act).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce(ready); await click("检查授权");
  expect(props.onSaved).not.toHaveBeenCalled();
  api.act.mockResolvedValueOnce({ ...ready, status: "succeeded", credentialId: "saved", savedRevision: "r2" });
  await click("保存到凭证池"); expect(props.onSaved).toHaveBeenCalledOnce();
});
it("retains an uncertain accepted save across reopening without a duplicate import", async () => {
  api.create.mockResolvedValue(ready); api.get.mockResolvedValue(ready);
  const view = render(<NousOAuthPanel {...props} />, { wrapper: NeuroProvider }); await start();
  api.act.mockRejectedValueOnce(new Error("lost response")); await click("保存到凭证池");
  expect(props.operations.size).toBe(1); view.unmount();
  render(<NousOAuthPanel {...props} />, { wrapper: NeuroProvider });
  expect(screen.queryByRole("button", { name: "保存到凭证池" })).not.toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  expect(api.act.mock.calls.filter((c) => c[1] === "import")).toHaveLength(1);
});
it("does not rebase an explicit operation after the saved revision changes", async () => {
  api.create.mockResolvedValue(ready);
  const view = render(<NousOAuthPanel {...props} />, { wrapper: NeuroProvider }); await start();
  view.rerender(<NousOAuthPanel {...props} expectedRevision="r2" />);
  expect(screen.getByRole("button", { name: "保存到凭证池" })).toBeDisabled();
  expect(api.act).not.toHaveBeenCalled();
});
it.each(["https://evil.invalid/device", "https://portal.nousresearch.com.evil.invalid/device", "http://portal.nousresearch.com/device", "https://x@portal.nousresearch.com/device", "https://portal.nousresearch.com/device#secret", "https://portal.nousresearch.com:444/device"])("refuses unsafe Nous URL %s", (value) => {
  render(<ChatgptAuthorizationLink kind="nous" value={value} busy={false} />, { wrapper: NeuroProvider });
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
});
