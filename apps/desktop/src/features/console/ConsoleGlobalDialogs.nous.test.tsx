import type { ComponentProps } from "react";
import { render, fireEvent, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { beforeEach, expect, it, vi } from "vitest";
import { ConsoleGlobalDialogs } from "./ConsoleGlobalDialogs";
import type { NousOAuthPanel } from "./NousOAuthPanel";
const capture = vi.hoisted(() => ({ panel: vi.fn() }));
vi.mock("./NousOAuthPanel", () => ({ NousOAuthPanel: capture.panel }));
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
    routeConfig: { routeConfig: { revision: { id: "saved-r1" } } }, draftDocumentState: { document: { providers: [{ id: "renamed", preset: "nous-portal-oauth" }] } },
    credentialDialogState: { mode, initialValue: { providerId: "renamed", credentialId: mode === "edit" ? "stable" : "", accountName: "", enabled: true,
      baseUrl: "", supportedModelsText: "", apiKeyOperation: mode === "edit" ? "keep" : "replace", apiKeyValue: "" } },
    credentialProviderOptions: [{ id: "renamed", label: "Renamed" }], explicitCredentialIds: [], displayedAccountCatalog: { accounts: [] }, groupDirectory: [],
    editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true, hasSecretAccess: true, setCredentialDialogState: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}
const latest = () => capture.panel.mock.calls.at(-1)![0] as ComponentProps<typeof NousOAuthPanel>;
beforeEach(() => { vi.clearAllMocks(); capture.panel.mockImplementation(() => <div data-testid="nous-panel" />); });
it("uses the saved preset rather than mutable provider ID and preserves save receipts on reopening", () => {
  const owner = controller(); const view = render(<ConsoleGlobalDialogs controller={owner} />, { wrapper: NeuroProvider });
  expect(latest()).toMatchObject({ providerId: "renamed", expectedRevision: "saved-r1", disabled: false });
  const operations = latest().operations;
  view.rerender(<ConsoleGlobalDialogs controller={{ ...owner, credentialDialogState: null }} />); view.rerender(<ConsoleGlobalDialogs controller={owner} />);
  expect(latest().operations).toBe(operations);
  view.rerender(<ConsoleGlobalDialogs controller={{ ...owner, draftDirty: true }} />); expect(latest().disabled).toBe(true);
});
it("only starts reauthorization of the stable account after an explicit action", () => {
  render(<ConsoleGlobalDialogs controller={controller("edit")} />, { wrapper: NeuroProvider });
  expect(screen.queryByTestId("nous-panel")).not.toBeInTheDocument(); fireEvent.click(screen.getByRole("button", { name: "重新授权此账号" }));
  expect(latest().credentialId).toBe("stable");
});
