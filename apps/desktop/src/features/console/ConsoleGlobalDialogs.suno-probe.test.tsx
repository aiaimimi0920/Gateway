//! Exercise real global-to-music wiring with an unchanged account across revisions.
import type { ComponentProps } from "react";
import { NeuroProvider } from "@neuro/ui";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { ConsoleGlobalDialogs } from "./ConsoleGlobalDialogs";
import { sunoProbeProps } from "./SunoMediaProbe.fixtures";

vi.mock("./AccountGroupDialog", () => ({ AccountGroupDialog: () => null }));
vi.mock("./ProviderCatalogDialog", () => ({ ProviderCatalogDialog: () => null }));
vi.mock("./CredentialPruneDialog", () => ({ CredentialPruneDialog: () => null }));
vi.mock("./GeminiManualAddDialog", () => ({ GeminiManualAddDialog: () => null }));
vi.mock("./useCredentialKeyReveal", () => ({ useCredentialKeyReveal: () => undefined }));
vi.mock("./useChatgptWebBrowserCapture", () => ({ useChatgptWebBrowserCapture: () => undefined }));
type Controller = ComponentProps<typeof ConsoleGlobalDialogs>["controller"];

it.each(["consent", "inflight", "grant-consent", "grant-inflight"])("invalidates Suno %s across a revision or grant change with identical account content", async mode => {
  const props = sunoProbeProps(), user = userEvent.setup();
  let resolve!: (value: unknown) => void;
  props.startProviderProbe = vi.fn().mockImplementation(() => new Promise(done => { resolve = done; }));
  const owner = { ...props, client: { request: vi.fn() },
    session: { managementToken: "synthetic-owner", secretGrant: { grant: "synthetic-grant" } },
    routeConfig: { routeConfig: { revision: { id: "r1" } } },
    displayedAccountCatalog: { accounts: [] }, groupDirectory: [], credentialProviderOptions: [],
    credentialDialogState: null, draftDocumentState: { document: props.testPolicyDocument }, hasSecretAccess: true,
  } as unknown as Controller;
  const view = render(<ConsoleGlobalDialogs controller={owner} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  if (mode.endsWith("inflight")) await user.click(screen.getByRole("button", { name: "确认生成" }));
  const next = mode.startsWith("grant")
    ? { ...owner, session: { ...owner.session, secretGrant: { ...owner.session.secretGrant!, grant: "synthetic-grant-2" } } }
    : { ...owner, routeConfig: { ...owner.routeConfig!, routeConfig: { ...owner.routeConfig!.routeConfig,
      revision: { ...owner.routeConfig!.routeConfig.revision, id: "r2" } } } };
  view.rerender(<ConsoleGlobalDialogs controller={next} />);
  if (mode.endsWith("consent")) {
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(props.startProviderProbe).not.toHaveBeenCalled();
  } else {
    await act(async () => resolve({ result: { results: [{ credentialId: "exact", status: "passed", message: "old-revision-music" }] } }));
    expect(screen.queryByText("old-revision-music")).not.toBeInTheDocument();
    expect(props.startProviderProbe).toHaveBeenCalledOnce();
  }
});
