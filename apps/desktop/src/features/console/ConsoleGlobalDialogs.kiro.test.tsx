import type { ComponentProps } from "react";
import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ConsoleGlobalDialogs } from "./ConsoleGlobalDialogs";
import type { GatewayApiRequestOptions } from "../../api/client";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
vi.mock("./AccountGroupDialog", () => ({ AccountGroupDialog: () => null }));
vi.mock("./ProviderCatalogDialog", () => ({ ProviderCatalogDialog: () => null }));
vi.mock("./CredentialPruneDialog", () => ({ CredentialPruneDialog: () => null }));
vi.mock("./GeminiManualAddDialog", () => ({ GeminiManualAddDialog: () => null }));
vi.mock("./PilotActionDialog", () => ({ PilotActionDialog: () => null }));
vi.mock("./useCredentialKeyReveal", () => ({ useCredentialKeyReveal: () => undefined }));
vi.mock("./useChatgptWebBrowserCapture", () => ({ useChatgptWebBrowserCapture: () => undefined }));
type Controller = ComponentProps<typeof ConsoleGlobalDialogs>["controller"];
function controller(mode = "add", preset = "kiro"): Controller {
  return {
    client: { request: vi.fn() }, session: { managementToken: "owner", secretGrant: { grant: "grant" }, confirmSecretAccess: vi.fn() },
    routeConfig: { routeConfig: { revision: { id: "saved-r1" } } },
    draftDocumentState: {
      document: {
        providers: [{
          id: "renamed-provider", preset,
          base_url: preset === "kiro-api-key" ? "https://runtime.us-east-1.kiro.dev" : "https://q.us-east-1.amazonaws.com"
        }]
      }
    },
    credentialDialogState: {
      mode, initialValue: {
        providerId: "renamed-provider", credentialId: mode === "edit" ? "stable-id" : "", accountName: "Preserved", enabled: false,
        baseUrl: "", supportedModelsText: "", apiKeyOperation: mode === "edit" ? "keep" : "replace", apiKeyValue: ""
      }
    },
    credentialProviderOptions: [{ id: "renamed-provider", label: "Renamed" }], explicitCredentialIds: [],
    displayedAccountCatalog: { accounts: [] }, groupDirectory: [], editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true,
    hasSecretAccess: true, setSecretDialogOpen: vi.fn(), setCredentialDialogState: vi.fn(), applyCredentialDialogValue: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}
const mount = (owner: Controller) => render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
function fill(editing = false) {
  fireEvent.change(screen.getByLabelText("授权目录"), { target: { value: "builder_id" } });
  fireEvent.click(screen.getByLabelText("codewhisperer:conversations"));
  if (!editing) fireEvent.change(screen.getByLabelText("Machine ID"), { target: { value: "a".repeat(64) } });
}

it("offers Kiro device authorization by exact preset without an API-key creation form", () => {
  mount(controller());
  expect(screen.getByRole("button", { name: "通过 Kiro 登录" })).toBeInTheDocument();
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
});

it.each(["add", "edit"])("uses the real Kiro API contract and explicitly polls and saves once: %s", async (mode) => {
  const owner = controller(mode);
  const request = vi.fn(async (path, schema, _options?: GatewayApiRequestOptions) => {
    const status = path.endsWith("/import") ? "succeeded" : path.endsWith("/poll") ? "ready" : "waiting_user";
    return schema.parse({
      session: {
        id: "login", providerId: "renamed-provider", expectedRevision: "saved-r1", status,
        verificationUri: "https://device.sso.us-east-1.amazonaws.com/", verificationUriComplete: null, userCode: "ABCD-EFGH",
        pollIntervalSecs: 5, expiresInSecs: 600, credentialId: status === "succeeded" ? mode === "edit" ? "stable-id" : "new-id" : null,
        targetCredentialId: mode === "edit" ? "stable-id" : null, savedRevision: status === "succeeded" ? "saved-r2" : null, errorCode: null
      }
    });
  });
  owner.client = { request }; mount(owner);
  if (mode === "edit") await click("重新授权此账号");
  fill(mode === "edit"); await click("通过 Kiro 登录");
  expect(request.mock.calls[0][0]).toBe("/v1/internal/gateway/console/kiro-auth-sessions");
  expect(request.mock.calls[0][2]).toEqual(expect.objectContaining({
    body: {
      providerId: "renamed-provider", expectedRevision: "saved-r1", credentialId: mode === "edit" ? "stable-id" : undefined,
      directory: { authMethod: "builder_id" }, scopes: ["codewhisperer:conversations"], ...(mode === "edit" ? {} : { machineId: "a".repeat(64) })
    }
  }));
  expect(screen.getByLabelText("用户授权码")).toHaveValue("ABCD-EFGH");
  expect(owner.refresh).not.toHaveBeenCalled(); await click("检查授权");
  expect(owner.refresh).not.toHaveBeenCalled(); await click("保存到凭证池");
  expect(owner.refresh).toHaveBeenCalledOnce(); expect(owner.setCredentialDialogState).toHaveBeenCalledWith(null);
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled();
  expect(request.mock.calls.filter(([path]) => path.endsWith("/poll"))).toHaveLength(1);
  expect(request.mock.calls.filter(([path]) => path.endsWith("/import"))).toHaveLength(1);
});

it("retains normal stable-ID metadata editing without a bearer/API-key replacement field", async () => {
  const owner = controller("edit"); mount(owner);
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  await click("保存");
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "stable-id", enabled: false, apiKeyOperation: "keep" }));
});

