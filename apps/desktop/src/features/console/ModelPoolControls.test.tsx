import { NeuroProvider } from "@neuro/ui";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState, type ComponentProps } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ModelPoolChainBoard } from "./ModelPoolChainBoard";
import { ProviderPoolToolbar } from "./ProviderPoolToolbar";
import { ConsoleModelWorkspace } from "./ConsoleModelWorkspace";
import { ProviderModelMappingDialog } from "./ProviderModelMappingDialog";

const t = (_zh: string, en: string) => en;
beforeEach(() => localStorage.setItem("gateway-ui-locale", "en-US"));
it("reserves the search-clear node without duplicating directory access filters", async () => {
  function Toolbar() {
    const [value, setValue] = useState<ComponentProps<typeof ProviderPoolToolbar>["value"]>({ query: "", sort: "default" });
    return <ProviderPoolToolbar value={value} onChange={setValue} t={t} />;
  }
  render(<Toolbar />, { wrapper: NeuroProvider }); const user = userEvent.setup();
  const clear = screen.getByRole("button", { name: "Clear site search" });
  expect(clear).toBeDisabled();
  await user.type(screen.getByRole("searchbox"), "model");
  expect(screen.getByRole("button", { name: "Clear site search" })).toBe(clear);
  await user.click(clear);
  expect(clear).toBeDisabled(); expect(screen.getByRole("searchbox")).toHaveValue("");
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
});

it("keeps per-row scope nodes, unknown exclusions and page boundaries while forwarding exact moves", async () => {
  const onMove = vi.fn(); const changes = vi.fn();
  const chain = Array.from({ length: 6 }, (_, i) => ({
    providerId: `p${i}`, providerLabel: `Provider ${i}`, detached: i === 5,
    accountCount: 1, enabledAccountCount: 1, accountIds: [`a${i}`], metrics: null
  }));
  function Chain() {
    const [excluded, setExcluded] = useState(["missing"]);
    return <ModelPoolChainBoard t={t} cardKey="card" model="model" chain={chain} editorLocked={false}
      deselectedProviderIds={excluded} onDeselectedProviderIdsChange={(next) => { changes(next); setExcluded(next); }} onMoveProvider={onMove} />;
  }
  render(<Chain />, { wrapper: NeuroProvider }); const user = userEvent.setup();
  const checkbox = screen.getByRole("checkbox", { name: "Provider 0" });
  await user.click(checkbox);
  expect(checkbox).toHaveFocus(); expect(checkbox).not.toBeChecked();
  expect(screen.getByRole("checkbox", { name: "Provider 0" })).toBe(checkbox);
  expect(changes).toHaveBeenLastCalledWith(["missing", "p0"]);
  expect(screen.getByRole("button", { name: "Move Provider 0 up one slot" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Next page" }));
  expect(screen.getByRole("checkbox", { name: "Provider 5" })).toBeChecked();
  expect(screen.getByText("No accounts")).toHaveClass("n-badge");
  expect(screen.getByRole("button", { name: "Move Provider 5 down one slot" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Move Provider 4 up one slot" }));
  expect(onMove).toHaveBeenCalledExactlyOnceWith("p4", "up");
});

it("keeps editor-only mapped candidates mounted after unchecking and allows close while locked/saving", async () => {
  const submit = vi.fn(); const close = vi.fn();
  const props = {
    open: true, providerId: "p", providerLabel: "Pool", modelOptions: ["source"], upstreamModelOptions: [],
    initialEntries: [{ model: "source", upstreamModel: "custom" }], locked: false, onSubmit: submit, onOpenChange: close
  };
  const wrapper = ({ children }: { children: React.ReactNode }) => <NeuroProvider><UiLocaleProvider>{children}</UiLocaleProvider></NeuroProvider>;
  const view = render(<ProviderModelMappingDialog {...props} />, { wrapper }); const user = userEvent.setup();
  const target = screen.getByRole("checkbox", { name: "custom" });
  await user.click(target);
  expect(screen.getByRole("checkbox", { name: "custom" })).toBe(target);
  expect(target).toHaveFocus(); expect(target).not.toBeChecked();
  expect(submit).toHaveBeenLastCalledWith([]);
  view.rerender(<ProviderModelMappingDialog {...props} locked savePending />);
  expect(target).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Close" }));
  expect(close).toHaveBeenCalledWith(false); expect(submit).toHaveBeenCalledTimes(1);
});

it("uses cancel-first deletion, revokes confirmation with editor lock and restores a removed trigger to the workspace", async () => {
  const removed = vi.fn();
  function Workspace({ locked = false }: { locked?: boolean }) {
    const [pending, setPending] = useState<string | null>("model");
    return <NeuroProvider><UiLocaleProvider><ConsoleModelWorkspace t={t} editorLocked={locked} modelPoolDirectory={[]}
      draftStructureNotice={null} entitlementAccountCardBridge={undefined} reorderModelPoolChain={vi.fn()}
      resetModelPoolChain={vi.fn()} toggleModelPoolEnabled={vi.fn()} openEditModelDialog={vi.fn()} setPendingModelPoolRemoval={setPending}
      modelPoolDialog={null} modelPoolDialogProviderOptions={[]} modelPoolModelNames={[]} setModelPoolDialog={vi.fn()}
      submitModelPoolDialog={vi.fn()} pendingModelPoolRemoval={pending} deleteModelPoolModel={removed} groupAccountRemovalDialog={null} /></UiLocaleProvider></NeuroProvider>;
  }
  const view = render(<Workspace />); const user = userEvent.setup();
  const dialog = screen.getByRole("alertdialog");
  await waitFor(() => expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus());
  expect(dialog).toHaveTextContent("every account declaration");
  view.rerender(<Workspace locked />);
  expect(screen.getByRole("button", { name: "Delete model" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(removed).not.toHaveBeenCalled();
  await waitFor(() => expect(view.container.querySelector(".nt-model-workspace-frame")).toHaveFocus());
});
