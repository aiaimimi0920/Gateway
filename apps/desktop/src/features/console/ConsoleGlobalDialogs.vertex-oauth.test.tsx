// Real GlobalDialogs/CredentialDialog/panel/API schema; request transport and persistence are synthetic.
import type { ComponentProps } from "react";
import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render, screen } from "@testing-library/react";
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
function controller(mode = "add", typed = true): Controller {
  return {
    client: { request: vi.fn() }, session: { managementToken: "owner", secretGrant: { grant: "grant" }, confirmSecretAccess: vi.fn() },
    routeConfig: { routeConfig: { revision: { id: "saved-r1" } } },
    draftDocumentState: { document: { providers: [{ id: "renamed", preset: "google-agent-platform-official-api",
      auth_mode: typed ? "vertex_user_oauth" : null,
      base_url: "https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google",
      credentials: [{ id: "stable-id", vertex_oauth: { target: { project: "project-a", location: "global", quota_project: "saved-quota" } } }] }] } },
    credentialDialogState: { mode, initialValue: { providerId: "renamed", credentialId: mode === "edit" ? "stable-id" : "", accountName: "", enabled: false,
      baseUrl: "", supportedModelsText: "gemini-contract", manualModelsText: "gemini-contract", fetchedModelsText: "", apiKeyOperation: mode === "edit" ? "keep" : "replace", apiKeyValue: "" } },
    credentialProviderOptions: [{ id: "renamed", label: "Renamed" }], explicitCredentialIds: [], displayedAccountCatalog: { accounts: [] }, groupDirectory: [],
    editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true, hasSecretAccess: true,
    setCredentialDialogState: vi.fn(), applyCredentialDialogValue: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}
const mount = (owner: Controller) => render(<ConsoleGlobalDialogs controller={owner} />, { wrapper: NeuroProvider });
it("offers the independent sign-in only for typed Full Vertex, even with a renamed provider", () => {
  const view = mount(controller()); expect(screen.getByRole("button", { name: "通过 Vertex 登录" })).toBeInTheDocument();
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument(); expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
  view.unmount(); mount(controller("add", false)); expect(screen.queryByRole("button", { name: "通过 Vertex 登录" })).not.toBeInTheDocument();
});
it("preserves metadata Keep without secret access and separates stable-ID reauthorization", () => {
  const owner = controller("edit"); owner.hasSecretAccess = false; mount(owner);
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument(); expect(screen.queryByRole("button", { name: "拉取模型列表" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "stable-id", apiKeyOperation: "keep", enabled: false }));
  fireEvent.click(screen.getByRole("button", { name: "重新授权此账号" }));
  expect(screen.getByLabelText("重新授权目标账号")).toHaveValue("stable-id"); expect(screen.getByLabelText("Quota Project（可选）")).toHaveValue("saved-quota");
});
it.each(["editorLocked", "draftDirty", "draftMatchesActiveRevision"] as const)("locks authorization when %s is unsafe", (field) => {
  const owner = controller(); owner[field] = field !== "draftMatchesActiveRevision"; mount(owner);
  expect(screen.getByLabelText("Client ID")).toBeDisabled(); expect(screen.getByRole("button", { name: "通过 Vertex 登录" })).toBeDisabled();
});
it.each(["add", "edit"])("uses the normal API once and refreshes only after confirmed save: %s", async (mode) => {
  const owner = controller(mode);
  const request = vi.fn(async (path, schema) => {
    const status = path.endsWith("/import") ? "succeeded" : path.endsWith("/complete") ? "ready" : "waiting_user";
    return schema.parse({ session: { id: "login", providerId: "renamed", expectedRevision: "saved-r1", status,
      authorizationUrl: "https://accounts.google.com/o/oauth2/v2/auth?state=synthetic", redirectUri: "http://127.0.0.1:43567/",
      credentialId: status === "succeeded" ? mode === "edit" ? "stable-id" : "new-id" : null, targetCredentialId: mode === "edit" ? "stable-id" : null,
      savedRevision: status === "succeeded" ? "saved-r2" : null, errorCode: null } });
  });
  owner.client = { request }; mount(owner);
  if (mode === "edit") fireEvent.click(screen.getByRole("button", { name: "重新授权此账号" }));
  fireEvent.change(screen.getByLabelText("Client ID"), { target: { value: "explicit-client" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 Vertex 登录" })); });
  expect(request.mock.calls[0][0]).toBe("/v1/internal/gateway/console/vertex-auth-sessions");
  fireEvent.change(screen.getByLabelText("手动回调地址"), { target: { value: "http://127.0.0.1:43567/?code=private" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "提交手动回调" })); }); expect(owner.refresh).not.toHaveBeenCalled();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "保存到凭证池" })); });
  expect(owner.refresh).toHaveBeenCalledOnce(); expect(owner.setCredentialDialogState).toHaveBeenCalledWith(null);
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled(); expect(request.mock.calls.filter(([path]) => path.endsWith("/import"))).toHaveLength(1);
});
it("blocks reauthorization after local model edits instead of silently discarding them", () => {
  mount(controller("edit")); fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "gemini-other" } });
  expect(screen.getByRole("button", { name: "重新授权此账号" })).toBeDisabled();
});
