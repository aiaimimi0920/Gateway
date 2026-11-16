import type { ComponentProps } from "react";
import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ConsoleGlobalDialogs } from "./ConsoleGlobalDialogs";
import { pendingPurchase } from "../../api/console/freebuff-purchases.fixtures";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
vi.mock("./AccountGroupDialog", () => ({ AccountGroupDialog: () => null }));
vi.mock("./ProviderCatalogDialog", () => ({ ProviderCatalogDialog: () => null }));
vi.mock("./CredentialPruneDialog", () => ({ CredentialPruneDialog: () => null }));
vi.mock("./GeminiManualAddDialog", () => ({ GeminiManualAddDialog: () => null }));
vi.mock("./PilotActionDialog", () => ({ PilotActionDialog: () => null }));
vi.mock("./useCredentialKeyReveal", () => ({ useCredentialKeyReveal: () => undefined }));
vi.mock("./useChatgptWebBrowserCapture", () => ({ useChatgptWebBrowserCapture: () => undefined }));
type Controller = ComponentProps<typeof ConsoleGlobalDialogs>["controller"];
afterEach(() => vi.useRealTimers());
function controller(mode = "add", preset = "freebuff"): Controller {
  return {
    client: { request: vi.fn() }, session: { managementToken: "owner", secretGrant: { grant: "grant" }, confirmSecretAccess: vi.fn() },
    routeConfig: { routeConfig: { revision: { id: "saved-r1" } } },
    draftDocumentState: { document: { providers: [{ id: "renamed-provider", preset, base_url: "https://freebuff.com", explicit_credentials_only: true, credentials: [] }] } },
    credentialDialogState: { mode, initialValue: {
      providerId: "renamed-provider", credentialId: mode === "edit" ? "stable-id" : "", accountName: "", enabled: false,
      baseUrl: "", supportedModelsText: "", apiKeyOperation: mode === "edit" ? "keep" : "replace", apiKeyValue: "",
    } },
    credentialProviderOptions: [{ id: "renamed-provider", label: "Renamed" }], explicitCredentialIds: [],
    displayedAccountCatalog: { accounts: [] }, groupDirectory: [], editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true,
    hasSecretAccess: true, setCredentialDialogState: vi.fn(), applyCredentialDialogValue: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}

it("checks and explicitly re-registers the saved device through the normal edit host", async () => {
  const owner = controller("edit"); const recordId = "31842df2-df63-43ee-a2a4-eab268547f31";
  const request = vi.mocked(owner.client.request);
  request.mockImplementation(async (_path, schema, options) => schema.parse({
    providerId: "renamed-provider", credentialId: "stable-id", revision: "saved-r1", businessOrigin: "https://codebuff.com", client: "cli",
    registration: "stored", recordId,
  }));
  render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  expect(request).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("业务站点"), { target: { value: "https://codebuff.com" } });
  fireEvent.change(screen.getByLabelText("客户端"), { target: { value: "cli" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "检查设备" })); });
  fireEvent.click(screen.getByRole("button", { name: "重新注册设备" }));
  expect(request).toHaveBeenCalledOnce();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "确认注册" })); });
  expect(request.mock.calls[1][0]).toBe("/v1/internal/gateway/console/freebuff-device-registration");
  expect(request.mock.calls[1][2]).toEqual(expect.objectContaining({ managementToken: "owner", secretGrant: "grant",
    body: { providerId: "renamed-provider", credentialId: "stable-id", expectedRevision: "saved-r1", businessOrigin: "https://codebuff.com", client: "cli", replaceStoredRecordId: recordId } }));
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled(); expect(owner.refresh).not.toHaveBeenCalled();
});

it("offers FreeBuff browser approval by exact preset while retaining explicit existing-session import", () => {
  const owner = controller();
  render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  expect(screen.getByRole("button", { name: "通过 FreeBuff 登录" })).toBeInTheDocument();
  expect(screen.queryByLabelText("会话材料 JSON")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "token" } });
  expect(screen.getByLabelText("会话材料 JSON")).toBeInTheDocument();
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled();
});

