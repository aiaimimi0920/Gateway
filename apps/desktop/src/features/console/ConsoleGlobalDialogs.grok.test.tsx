import type { ComponentProps } from "react";
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render as renderUi, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { ConsoleGlobalDialogs } from "./ConsoleGlobalDialogs";
import type { GrokOAuthProps } from "./useGrokOAuth";

const render = (ui: React.ReactNode) => renderUi(ui, { wrapper: NeuroProvider });

const capture = vi.hoisted(() => ({ panel: vi.fn() }));
vi.mock("./GrokOAuthPanel", () => ({ GrokOAuthPanel: capture.panel }));
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
vi.mock("./AccountGroupDialog", () => ({ AccountGroupDialog: () => null }));
vi.mock("./ProviderCatalogDialog", () => ({ ProviderCatalogDialog: () => null }));
vi.mock("./CredentialPruneDialog", () => ({ CredentialPruneDialog: () => null }));
vi.mock("./GeminiManualAddDialog", () => ({ GeminiManualAddDialog: () => null }));
vi.mock("./PilotActionDialog", () => ({ PilotActionDialog: () => null }));
vi.mock("./useCredentialKeyReveal", () => ({ useCredentialKeyReveal: () => undefined }));
vi.mock("./useChatgptWebBrowserCapture", () => ({ useChatgptWebBrowserCapture: () => undefined }));
type Controller = ComponentProps<typeof ConsoleGlobalDialogs>["controller"];
function controller(mode = "add", preset = "grok-oauth"): Controller {
  return {
    client: { request: vi.fn() }, session: { managementToken: "owner", secretGrant: { grant: "grant" }, confirmSecretAccess: vi.fn() },
    routeConfig: { routeConfig: { revision: { id: "saved-r1" } } },
    draftDocumentState: { document: { providers: [{ id: "renamed-provider", preset, base_url: "https://cli-chat-proxy.grok.com/v1" }] } },
    credentialDialogState: {
      mode, initialValue: {
        providerId: "renamed-provider", credentialId: mode === "edit" ? "stable-id" : "", accountName: "", enabled: false,
        baseUrl: "", supportedModelsText: "", apiKeyOperation: mode === "edit" ? "keep" : "replace", apiKeyValue: ""
      }
    },
    credentialProviderOptions: [{ id: "renamed-provider", label: "Renamed" }], explicitCredentialIds: [],
    displayedAccountCatalog: { accounts: [] }, groupDirectory: [], editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true,
    hasSecretAccess: true, setCredentialDialogState: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}
const latest = () => capture.panel.mock.calls.at(-1)![0] as GrokOAuthProps;
beforeEach(() => { vi.clearAllMocks(); capture.panel.mockImplementation(() => <div data-testid="grok-panel" />); });

it("selects Grok OAuth by saved preset rather than provider ID, and uses the active saved revision", async () => {
  const owner = controller(); render(<ConsoleGlobalDialogs controller={owner} />);
  expect(screen.getByTestId("grok-panel")).toBeInTheDocument();
  expect(latest()).toMatchObject({ providerId: "renamed-provider", expectedRevision: "saved-r1", managementToken: "owner", secretGrant: "grant", disabled: false });
  await latest().onRequestSecretAccess(); expect(owner.session.confirmSecretAccess).toHaveBeenCalledWith("owner");
  latest().onSaved(); expect(owner.setCredentialDialogState).toHaveBeenCalledWith(null); expect(owner.refresh).toHaveBeenCalledOnce();
});

it.each(["editorLocked", "draftDirty", "draftMatchesActiveRevision"] as const)("blocks authorization on unsafe draft state: %s", (field) => {
  const owner = controller(); owner[field] = field !== "draftMatchesActiveRevision";
  render(<ConsoleGlobalDialogs controller={owner} />); expect(latest().disabled).toBe(true);
});

it("keeps operation receipts across dialog close/reopen, but changes owner on management identity replacement", () => {
  const owner = controller(); const view = render(<ConsoleGlobalDialogs controller={owner} />);
  const operations = latest().operations;
  view.rerender(<ConsoleGlobalDialogs controller={{ ...owner, credentialDialogState: null }} />);
  view.rerender(<ConsoleGlobalDialogs controller={owner} />); expect(latest().operations).toBe(operations);
  view.rerender(<ConsoleGlobalDialogs controller={{ ...owner, session: { ...owner.session, managementToken: "new-owner" } }} />);
  expect(latest().operations).not.toBe(operations);
});

it("reauthorizes a stable ID only after the explicit edit action", () => {
  render(<ConsoleGlobalDialogs controller={controller("edit")} />);
  expect(screen.queryByTestId("grok-panel")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "重新授权此账号" }));
  expect(latest().credentialId).toBe("stable-id");
});

it("does not turn another preset into Build OAuth based on an ID or brand", () => {
  render(<ConsoleGlobalDialogs controller={controller("add", "xai-api")} />);
  expect(screen.queryByTestId("grok-panel")).not.toBeInTheDocument();
});