it.each(["editorLocked", "draftDirty", "draftMatchesActiveRevision"] as const)("blocks new Kiro authorization when %s is unsafe", (field) => {
  const owner = controller(); owner[field] = field !== "draftMatchesActiveRevision"; mount(owner);
  expect(screen.getByLabelText("授权目录")).toBeDisabled(); expect(screen.getByRole("button", { name: "通过 Kiro 登录" })).toBeDisabled();
});

it("does not attach Kiro authorization to an unrelated preset or a guessed API-key candidate", () => {
  mount(controller("add", "kiro-api"));
  expect(screen.queryByRole("button", { name: "通过 Kiro 登录" })).not.toBeInTheDocument();
});
it("adds a native Kiro Key with an exact model through the normal form, never OAuth or discovery", async () => {
  const owner = controller("add", "kiro-api-key"); mount(owner);
  expect(screen.queryByRole("button", { name: "通过 Kiro 登录" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "重新授权此账号" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "刷新协议" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-key" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "synthetic-key" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "Exact.Model/Version" } });
  await click("保存");
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({
    providerId: "renamed-provider",
    credentialId: "new-key", apiKeyOperation: "replace", apiKeyValue: "synthetic-key", supportedModelsText: "Exact.Model/Version"
  }));
  expect(owner.client.request).not.toHaveBeenCalled();
});
it.each(["keep", "replace", "clear"])("edits a stable native Key account using normal %s semantics", async (operation) => {
  const owner = controller("edit", "kiro-api-key"); mount(owner);
  expect(screen.queryByRole("button", { name: "重新授权此账号" })).not.toBeInTheDocument();
  if (operation !== "keep") fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "synthetic-new" } });
  if (operation === "clear") fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "" } });
  await click("保存");
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({
    credentialId: "stable-id",
    enabled: false, accountName: "Preserved", apiKeyOperation: operation, apiKeyValue: operation === "replace" ? "synthetic-new" : ""
  }));
  expect(owner.client.request).not.toHaveBeenCalled();
});
it("requires secret access for native Key replacement and never reveals a saved Key automatically", async () => {
  const owner = controller("add", "kiro-api-key"); owner.hasSecretAccess = false; mount(owner);
  expect(screen.getByLabelText("API Key")).toBeDisabled();
  await click("保存");
  expect(owner.setSecretDialogOpen).toHaveBeenCalledWith(true);
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled();
  expect(owner.client.request).not.toHaveBeenCalled();
});
