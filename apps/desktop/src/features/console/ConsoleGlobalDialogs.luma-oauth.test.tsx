import type { ComponentProps } from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
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
function controller(mode = "add"): Controller {
  return { client: { request: vi.fn() }, session: { managementToken: "owner", secretGrant: { grant: "grant" }, confirmSecretAccess: vi.fn() },
    routeConfig: { routeConfig: { revision: { id: "r1" } } }, draftDocumentState: { document: { providers: [{ id: "renamed",
      adapter: "lumalabs_mcp_readonly", protocol_profile: "lumalabs_mcp_readonly", auth_mode: "luma_mcp_oauth",
      base_url: "https://agents.lumalabs.ai/mcp", credentials: [{ id: "stable", luma_oauth: {
        client_id: "registered-client", redirect_uri: "http://127.0.0.1:43567/callback" } }] }] } },
    credentialDialogState: { mode, initialValue: { providerId: "renamed", credentialId: mode === "edit" ? "stable" : "",
      accountName: "", enabled: false, baseUrl: "", supportedModelsText: "", apiKeyOperation: "keep", apiKeyValue: "" } },
    credentialProviderOptions: [{ id: "renamed", label: "Renamed" }], explicitCredentialIds: [], displayedAccountCatalog: { accounts: [] }, groupDirectory: [],
    editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true, hasSecretAccess: true,
    setCredentialDialogState: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}
it("opens a dedicated authorization entry for an existing MCP provider without API-key or model editors", () => {
  render(<ConsoleGlobalDialogs controller={controller()} />, { wrapper: NeuroProvider });
  expect(screen.getByRole("button", { name: "通过 Luma 登录" })).toBeDisabled();
  expect(screen.getByLabelText("Client ID")).toHaveValue("");
  expect(screen.queryByText("API Key")).not.toBeInTheDocument();
});
it("reauthorizes with stored registration and blocks a dirty configuration", () => {
  const owner = controller("edit");
  const view = render(<ConsoleGlobalDialogs controller={owner} />, { wrapper: NeuroProvider });
  expect(screen.getByLabelText("Client ID")).toHaveValue("registered-client");
  expect(screen.getByLabelText("Client ID")).toBeDisabled();
  expect(screen.getByLabelText("已登记的回调地址")).toHaveValue("http://127.0.0.1:43567/callback");
  expect(screen.getByRole("button", { name: "通过 Luma 登录" })).toBeEnabled();
  view.rerender(<ConsoleGlobalDialogs controller={{ ...owner, draftDirty: true }} />);
  expect(screen.getByRole("button", { name: "通过 Luma 登录" })).toBeDisabled();
});
it("supports selecting MCP from the generic add-account dialog without a second save form", () => {
  const owner = controller();
  owner.credentialDialogState!.initialValue!.providerId = "";
  render(<ConsoleGlobalDialogs controller={owner} />, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("Provider"), { target: { value: "renamed" } });
  expect(screen.getByRole("button", { name: "通过 Luma 登录" })).toBeDisabled();
  expect(screen.queryByText("API Key")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
});
it("submits the original credential ID for reauthorization", async () => {
  const owner = controller("edit");
  vi.mocked(owner.client.request).mockImplementation(async (_path, schema) => schema.parse({ session: {
    id: "session", providerId: "renamed", expectedRevision: "r1", status: "waiting_user",
    authorizationUrl: "https://agents.lumalabs.ai/mcp/oauth/authorize?state=synthetic", redirectUri: "http://127.0.0.1:43567/callback",
    credentialId: null, targetCredentialId: "stable", savedRevision: null, errorCode: null,
  } }));
  render(<ConsoleGlobalDialogs controller={owner} />, { wrapper: NeuroProvider });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 Luma 登录" })); });
  expect(owner.client.request).toHaveBeenCalledWith(expect.stringContaining("/luma-auth-sessions"), expect.anything(), expect.objectContaining({
    body: { providerId: "renamed", expectedRevision: "r1", credentialId: "stable", clientId: "registered-client", redirectUri: "http://127.0.0.1:43567/callback" },
  }));
});
