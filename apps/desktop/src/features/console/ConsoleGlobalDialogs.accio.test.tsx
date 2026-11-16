import type { ComponentProps } from "react";
import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render as renderUi, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ConsoleGlobalDialogs } from "./ConsoleGlobalDialogs";

const render = (ui: React.ReactNode) => renderUi(ui, { wrapper: NeuroProvider });

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
vi.mock("./AccountGroupDialog", () => ({ AccountGroupDialog: () => null }));
vi.mock("./ProviderCatalogDialog", () => ({ ProviderCatalogDialog: () => null }));
vi.mock("./CredentialPruneDialog", () => ({ CredentialPruneDialog: () => null }));
vi.mock("./GeminiManualAddDialog", () => ({ GeminiManualAddDialog: () => null }));
vi.mock("./PilotActionDialog", () => ({ PilotActionDialog: () => null }));
vi.mock("./useCredentialKeyReveal", () => ({ useCredentialKeyReveal: () => undefined }));
vi.mock("./useChatgptWebBrowserCapture", () => ({ useChatgptWebBrowserCapture: () => undefined }));
type Controller = ComponentProps<typeof ConsoleGlobalDialogs>["controller"];
function controller(mode = "add", preset = "accio"): Controller {
  return {
    client: { request: vi.fn() }, session: { managementToken: "owner", secretGrant: { grant: "grant" }, confirmSecretAccess: vi.fn() },
    routeConfig: { routeConfig: { revision: { id: "saved-r1" } } },
    draftDocumentState: { document: { providers: [{ id: "renamed-provider", preset, base_url: "https://phoenix-gw.alibaba.com" }] } },
    credentialDialogState: {
      mode, initialValue: {
        providerId: "renamed-provider", credentialId: mode === "edit" ? "stable-id" : "", accountName: "", enabled: false,
        baseUrl: "", supportedModelsText: "", apiKeyOperation: mode === "edit" ? "keep" : "replace", apiKeyValue: ""
      }
    },
    credentialProviderOptions: [{ id: "renamed-provider", label: "Renamed" }], explicitCredentialIds: [],
    displayedAccountCatalog: { accounts: [] }, groupDirectory: [], editorLocked: false, draftDirty: false, draftMatchesActiveRevision: true,
    hasSecretAccess: true, setCredentialDialogState: vi.fn(), applyCredentialDialogValue: vi.fn(), refresh: vi.fn(), t: (zh: string) => zh,
  } as unknown as Controller;
}

it("offers Accio sign-in by exact preset even when the provider ID was renamed", () => {
  render(<ConsoleGlobalDialogs controller={controller()} />);
  expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
});

it("keeps stable-ID editing separate from explicit reauthorization", () => {
  const owner = controller("edit"); render(<ConsoleGlobalDialogs controller={owner} />);
  expect(screen.queryByRole("button", { name: "通过 Accio 登录" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "重新授权此账号" }));
  expect(screen.getByText("stable-id")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toBeInTheDocument();
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled();
});

it("retains the explicitly selected legacy manual entry without feeding OAuth results into it", () => {
  const owner = controller(); render(<ConsoleGlobalDialogs controller={owner} />);
  fireEvent.change(screen.getByLabelText("录入方式"), { target: { value: "token" } });
  expect(screen.queryByRole("button", { name: "通过 Accio 登录" })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "manual-stable" } });
  fireEvent.change(screen.getByLabelText("Access Token"), { target: { value: "synthetic-access" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({
    providerId: "renamed-provider", credentialId: "manual-stable", apiKeyValue: "synthetic-access", apiKeyOperation: "replace",
  }));
});

it("preserves ordinary keep editing and blocks reauthorization after local secret changes", () => {
  const owner = controller("edit"); render(<ConsoleGlobalDialogs controller={owner} />);
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(owner.applyCredentialDialogValue).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "stable-id", enabled: false, apiKeyOperation: "keep" }));
  fireEvent.change(screen.getByLabelText("Access Token"), { target: { value: "synthetic-new" } });
  expect(screen.getByRole("button", { name: "重新授权此账号" })).toBeDisabled();
});

it.each(["editorLocked", "draftDirty", "draftMatchesActiveRevision"] as const)("locks Accio when %s is unsafe", (field) => {
  const owner = controller(); owner[field] = field !== "draftMatchesActiveRevision";
  render(<ConsoleGlobalDialogs controller={owner} />);
  expect(screen.getByLabelText("Client ID")).toBeDisabled(); expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toBeDisabled();
});

it("does not enable Accio login on another preset with the same display identity", () => {
  render(<ConsoleGlobalDialogs controller={controller("add", "xai-api")} />);
  expect(screen.queryByRole("button", { name: "通过 Accio 登录" })).not.toBeInTheDocument();
});

