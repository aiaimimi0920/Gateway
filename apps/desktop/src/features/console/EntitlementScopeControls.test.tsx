import { useState } from "react";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NeuroProvider } from "@neuro/ui";
import { afterEach, expect, it, vi } from "vitest";

import { EntitlementGroupScopeBoard, type EntitlementGroupScopeBoardProps } from "./EntitlementGroupScopeBoard";
import { ScopePager } from "./EntitlementScopePager";

afterEach(cleanup);
const t = (_zh: string, en: string) => en;
const providerScopes = (count: number) => Array.from({ length: count }, (_, index) => ({
  providerId: `provider-${index}`, providerLabel: `Provider ${index}`,
  accountCount: 1, providerAccountCount: index === 0 ? 4 : 1, modelCount: 6,
}));
const modelScopes = Array.from({ length: 6 }, (_, index) => ({
  model: `model-${index}`,
  members: providerScopes(10).map(scope => ({
    accountId: `account-${scope.providerId}`, providerId: scope.providerId,
    providerLabel: scope.providerLabel, enabled: true, metrics: null,
  })),
  providerMetrics: [],
}));

function renderBoard(count: number, overrides: Partial<EntitlementGroupScopeBoardProps> = {}) {
  const props: EntitlementGroupScopeBoardProps = {
    t, cardKey: "test-scope", label: "Premium", providerScopes: providerScopes(count),
    modelScopes, deselectedProviderIds: [], onDeselectedProviderIdsChange: vi.fn(), ...overrides,
  };
  function Harness({ current }: { current: EntitlementGroupScopeBoardProps }) {
    const [deselected, setDeselected] = useState(current.deselectedProviderIds);
    return <EntitlementGroupScopeBoard {...current} deselectedProviderIds={deselected}
      onDeselectedProviderIdsChange={next => { current.onDeselectedProviderIdsChange(next); setDeselected(next); }} />;
  }
  const view = (current: EntitlementGroupScopeBoardProps) => <NeuroProvider><Harness current={current} /></NeuroProvider>;
  const result = render(view(props));
  return { ...result, props, user: userEvent.setup(), update: (next: Partial<EntitlementGroupScopeBoardProps>) => result.rerender(view({ ...props, ...next })) };
}

it("uses native inline checkboxes through six providers without replacing the focused choice", async () => {
  const { user, props } = renderBoard(6, { deselectedProviderIds: ["temporarily-absent"] });
  const group = screen.getByRole("group", { name: "Providers, multi-select" });
  const first = within(group).getByRole("checkbox", { name: "Provider 0" });
  expect(within(group).getAllByRole("checkbox")).toHaveLength(6);
  expect(first).toBeChecked();
  expect(first.closest("label")).toHaveClass("n-check");
  expect(first).toHaveAttribute("data-entitlement-scope-partial", "true");
  expect(first).toHaveAccessibleDescription(/provider has 4 accounts; usage is measured per provider account/);
  await user.click(first);
  expect(first).not.toBeChecked();
  expect(first).toHaveFocus();
  expect(within(group).getByRole("checkbox", { name: "Provider 0" })).toBe(first);
  expect(props.onDeselectedProviderIdsChange).toHaveBeenLastCalledWith(["temporarily-absent", "provider-0"]);
  await user.keyboard(" ");
  expect(first).toBeChecked();
  expect(props.onDeselectedProviderIdsChange).toHaveBeenLastCalledWith(["temporarily-absent"]);
  expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
});

it("switches larger provider sets to the standard searchable multi-select", async () => {
  const { user, props } = renderBoard(10, { deselectedProviderIds: ["temporarily-absent", "provider-3"] });
  const trigger = screen.getByRole("button", { name: /Provider scope.*9\/10 selected/ });
  const action = screen.getByRole("button", { name: "Select all" });
  await user.click(trigger);
  const popup = screen.getByRole("dialog", { name: "Provider scope" });
  const first = within(popup).getByRole("checkbox", { name: /^Provider 0/ });
  expect(within(popup).getAllByRole("checkbox")).toHaveLength(10);
  expect(within(popup).getByText("1/4 accounts; provider-wide usage")).toBeVisible();
  await user.click(first);
  expect(first).toHaveFocus();
  expect(first).not.toBeChecked();
  expect(within(popup).getByRole("checkbox", { name: /^Provider 0/ })).toBe(first);
  expect(props.onDeselectedProviderIdsChange).toHaveBeenLastCalledWith(["temporarily-absent", "provider-3", "provider-0"]);
  expect(screen.getByRole("button", { name: /Provider scope.*8\/10 selected/ })).toBe(trigger);
  const search = within(popup).getByRole("searchbox", { name: "Search providers" });
  await user.type(search, "Provider 3");
  expect(within(popup).getAllByRole("checkbox")).toHaveLength(1);
  await user.click(within(popup).getByRole("checkbox"));
  expect(props.onDeselectedProviderIdsChange).toHaveBeenLastCalledWith(["temporarily-absent", "provider-0"]);
  await user.clear(search);
  await user.type(search, "not-present");
  expect(within(popup).getByRole("status")).toHaveTextContent("No providers found");
  await user.keyboard("{Escape}");
  expect(trigger).toHaveFocus();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Select all" })).toBe(action);
});

