import { act, fireEvent, render, renderHook, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { ModelPoolWorkspace } from "./ModelPoolWorkspace";
import { ModelPriorityList } from "./ModelPriorityList";
import type { ModelPoolCard } from "./modelPoolViewModel";
import { useModelPoolEditor } from "./useModelPoolEditor";
import { populatedWorkspaceProps, renderWorkspace } from "./CredentialGroupsWorkspace.fixtures";

const t = (_zh: string, en: string) => en;
const model: ModelPoolCard = { rowId: "m", model: "sample", accountCount: 3, enabledAccountCount: 3, enabled: false, pinned: true, routedPatterns: ["sample"], metrics: null,
  chain: ["alpha", "beta", "gamma"].map(id => ({ providerId: id, providerLabel: id, accountCount: 1, enabledAccountCount: 1, accountIds: [id], detached: false, metrics: null })) };

it("expands the directory, preserves route controls and bounds the catalog", async () => {
  const user = userEvent.setup();
  const move = vi.fn(); const edit = vi.fn(); const reset = vi.fn(); const toggle = vi.fn();
  render(<NeuroProvider><ModelPoolWorkspace t={t} models={[model]} editorLocked={false} onMoveProvider={move} onResetChain={reset} onEditModel={edit} onDeleteModel={vi.fn()} onToggleEnabled={toggle} /></NeuroProvider>);
  expect(screen.queryByRole("table", { name: "Provider priority" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "sample" }));
  await user.click(screen.getByRole("button", { name: "Move gamma up" }));
  expect(move).toHaveBeenCalledWith("sample", "gamma", "up");
  await user.click(screen.getByRole("button", { name: "Edit model" })); expect(edit).toHaveBeenCalledWith("sample");
  await user.click(screen.getByRole("button", { name: "Restore inherited order" })); expect(reset).toHaveBeenCalledWith("sample");
  await user.click(screen.getByRole("switch")); expect(toggle).toHaveBeenCalledWith("sample", true);
});

it("commits an arbitrary reorder once and preserves disabled state and unknown route fields", () => {
  const replace = vi.fn();
  const route = { pattern: "sample", provider_ids: ["alpha", "beta", "gamma"], priority: 9, enabled: false, extra: "preserved" };
  const { result } = renderHook(() => useModelPoolEditor({
    t, editorText: JSON.stringify({ providers: [], model_routes: [route], aliases: {} }), modelRouteDraftRows: [{ id: "r", pattern: "sample", route }],
    setModelRouteDraftRows: vi.fn(), setError: vi.fn(), replaceEditorDocument: replace, modelPoolDirectory: [model], modelPoolDialog: null,
    setModelPoolDialog: vi.fn(), providerModelMappingProviderId: null, setProviderModelMappingProviderId: vi.fn(),
  }));
  act(() => result.current.reorderModelPoolChain("sample", "gamma", 0));
  expect(replace).toHaveBeenCalledTimes(1);
  expect(replace.mock.calls[0][0].model_routes[0]).toEqual({ ...route, provider_ids: ["gamma", "alpha", "beta"] });
  act(() => result.current.reorderModelPoolChain("sample", "gamma", -1));
  act(() => result.current.reorderModelPoolChain("sample", "unknown", 0));
  act(() => result.current.reorderModelPoolChain("sample", "gamma", 2));
  expect(replace).toHaveBeenCalledTimes(1);
});

it("supports keyboard order changes and blocks changes while locked", async () => {
  const onMove = vi.fn(); const user = userEvent.setup();
  const props = { chain: model.chain, t, locked: false, onMove, onAccounts: vi.fn() };
  const view = render(<NeuroProvider><ModelPriorityList {...props} /></NeuroProvider>);
  const grip = screen.getByRole("button", { name: "Drag gamma to reorder" });
  grip.focus(); await user.keyboard("{ArrowUp}"); expect(onMove).toHaveBeenCalledWith("gamma", "up");
  view.rerender(<NeuroProvider><ModelPriorityList {...props} locked /></NeuroProvider>);
  expect(screen.getByRole("button", { name: "Drag gamma to reorder" })).toBeDisabled();
  onMove.mockClear(); fireEvent.keyDown(grip, { key: "ArrowUp" }); expect(onMove).not.toHaveBeenCalled();
});

it("renders entitlement master/detail using selected membership and exposes model scope", async () => {
  const props = populatedWorkspaceProps(); const user = userEvent.setup();
  renderWorkspace(props);
  expect(screen.getByRole("navigation", { name: "Entitlement groups" })).toBeVisible();
  const accounts = screen.getByRole("table", { name: "Group accounts" });
  expect(within(accounts).getAllByRole("row").length).toBeGreaterThan(1);
  await user.click(screen.getByRole("radio", { name: "Models" }));
  expect(screen.getByRole("table", { name: "Group models" })).toBeVisible();
  expect(screen.queryByText("Flip card")).not.toBeInTheDocument();
});