it.each(["add", "edit"])("saves through the real Accio API client once and only refreshes on success: %s", async (mode) => {
  const owner = controller(mode);
  const request = vi.fn(async (path, schema, options) => {
    const status = path.endsWith("/import") ? "succeeded" : path.endsWith("/complete") ? "ready" : "waiting_user";
    return schema.parse({
      session: {
        id: "login", providerId: "renamed-provider", expectedRevision: "saved-r1", status,
        authorizationUrl: "https://zh.accio.com/login?state=synthetic", credentialId: status === "succeeded" ? mode === "edit" ? "stable-id" : "new-id" : null,
        targetCredentialId: mode === "edit" ? "stable-id" : null, savedRevision: status === "succeeded" ? "saved-r2" : null, errorCode: null
      }
    });
  });
  owner.client = { request };
  render(<ConsoleGlobalDialogs controller={owner} />);
  if (mode === "edit") fireEvent.click(screen.getByRole("button", { name: "重新授权此账号" }));
  for (const [label, value] of [["Client ID", "permitted"], ["授权站点", "global"], ["回调端口", "34567"], ["utdid", "explicit-device"]]) {
    fireEvent.change(screen.getByLabelText(label), { target: { value } });
  }
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 Accio 登录" })); });
  expect(request.mock.calls[0][2].body).toMatchObject({
    providerId: "renamed-provider", expectedRevision: "saved-r1", loginSite: "global", callbackPort: 34567,
    ...(mode === "edit" ? { credentialId: "stable-id" } : {})
  });
  fireEvent.change(screen.getByLabelText("完整回调地址"), { target: { value: "synthetic-callback" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "完成授权" })); });
  expect(owner.refresh).not.toHaveBeenCalled();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "保存到凭证池" })); });
  expect(owner.refresh).toHaveBeenCalledOnce(); expect(owner.setCredentialDialogState).toHaveBeenCalledWith(null);
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled();
  expect(request.mock.calls.filter(([path]) => path.endsWith("/import"))).toHaveLength(1);
});

it("cannot bypass an unknown OAuth save by switching into legacy manual creation", async () => {
  const owner = controller();
  owner.client = {
    request: vi.fn(async (path, schema) => {
      if (path.endsWith("/import")) throw new Error("lost save response");
      return schema.parse({
        session: {
          id: "login", providerId: "renamed-provider", expectedRevision: "saved-r1",
          status: path.endsWith("/complete") ? "ready" : "waiting_user", authorizationUrl: "https://zh.accio.com/login",
          credentialId: null, targetCredentialId: null, savedRevision: null, errorCode: null
        }
      });
    })
  };
  render(<ConsoleGlobalDialogs controller={owner} />);
  for (const [label, value] of [["Client ID", "permitted"], ["授权站点", "global"], ["回调端口", "34567"], ["utdid", "device"]]) {
    fireEvent.change(screen.getByLabelText(label), { target: { value } });
  }
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 Accio 登录" })); });
  fireEvent.change(screen.getByLabelText("完整回调地址"), { target: { value: "synthetic-callback" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "完成授权" })); });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "保存到凭证池" })); });
  expect(screen.getByLabelText("录入方式")).toBeDisabled();
  expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
  expect(owner.applyCredentialDialogValue).not.toHaveBeenCalled();
});

it("presents restoration failures through the standard notice and delegates retry to the session owner", async () => {
  const owner = controller();
  owner.credentialDialogState = null;
  owner.secretDialogOpen = true;
  const retry = vi.fn().mockRejectedValue(new Error("session owner retains this failure"));
  owner.session = { ...owner.session, secretGrant: null, error: "restore failed", busy: false, confirmSecretAccess: retry };
  const { rerender } = render(<ConsoleGlobalDialogs controller={owner} />);
  expect(screen.getByRole("alert")).toHaveClass("n-notice", "n-tone-danger");
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "重试" })); });
  expect(retry).toHaveBeenCalledExactlyOnceWith("owner");
  expect(screen.getByRole("alert")).toHaveTextContent("restore failed");
  owner.session = { ...owner.session, busy: true };
  rerender(<ConsoleGlobalDialogs controller={owner} />);
  expect(screen.getByRole("button", { name: "重试" })).toBeDisabled();
  owner.session = { ...owner.session, busy: false, error: null };
  rerender(<ConsoleGlobalDialogs controller={owner} />);
  expect(screen.getByRole("status")).toHaveTextContent("正在恢复管理会话…");
  expect(screen.queryByRole("button", { name: "重试" })).not.toBeInTheDocument();
  owner.secretDialogOpen = false;
  rerender(<ConsoleGlobalDialogs controller={owner} />);
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
});
