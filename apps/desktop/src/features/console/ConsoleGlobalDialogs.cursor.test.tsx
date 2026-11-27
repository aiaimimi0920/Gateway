import type { ComponentProps } from "react";
import { render, fireEvent, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { beforeEach, expect, it, vi } from "vitest";
import { ConsoleGlobalDialogs } from "./ConsoleGlobalDialogs";
import type { CursorImportPanel } from "./CursorImportPanel";
const capture = vi.hoisted(() => ({ panel: vi.fn() }));
vi.mock("./CursorImportPanel", () => ({ CursorImportPanel: capture.panel }));
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
vi.mock("./AccountGroupDialog", () => ({ AccountGroupDialog: () => null }));
vi.mock("./ProviderCatalogDialog", () => ({ ProviderCatalogDialog: () => null }));
vi.mock("./CredentialPruneDialog", () => ({ CredentialPruneDialog: () => null }));
vi.mock("./GeminiManualAddDialog", () => ({ GeminiManualAddDialog: () => null }));
vi.mock("./PilotActionDialog", () => ({ PilotActionDialog: () => null }));
vi.mock("./useCredentialKeyReveal", () => ({ useCredentialKeyReveal: () => undefined }));
vi.mock("./useChatgptWebBrowserCapture", () => ({ useChatgptWebBrowserCapture: () => undefined }));
type Controller = ComponentProps<typeof ConsoleGlobalDialogs>["controller"];
function controller(mode = "add", configured = true): Controller {
  const provider = { id: "renamed", auth_mode: "cursor_remote", credentials: [
    { id: "stable", account_name: "owner@example.test", enabled: false, expires_at: "2099-01-01T00:00:00Z" },
  ] };
  return { client: { request: vi.fn() }, session: { managementToken: "owner", secretGrant: { grant: "grant" }, confirmSecretAccess: vi.fn() },
    routeConfig: { routeConfig: { revision: { id: "saved-r1" }, document: { providers: [provider] },
      secrets: [{ path: "/providers/0/credentials/0/auth_token", configured }] } },
    draftDocumentState: { document: { providers: [provider] } },
    credentialDialogState: { mode, initialValue: { providerId: "renamed", credentialId: mode === "edit" ? "stable" : "", accountName: "", enabled: false,
      baseUrl: "", supportedModelsText: "", apiKeyOperation: mode === "edit" ? "keep" : "replace", apiKeyValue: "" } },
    credentialProviderOptions: [{ id: "renamed", label: "Renamed" }], explicitCredentialIds: [], displayedAccountCatalog: { accounts: [] }, groupDirectory: [],
    editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true, hasSecretAccess: true, setCredentialDialogState: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}
const latest = () => capture.panel.mock.calls.at(-1)![0] as ComponentProps<typeof CursorImportPanel>;
beforeEach(() => { vi.clearAllMocks(); capture.panel.mockImplementation(() => <div data-testid="cursor-panel" />); });
it("uses Cursor managed import in add dialog and preserves operations across reopen", () => {
  const owner = controller(), view = render(<ConsoleGlobalDialogs controller={owner} />, { wrapper: NeuroProvider });
  expect(latest()).toMatchObject({ providerId: "renamed", expectedRevision: "saved-r1", disabled: false });
  const operations = latest().operations;
  view.rerender(<ConsoleGlobalDialogs controller={{ ...owner, credentialDialogState: null }} />); view.rerender(<ConsoleGlobalDialogs controller={owner} />);
  expect(latest().operations).toBe(operations);
  latest().onSaved(); expect(owner.refresh).toHaveBeenCalledOnce(); expect(owner.setCredentialDialogState).not.toHaveBeenCalled();
});
it.each([true, false])("edit binds the original account and uses actual secret descriptor: %s", configured => {
  render(<ConsoleGlobalDialogs controller={controller("edit", configured)} />, { wrapper: NeuroProvider });
  expect(screen.queryByTestId("cursor-panel")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "重新授权此账号" }));
  expect(latest().credentialId).toBe("stable");
  expect(latest().savedAccount?.hasToken).toBe(configured);
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
});
