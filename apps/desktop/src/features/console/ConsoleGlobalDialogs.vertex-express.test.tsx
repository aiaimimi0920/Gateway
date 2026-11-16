// Use the real global dialog and product form; only the final save callback is synthetic.
import type { ComponentProps } from "react";
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ConsoleGlobalDialogs } from "./ConsoleGlobalDialogs";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
vi.mock("./AccountGroupDialog", () => ({ AccountGroupDialog: () => null }));
vi.mock("./ProviderCatalogDialog", () => ({ ProviderCatalogDialog: () => null }));
vi.mock("./CredentialPruneDialog", () => ({ CredentialPruneDialog: () => null }));
vi.mock("./GeminiManualAddDialog", () => ({ GeminiManualAddDialog: () => null }));
vi.mock("./PilotActionDialog", () => ({ PilotActionDialog: () => null }));
vi.mock("./useCredentialKeyReveal", () => ({ useCredentialKeyReveal: () => undefined }));
vi.mock("./useChatgptWebBrowserCapture", () => ({ useChatgptWebBrowserCapture: () => undefined }));
type Controller = ComponentProps<typeof ConsoleGlobalDialogs>["controller"];
function controller(edit = false): Controller {
  return {
    client: { request: vi.fn() }, session: { managementToken: "owner", secretGrant: { grant: "grant" }, confirmSecretAccess: vi.fn() },
    routeConfig: { routeConfig: { revision: { id: "saved-r1" } } },
    draftDocumentState: { document: { providers: [{ id: "renamed", preset: "vertex-express-api",
      base_url: "https://aiplatform.googleapis.com/v1/publishers/google" }] } },
    credentialDialogState: { mode: edit ? "edit" : "add", initialValue: {
      providerId: "renamed", credentialId: edit ? "stable" : "", accountName: "Preserved", enabled: false, baseUrl: "",
      supportedModelsText: "gemini-contract", apiKeyOperation: edit ? "keep" : "replace", apiKeyValue: "" } },
    credentialProviderOptions: [{ id: "renamed", label: "Renamed" }], explicitCredentialIds: [],
    displayedAccountCatalog: { accounts: [] }, groupDirectory: [], editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true,
    hasSecretAccess: true, setSecretDialogOpen: vi.fn(), setCredentialDialogState: vi.fn(), applyCredentialDialogValue: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}
function mount(owner: Controller) { return render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>); }
const save = () => fireEvent.click(screen.getByRole("button", { name: "保存" }));
it("adds an exact bounded resource segment without OAuth, directory reads or silent truncation", () => {
  const owner = controller(); const view = mount(owner);
  expect(screen.queryByRole("button", { name: /登录|重新授权|刷新协议|拉取模型/ })).not.toBeInTheDocument();
  expect(view.container.querySelector(".nt-credential-model-status")).toBeNull();
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "stable" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "synthetic-only" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "A".repeat(2048) } }); save();
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({
    credentialId: "stable", supportedModelsText: "A".repeat(2048), apiKeyOperation: "replace", apiKeyValue: "synthetic-only" }));
  expect(owner.client.request).not.toHaveBeenCalled();
});
it.each(["models/foo", "x%2fsecret", "x#f", "x:generateContent", "A".repeat(2049)])("rejects a non-segment model before submitting: %s", (model) => {
  const owner = controller(true); mount(owner);
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: model } }); save();
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled(); expect(owner.client.request).not.toHaveBeenCalled();
  expect(screen.getByText(/模型 ID 无效/)).toBeInTheDocument();
});
it.each(["keep", "replace", "clear"])("preserves stable account metadata with normal %s semantics", (operation) => {
  const owner = controller(true); mount(owner);
  if (operation !== "keep") fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "synthetic-new" } });
  if (operation === "clear") fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "" } }); save();
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "stable", accountName: "Preserved", enabled: false,
    apiKeyOperation: operation, apiKeyValue: operation === "replace" ? "synthetic-new" : "" }));
  expect(owner.client.request).not.toHaveBeenCalled();
});
it("allows metadata Keep without revealing Key but requires secret access to add material", () => {
  const owner = controller(true); owner.hasSecretAccess = false; const view = mount(owner); save();
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({ apiKeyOperation: "keep" }));
  expect(owner.client.request).not.toHaveBeenCalled(); view.unmount();
  const adding = controller(); adding.hasSecretAccess = false; mount(adding); save();
  expect(adding.setSecretDialogOpen).toHaveBeenCalledWith(true); expect(adding.applyCredentialDialogValue).not.toHaveBeenCalled();
});