it("keeps an absent provider excluded after a directory refresh and a dropdown change", async () => {
  const { user, update, props } = renderBoard(10, { deselectedProviderIds: ["provider-2"] });
  update({ providerScopes: providerScopes(10).filter(scope => scope.providerId !== "provider-2") });
  await user.click(screen.getByRole("button", { name: /Provider scope/ }));
  await user.click(screen.getByRole("checkbox", { name: /^Provider 0/ }));
  expect(props.onDeselectedProviderIdsChange).toHaveBeenLastCalledWith(["provider-2", "provider-0"]);
  await user.keyboard("{Escape}");
  update({ providerScopes: providerScopes(10) });
  await user.click(screen.getByRole("button", { name: /Provider scope/ }));
  expect(screen.getByRole("checkbox", { name: /^Provider 2/ })).not.toBeChecked();
  expect(screen.getByRole("checkbox", { name: /^Provider 0/ })).not.toBeChecked();
});

it.each([2, 10])("keeps whole-scope actions and resets model pagination for %i providers", async count => {
  const { user, props } = renderBoard(count);
  const pager = () => within(screen.getByRole("navigation", { name: "Model scope pagination" }));
  await user.click(pager().getByRole("button", { name: "Next page" }));
  expect(pager().getByText("2 / 2")).toBeInTheDocument();
  if (count > 6) await user.click(screen.getByRole("button", { name: /Provider scope/ }));
  await user.click(screen.getByRole("checkbox", { name: count > 6 ? /^Provider 0/ : "Provider 0" }));
  if (count > 6) await user.keyboard("{Escape}");
  expect(pager().getByText("1 / 2")).toBeInTheDocument();
  const action = screen.getByRole("button", { name: "Select all" });
  await user.click(action);
  expect(props.onDeselectedProviderIdsChange).toHaveBeenLastCalledWith([]);
  expect(screen.getByRole("button", { name: "Clear" })).toBe(action);
  await user.click(action);
  expect(props.onDeselectedProviderIdsChange).toHaveBeenLastCalledWith(providerScopes(count).map(scope => scope.providerId));
  expect(screen.queryByRole("navigation", { name: "Model scope pagination" })).not.toBeInTheDocument();
  expect(screen.getByText("Select providers to list models")).toBeVisible();
  expect(screen.getByRole("button", { name: "Select all" })).toBe(action);
  await user.click(action);
  expect(pager().getByText("1 / 2")).toBeInTheDocument();
});

it("renders truthful empty scopes without creating a picker or pager", () => {
  renderBoard(0);
  expect(screen.getByText("No providers yet, attach accounts first")).toBeVisible();
  expect(screen.getByText("Select providers to list models")).toBeVisible();
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
  expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
});

it("shares standard pagination with exact callbacks, boundary disabling and stable buttons", async () => {
  const onPage = vi.fn();
  const user = userEvent.setup();
  const props = { t, page: 1, totalPages: 10, navLabel: "Scope pages", onPage };
  const { rerender } = render(<ScopePager {...props} />);
  const previous = screen.getByRole("button", { name: "Previous page" });
  const next = screen.getByRole("button", { name: "Next page" });
  expect(previous).toBeDisabled();
  expect(previous).toHaveClass("n-icon-button");
  expect(previous).not.toHaveClass("nt-icon-action");
  await user.click(previous);
  expect(onPage).not.toHaveBeenCalled();
  await user.click(next);
  expect(onPage).toHaveBeenLastCalledWith(2);
  rerender(<ScopePager {...props} page={9} />);
  expect(screen.getByRole("button", { name: "Next page" })).toBe(next);
  await user.click(next);
  expect(onPage).toHaveBeenLastCalledWith(10);
  rerender(<ScopePager {...props} page={10} />);
  expect(next).toBeDisabled();
  expect(screen.getByText("10 / 10")).toBeVisible();
  await user.click(previous);
  expect(onPage).toHaveBeenLastCalledWith(9);
  rerender(<ScopePager {...props} totalPages={1} />);
  expect(screen.queryByRole("navigation")).not.toBeInTheDocument();
});
