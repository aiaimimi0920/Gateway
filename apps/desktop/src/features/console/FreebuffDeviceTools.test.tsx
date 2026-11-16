import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { FreebuffCatalogPanel, type FreebuffCatalogProps } from "./FreebuffCatalogPanel";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const endpoint = "/v1/internal/gateway/console/freebuff-device-registration";
const recordId = "31842df2-df63-43ee-a2a4-eab268547f31";
function owner() {
  const value = { revision: "r1", providerId: "pool", credentialId: "b", businessOrigin: "https://codebuff.com", client: "cli" };
  const request = vi.fn().mockResolvedValue({ ...value, registration: "absent" });
  const props: FreebuffCatalogProps = { client: { request }, providerId: "pool", credentialId: "b", expectedRevision: "r1",
    managementToken: "owner", secretGrant: "grant", disabled: false, onRequestSecretAccess: vi.fn() };
  return { props, request, value };
}
function panel(props: FreebuffCatalogProps) { return <NeuroProvider><FreebuffCatalogPanel {...props} /></NeuroProvider>; }
function choose() {
  fireEvent.change(screen.getByLabelText("业务站点"), { target: { value: "https://codebuff.com" } });
  fireEvent.change(screen.getByLabelText("客户端"), { target: { value: "cli" } });
}
async function click(name: string) { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); }
it("checks local state explicitly and registers only after a scoped confirmation", async () => {
  const { props, request, value } = owner(); render(panel(props));
  expect(request).not.toHaveBeenCalled(); expect(screen.getByRole("button", { name: "检查设备" })).toBeDisabled(); choose();
  await click("检查设备");
  expect(request).toHaveBeenCalledExactlyOnceWith(`${endpoint}/status`, expect.anything(), expect.objectContaining({
    body: { providerId: "pool", credentialId: "b", expectedRevision: "r1", businessOrigin: value.businessOrigin, client: "cli" },
    method: "POST", managementToken: "owner", secretGrant: "grant",
  }));
  await click("注册设备"); expect(request).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("alertdialog")).toHaveTextContent("https://codebuff.com");
  request.mockResolvedValue({ ...value, registration: "stored", recordId });
  await click("确认注册");
  expect(request.mock.calls[1][0]).toBe(endpoint);
  expect(screen.getByText("设备注册已保存")).toBeInTheDocument();
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
});

it.each(["stored", "unconfirmed"] as const)("requires an exact one-use %s confirmation, and never retries on reopen", async (registration) => {
  const { props, request, value } = owner();
  request.mockResolvedValue({ ...value, registration, ...(registration === "stored" ? { recordId } : { unconfirmedAttemptId: recordId }) });
  const view = render(panel(props)); choose(); await click("检查设备");
  await click(registration === "stored" ? "重新注册设备" : "再次注册");
  expect(request).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("alertdialog")).toHaveTextContent(registration === "stored" ? "保留旧记录" : "不保证幂等");
  request.mockRejectedValue(new Error("synthetic-secret-upstream")); await click("确认注册");
  expect(request.mock.calls[1][2].body).toEqual(expect.objectContaining(registration === "stored"
    ? { replaceStoredRecordId: recordId } : { retryUnconfirmedAttemptId: recordId }));
  expect(screen.getByText(/未取得注册确认/)).toBeInTheDocument();
  expect(screen.queryByText(/synthetic-secret-upstream/)).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "确认注册" })).not.toBeInTheDocument();
  view.unmount(); render(panel(props)); choose();
  expect(request).toHaveBeenCalledTimes(2);
  expect(screen.queryByRole("button", { name: "再次注册" })).not.toBeInTheDocument();
});

it.each([
  { providerId: "other" }, { credentialId: "other" }, { expectedRevision: "r2" }, { managementToken: "other" },
  { secretGrant: "new" }, { secretGrant: null }, { disabled: true }, { client: { request: vi.fn() } },
])("invalidates an open confirmation when its saved context changes: %j", async patch => {
  const { props, request } = owner(); const view = render(panel(props)); choose();
  await click("检查设备"); await click("注册设备"); expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  view.rerender(panel({ ...props, ...patch }));
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(); expect(request).toHaveBeenCalledTimes(1);
});

it.each(["revision", "providerId", "credentialId", "businessOrigin", "client", "apiKey", "keyId", "sealed"])("rejects foreign or secret state DTO: %s", async field => {
  const { props, request, value } = owner(); request.mockResolvedValue({ ...value, registration: "stored", recordId, [field]: "foreign" });
  render(panel(props)); choose(); await click("检查设备");
  expect(screen.getByText(/设备状态响应与当前账号/)).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "重新注册设备" })).not.toBeInTheDocument();
});

it("requires restored Grant without automatically checking state", async () => {
  const { props, request } = owner(); const view = render(panel({ ...props, secretGrant: null })); choose(); await click("检查设备");
  expect(request).not.toHaveBeenCalled(); expect(props.onRequestSecretAccess).toHaveBeenCalledOnce();
  view.rerender(panel(props)); expect(request).not.toHaveBeenCalled(); await click("检查设备"); expect(request).toHaveBeenCalledOnce();
});

it("stops waiting without rollback/replay and rejects a late write result", async () => {
  const { props, request, value } = owner(); const view = render(panel(props)); choose();
  await click("检查设备"); await click("注册设备");
  let resolve!: (value: unknown) => void;
  request.mockImplementation(() => new Promise(done => { resolve = done; })); await click("确认注册");
  const signal = request.mock.calls[1][2].signal;
  expect(screen.getByRole("button", { name: "读取目录" })).toBeDisabled(); expect(screen.getByLabelText("客户端")).toBeDisabled();
  await click("停止等待"); expect(signal.aborted).toBe(true); expect(screen.getByText(/已停止等待/)).toBeInTheDocument();
  await act(async () => { resolve({ ...value, registration: "stored", recordId }); });
  expect(screen.queryByText("设备注册已保存")).not.toBeInTheDocument();
  request.mockResolvedValue({ ...value, registration: "unconfirmed", unconfirmedAttemptId: recordId });
  await click("检查设备"); await click("再次注册");
  request.mockImplementation(() => new Promise(() => {})); await click("确认注册");
  const nextSignal = request.mock.calls[3][2].signal; view.unmount(); expect(nextSignal.aborted).toBe(true);
  expect(request).toHaveBeenCalledTimes(4);
});

it.each([{ disabled: true }, { expectedRevision: "" }, { managementToken: null }])("blocks device work without clean saved authority: %j", patch => {
  const { props, request } = owner(); render(panel({ ...props, ...patch })); choose();
  expect(screen.getByRole("button", { name: "检查设备" })).toBeDisabled(); expect(request).not.toHaveBeenCalled();
});
