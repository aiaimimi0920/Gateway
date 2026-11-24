// Exercise the actual global dialog wiring; only the final save callback is synthetic.
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
    draftDocumentState: { document: { providers: [{ id: "renamed", preset: "lumalabs-api", base_url: "https://agents.lumalabs.ai/v1" }] } },
    credentialDialogState: { mode: edit ? "edit" : "add", initialValue: { providerId: "renamed", credentialId: edit ? "stable" : "",
      accountName: "Preserved", enabled: false, baseUrl: "", supportedModelsText: "uni-1", apiKeyOperation: edit ? "keep" : "replace", apiKeyValue: "" } },
    credentialProviderOptions: [{ id: "renamed", label: "Renamed" }], explicitCredentialIds: [], displayedAccountCatalog: { accounts: [] }, groupDirectory: [],
    editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true, hasSecretAccess: true, setSecretDialogOpen: vi.fn(),
    setCredentialDialogState: vi.fn(), applyCredentialDialogValue: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}
const mount = (owner: Controller) => render(<NeuroProvider><ConsoleGlobalDialogs controller={owner} /></NeuroProvider>);
const save = () => fireEvent.click(screen.getByRole("button", { name: "保存" }));
it("adds an API Key with exact manual models and no OAuth, Web import or directory controls", () => {
  const owner = controller(); const view = mount(owner);
  expect(screen.queryByRole("button", { name: /登录|重新授权|刷新协议|拉取模型/ })).not.toBeInTheDocument();
  expect(screen.queryByLabelText("拉取模型列表")).not.toBeInTheDocument(); expect(view.container.querySelector(".nt-credential-model-status")).toBeNull();
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "stable" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "luma-api-synthetic-only" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "uni-1\nray-3.2" } }); save();
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "stable", supportedModelsText: "uni-1\nray-3.2",
    apiKeyOperation: "replace", apiKeyValue: "luma-api-synthetic-only" })); expect(owner.client.request).not.toHaveBeenCalled();
});
it("exposes Files only for an existing saved Agents account and locks it for local edits", () => {
  const owner = controller(true); owner.credentialDialogState!.initialValue!.enabled = true; mount(owner);
  expect(screen.getByRole("region", { name: "Luma 文件" })).toBeInTheDocument();
  expect(screen.getByRole("region", { name: "Luma 任务查询" })).toBeInTheDocument();
  expect(screen.getByLabelText("待查询任务 ID")).toBeEnabled();
  expect(screen.getByRole("button", { name: "读取文件列表" })).toBeEnabled();
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "ray-3.2" } });
  expect(screen.getByRole("button", { name: "读取文件列表" })).toBeDisabled();
  expect(screen.getByLabelText("待查询任务 ID")).toBeDisabled();
  expect(owner.client.request).not.toHaveBeenCalled();
});
it("disables Files for a saved disabled account", () => {
  mount(controller(true)); expect(screen.getByRole("button", { name: "读取文件列表" })).toBeDisabled();
});
it.each(["uni-unknown", "UNI-1", "models/uni-1", "uni-1*", "ray3.14"]) ("rejects a model outside the implemented Agents contract: %s", (model) => {
  const owner = controller(true); mount(owner); fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: model } }); save();
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled(); expect(screen.getByText(/Luma Agents.*模型/)).toBeInTheDocument();
});
it.each(["keep", "replace", "clear"])("uses ordinary %s with stable ID and metadata", (operation) => {
  const owner = controller(true); mount(owner);
  if (operation !== "keep") fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "luma-api-synthetic-new" } });
  if (operation === "clear") fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "" } }); save();
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "stable", accountName: "Preserved", enabled: false,
    apiKeyOperation: operation, apiKeyValue: operation === "replace" ? "luma-api-synthetic-new" : "" }));
  expect(owner.client.request).not.toHaveBeenCalled();
});
it("allows metadata Keep without secret access and requires it for adding material", () => {
  const owner = controller(true); owner.hasSecretAccess = false; const view = mount(owner); save();
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({ apiKeyOperation: "keep" })); view.unmount();
  const adding = controller(); adding.hasSecretAccess = false; mount(adding); save();
  expect(adding.setSecretDialogOpen).toHaveBeenCalledWith(true); expect(adding.applyCredentialDialogValue).not.toHaveBeenCalled();
});