it("does not enable FreeBuff approval on another product with the same mutable identity", () => {
  render(<NeuroProvider><ConsoleGlobalDialogs controller={controller("add", "xai-api")} /></NeuroProvider>);
  expect(screen.queryByRole("button", { name: "通过 FreeBuff 登录" })).not.toBeInTheDocument();
});

it.each(["editorLocked", "draftDirty", "draftMatchesActiveRevision"] as const)("blocks approval in an unsafe saved context: %s", (field) => {
  const owner = controller(); owner[field] = field !== "draftMatchesActiveRevision";
  render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  expect(screen.getByRole("button", { name: "通过 FreeBuff 登录" })).toBeDisabled();
});

it("retains the old product-bound material import through the normal credential form", async () => {
  const owner = controller(); render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "token" } });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "manual-stable" } });
  fireEvent.change(screen.getByLabelText("会话材料 JSON"), { target: { value: JSON.stringify({ product: "freebuff-web", token: "synthetic-token", agentId: "synthetic-agent" }) } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取会话材料" })); });
  expect(screen.getByLabelText("会话材料 JSON")).toHaveValue("");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({
    providerId: "renamed-provider", credentialId: "manual-stable", apiKeyValue: "", apiKeyOperation: "replace",
    sessionMaterial: expect.objectContaining({ product: "freebuff-web", token: "synthetic-token", agentId: "synthetic-agent" }),
  }));
});

function transport(owner: Controller, mode = "add", lost = false) {
  const request = vi.fn(async (path, schema, options) => {
    if (lost && path.endsWith("/import")) throw new Error("synthetic lost response");
    const status = path.endsWith("/import") ? "succeeded" : path.endsWith("/poll") ? "ready" : path.endsWith("/cancel") ? "cancelled" : "waiting_user";
    return schema.parse({ session: { id: "login", providerId: "renamed-provider", expectedRevision: "saved-r1", status,
      loginUrl: "https://freebuff.com/login?code=synthetic", pollIntervalSecs: 5, expiresInSecs: 590,
      targetCredentialId: mode === "edit" ? "stable-id" : null, credentialId: status === "succeeded" ? mode === "edit" ? "stable-id" : "new-id" : null,
      savedRevision: status === "succeeded" ? "saved-r2" : null, errorCode: null } });
  });
  owner.client = { request }; return request;
}
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };

it.each(["add", "edit"])("saves using the real FreeBuff client and only closes/refreshes on a valid receipt: %s", async (mode) => {
  const owner = controller(mode); const request = transport(owner, mode);
  render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  if (mode === "edit") { expect(screen.queryByRole("button", { name: "通过 FreeBuff 登录" })).not.toBeInTheDocument(); await click("重新授权此账号"); expect(screen.getByText("stable-id")).toBeInTheDocument(); }
  await click("通过 FreeBuff 登录");
  expect(request.mock.calls[0][2].body).toEqual({ providerId: "renamed-provider", expectedRevision: "saved-r1", credentialId: mode === "edit" ? "stable-id" : undefined });
  expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument(); await click("检查授权");
  expect(owner.refresh).not.toHaveBeenCalled(); await click("保存到凭证池");
  expect(owner.refresh).toHaveBeenCalledOnce(); expect(owner.setCredentialDialogState).toHaveBeenCalledWith(null); expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled();
  expect(request.mock.calls.filter(([path]) => path.endsWith("/import"))).toHaveLength(1);
});

it("preserves ordinary stable-ID Keep editing and blocks reauthorization with unsubmitted model edits", () => {
  const owner = controller("edit"); render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "stable-id", enabled: false, apiKeyOperation: "keep" }));
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "new-model" } });
  expect(screen.getByRole("button", { name: "重新授权此账号" })).toBeDisabled();
});

it("cannot bypass an unknown save via manual mode or close/reopen, and management identity changes isolate receipts", async () => {
  vi.useFakeTimers(); const owner = controller(); const request = transport(owner, "add", true);
  const view = render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  await click("通过 FreeBuff 登录"); await click("检查授权"); await click("保存到凭证池");
  expect(screen.getByLabelText("录入方式")).toBeDisabled(); expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
  const dialog = owner.credentialDialogState; owner.credentialDialogState = null; view.rerender(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  owner.credentialDialogState = dialog; view.rerender(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  expect(screen.getByLabelText("录入方式")).toBeDisabled(); expect(screen.getByRole("button", { name: "通过 FreeBuff 登录" })).toBeDisabled();
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled(); expect(owner.refresh).not.toHaveBeenCalled(); expect(request.mock.calls.filter(([path]) => path.endsWith("/import"))).toHaveLength(1);
  owner.session = { ...owner.session, managementToken: "different-owner" }; view.rerender(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  expect(screen.getByRole("button", { name: "通过 FreeBuff 登录" })).toBeEnabled();
});

it("reads a stored catalog through the normal edit host without applying models or submitting secrets", async () => {
  const owner = controller("edit");
  const request = vi.fn(async (_path, schema, options) => schema.parse({ providerId: options.body.providerId,
    credentialId: options.body.credentialId, businessOrigin: options.body.businessOrigin, client: options.body.client,
    revision: "saved-r1", models: [{ key: "stable", displayName: "Catalog model", tagline: "", warning: null,
      badges: [], multimodal: false, premium: false, dataUse: "service", access: "locked" }] }));
  owner.client = { request }; render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  expect(request).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("业务站点"), { target: { value: "https://www.codebuff.com" } });
  fireEvent.change(screen.getByLabelText("客户端"), { target: { value: "desktop" } }); await click("读取目录");
  expect(request).toHaveBeenCalledOnce(); expect(request.mock.calls[0][2].body).toEqual({ providerId: "renamed-provider", credentialId: "stable-id",
    expectedRevision: "saved-r1", businessOrigin: "https://www.codebuff.com", client: "desktop" });
  expect(screen.getByText("Catalog model")).toBeInTheDocument(); expect(screen.getByLabelText("手动模型列表")).toHaveValue("");
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled(); expect(owner.refresh).not.toHaveBeenCalled();
});
it("stages only explicitly selected stable catalog keys and the business audience, then uses normal account save", async () => {
  const owner = controller("edit"); owner.credentialDialogState!.initialValue!.supportedModelsText = "manual-model";
  owner.client = { request: vi.fn(async (_path, schema, options) => schema.parse({
    providerId: "renamed-provider", credentialId: "stable-id", revision: "saved-r1",
    businessOrigin: "https://codebuff.com", client: "cli", models: [
      { key: "stable", displayName: "Catalog model", tagline: "", warning: null, badges: [], multimodal: false, premium: false, dataUse: "service", access: "open" },
      { key: "locked", displayName: "Locked model", tagline: "", warning: null, badges: [], multimodal: false, premium: true, dataUse: "training", access: "locked" },
    ],
  })) };
  render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  fireEvent.change(screen.getByLabelText("业务站点"), { target: { value: "https://codebuff.com" } });
  fireEvent.change(screen.getByLabelText("客户端"), { target: { value: "cli" } }); await click("读取目录");
  expect(screen.getByRole("checkbox", { name: "选择 locked" })).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox", { name: "选择 stable" })); await click("应用到账号");
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled();
  expect(screen.getByRole("alertdialog", { name: "确认目录模式" })).toBeInTheDocument(); await click("确认应用");
  expect(screen.getByLabelText("手动模型列表")).toHaveValue("manual-model");
  expect(screen.getByLabelText("拉取模型")).toHaveValue("stable");
  expect(screen.getByRole("button", { name: "读取目录" })).toBeDisabled(); await click("保存");
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({
    credentialId: "stable-id", apiKeyOperation: "keep", apiKeyValue: "", fetchedModelsText: "stable", manualModelsText: "manual-model",
    freebuffBusiness: { origin: "https://codebuff.com", client: "cli" },
  }));
  expect(owner.client.request).toHaveBeenCalledOnce();
});
it.each(["editorLocked", "draftDirty", "draftMatchesActiveRevision", "autosavePending"] as const)("blocks catalog in unsafe host state: %s", (field) => {
  const owner = controller("edit"); owner[field] = field !== "draftMatchesActiveRevision";
  render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  expect(screen.getByRole("button", { name: "读取目录" })).toBeDisabled(); expect(owner.client.request).not.toHaveBeenCalled();
});
it.each([["add", "freebuff"], ["edit", "xai-api"]])("does not mount stored FreeBuff tools for %s/%s", (mode,preset) => {
  render(<NeuroProvider><ConsoleGlobalDialogs controller={controller(mode,preset)} /></NeuroProvider>);
  expect(screen.queryByRole("region", { name: "FreeBuff 模型目录" })).not.toBeInTheDocument();
});

function savedBusinessOwner() {
  const owner = controller("edit");
  owner.draftDocumentState.document!.providers[0] = { id: "renamed-provider", preset: "freebuff", credentials: [{ id: "stable-id",
    extra_body: { freebuffBusiness: { origin: "https://codebuff.com", client: "cli" } } }] };
  return owner;
}
it("reads pending request purchases through the normal saved-account host without buying or saving", async () => {
  const owner = savedBusinessOwner();
  const request = vi.mocked(owner.client.request);
  request.mockImplementation(async (_path, schema) => schema.parse({ revision: "saved-r1", providerId: "renamed-provider", credentialId: "stable-id", pending: [] }));
  render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  expect(request).not.toHaveBeenCalled(); await click("读取待确认购买");
  expect(request).toHaveBeenCalledExactlyOnceWith("/v1/internal/gateway/console/freebuff-pending-purchases", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner", secretGrant: "grant", signal: expect.any(AbortSignal),
    body: { providerId: "renamed-provider", credentialId: "stable-id", expectedRevision: "saved-r1" },
  }));
  expect(screen.getByText("无待确认购买")).toBeInTheDocument();
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled(); expect(owner.refresh).not.toHaveBeenCalled();
});
it.each([["查看并批准", "批准此次购买", "approve"], ["取消此次购买", "确认取消购买", "cancel"]])(
  "delivers %s only after the normal host shows and explicitly confirms the original quote", async (review, confirm, decision) => {
    const owner = savedBusinessOwner(); const request = vi.mocked(owner.client.request);
    request.mockImplementation(async (path, schema) => schema.parse(path.endsWith("/decision") ? { delivered: true }
      : { revision: "saved-r1", providerId: "renamed-provider", credentialId: "stable-id", pending: [pendingPurchase()] }));
    render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
    await click("读取待确认购买"); await click(review); expect(request).toHaveBeenCalledOnce();
    expect(screen.getByRole("alertdialog")).toHaveTextContent("预计钱包支出"); await click(confirm);
    expect(request.mock.calls[1][2]!.body).toEqual({ providerId: "renamed-provider", credentialId: "stable-id", expectedRevision: "saved-r1",
      pendingId: pendingPurchase().pendingId, quoteId: pendingPurchase().quoteId, decision });
    expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled(); expect(owner.refresh).not.toHaveBeenCalled();
  });
it.each(["editorLocked", "draftDirty", "draftMatchesActiveRevision", "autosavePending"] as const)(
  "blocks pending purchases in unsafe normal host state: %s", field => {
    const owner = savedBusinessOwner(); owner[field] = field !== "draftMatchesActiveRevision";
    render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
    expect(screen.getByRole("button", { name: "读取待确认购买" })).toBeDisabled(); expect(owner.client.request).not.toHaveBeenCalled();
  });
it("invalidates pending confirmation when ordinary account fields have unsubmitted edits", async () => {
  const owner = savedBusinessOwner(); vi.mocked(owner.client.request).mockResolvedValue({ revision: "saved-r1", providerId: "renamed-provider", credentialId: "stable-id", pending: [pendingPurchase()] });
  render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
  await click("读取待确认购买"); await click("查看并批准"); await click("返回");
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "unsaved-model" } });
  expect(screen.queryByRole("button", { name: "查看并批准" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "读取待确认购买" })).toBeDisabled(); expect(owner.client.request).toHaveBeenCalledOnce();
});
